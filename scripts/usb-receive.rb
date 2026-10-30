#!/usr/bin/env ruby
# Receive the file selected in Explorer. Linux only; reuses the guarded SG_IO transport.
require_relative 'usb-probe'
require 'securerandom'
require 'tempfile'
require 'zlib'
require 'tmpdir'
require 'fileutils'
require_relative 'usb-directory'

module BrainUsb
  class FileReceiver
    MAGIC = 'BRNFILE3'.b.freeze
    CHUNK = 64 * 1024
    MAX_ENCODED = CHUNK + 1024

    def initialize(device)
      @device = device
      @token = SecureRandom.random_bytes(8)
      @sequence = 0
      @buffer = ''.b
      @last_sector = nil
      @wire_bytes = 0
    end

    def sector(lba, data = nil)
      result = @device.sector(lba, data)
      @wire_bytes += 512
      result
    end

    def progress(offset, size)
      now = Process.clock_gettime(Process::CLOCK_MONOTONIC)
      elapsed = now - @started
      return if now - @last_report < 0.25 && offset != size
      @last_report = now
      percent = size.zero? ? 100.0 : offset * 100.0 / size
      $stderr.print format("\r%5.1f%%  Raw %d / %d B (%.1f KiB/s)  USB %d B (%.1f KiB/s)   ",
        percent, offset, size, offset / elapsed / 1024, @wire_bytes, @wire_bytes / elapsed / 1024)
    end

    def inflate(frame, expected)
      inflater = Zlib::Inflate.new
      result = ''.b
      begin
        inflater.inflate(frame) do |chunk|
          raise 'Decompressed block exceeds expected size' if result.bytesize + chunk.bytesize > expected
          result << chunk
        end
        raise 'Incomplete or trailing compressed data' unless inflater.finished? && inflater.total_in == frame.bytesize
        raise 'Invalid decompressed block length' unless result.bytesize == expected
        result
      ensure
        inflater.close
      end
    end

    def request(offset, command)
      payload = MAGIC + @token + [offset, command].pack('Q<V')
      packet = "\0".b * 512
      packet[0, 16] = BrainUsb::MAGIC
      packet[0x10, 28] = [0x2c, 0x210 | (@version << 16), 1, payload.bytesize,
        0xffff0000, payload.bytesize, @sequence].pack('V*')
      packet[0x2c, payload.bytesize] = payload
      sum = packet.bytes.sum & 0xffff
      packet[0x20, 4] = [sum | ((0xffff - sum) << 16)].pack('V')
      sector(0xffd, packet)
      @sequence += 1
    end

    def response(offset, command)
      deadline = Process.clock_gettime(Process::CLOCK_MONOTONIC) + 30
      loop do
        while @buffer.bytesize >= 44
          unless @buffer.start_with?(MAGIC)
            # A retained packet can belong to UsbProbe or a previous session.
            index = @buffer.index(MAGIC, 1)
            @buffer = index ? @buffer.byteslice(index..) : @buffer.byteslice(-7, 7)
            break
          end
          position, size, length, kind, raw_length = @buffer.byteslice(16, 28).unpack('Q<2V3')
          raise 'Invalid file response' unless length <= MAX_ENCODED && [1, 2].include?(kind) && position <= size && raw_length <= CHUNK && raw_length <= size - position
          break if @buffer.bytesize < 44 + length
          frame = @buffer.slice!(0, 44 + length)
          next unless frame.byteslice(8, 8) == @token
          next if position < offset || (kind == 1 && command == 2)
          raise 'Unexpected response offset/command' unless position == offset && kind == command
          expected = command == 1 ? [CHUNK, size - position].min : 0
          raise 'Invalid file chunk length' unless raw_length == expected
          raise 'Invalid completion response' if command == 2 && length != 0
          payload = frame.byteslice(44, length)
          return [size, command == 1 ? inflate(payload, expected) : ''.b]
        end
        raise "Timed out at offset #{offset}; cancel on Brain and restart" if Process.clock_gettime(Process::CLOCK_MONOTONIC) >= deadline
        sector(0xfec, BrainUsb.packet(3, @version))
        status = BrainUsb.decode(sector(0xfec), 0x34)
        raise 'Invalid status length' unless status.bytesize == 4
        flags = status.unpack1('V')
        raise 'DRM communication is disabled' if flags & 1 == 0
        raise 'Device rejected the request' if flags & 0x200 != 0
        sector(0xfec, BrainUsb.packet(2, @version)) if flags & 0x10 == 0
        data = sector(0xffd)
        if data != "\0".b * 512 && data != @last_sector
          payload = BrainUsb.decode(data, 0x2c)
          raise 'Invalid transport total length' unless data.byteslice(0x24, 4).unpack1('V') == payload.bytesize
          @last_sector = data
          @buffer << payload
          raise 'File response buffer overflow' if @buffer.bytesize > 44 + MAX_ENCODED + 512
        else
          sleep 0.005
        end
      end
    end

    def download
      @started = Process.clock_gettime(Process::CLOCK_MONOTONIC)
      @last_report = @started
      offset = 0
      size = nil
      loop do
        request(offset, 1)
        total, data = response(offset, 1)
        raise 'Transfer size changed' if size && size != total
        size = total
        yield data
        offset += data.bytesize
        progress(offset, size)
        break if offset == size
      end
      size
    end

    def acknowledge(size)
      request(size, 2)
      total, = response(size, 2)
      raise 'Final size differs' unless total == size
    end

    def receive(output, directory: false)
      @device.guard_drm!
      @version = Integer(@device.attribute('bcdDevice'), 16)
      destination = File.expand_path(output)
      raise 'Output already exists' if File.exist?(destination) || File.symlink?(destination)
      if directory
        # Linux RENAME_NOREPLACE refuses even an empty directory created during reception.
        Native.extern 'int renameat2(int, const char*, int, const char*, unsigned int)'
        staging = Dir.mktmpdir('.brain-receive-', File.dirname(destination))
        writer = DirectoryWriter.new(staging)
        begin
          size = download { |data| writer.write(data) }
          writer.finish
          acknowledge(size)
          if Process.uid.zero? && ENV['SUDO_UID'] && ENV['SUDO_GID']
            File.chown(Integer(ENV.fetch('SUDO_UID')), Integer(ENV.fetch('SUDO_GID')), staging)
          end
          if Native.renameat2(-100, staging, -100, destination, 1) != 0
            raise SystemCallError.new('Publish received folder', Fiddle.last_error)
          end
          progress(size, size)
        ensure
          writer.close
          FileUtils.remove_entry(staging) if File.directory?(staging)
        end
      else
        # Publish only a complete file; link refuses a destination created meanwhile.
        Tempfile.create(['.brain-receive-', '.part'], File.dirname(destination), binmode: true) do |file|
          size = download { |data| file.write(data) }
          file.flush
          file.fsync
          acknowledge(size)
          if Process.uid.zero? && ENV['SUDO_UID'] && ENV['SUDO_GID']
            file.chown(Integer(ENV.fetch('SUDO_UID')), Integer(ENV.fetch('SUDO_GID')))
          end
          File.link(file.path, destination)
          progress(size, size)
        end
      end
      $stderr.puts
      puts "Saved #{destination}"
    end
  end
end

if $PROGRAM_NAME == __FILE__
  directory = ARGV.delete('--directory') != nil
  abort 'Usage: sudo ruby scripts/usb-receive.rb [--directory] /dev/sgN OUTPUT' unless ARGV.size == 2
  device = nil
  begin
    device = BrainUsb::Device.new(ARGV[0], write: true)
    BrainUsb::FileReceiver.new(device).receive(ARGV[1], directory: directory)
  rescue StandardError => e
    warn e.message
    exit 1
  ensure
    device&.close
  end
end
