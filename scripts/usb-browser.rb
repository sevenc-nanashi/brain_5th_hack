#!/usr/bin/env ruby
require_relative 'usb-probe'
require_relative 'browser-http'
require 'securerandom'

module BrainUsb
  class BrowserBridge
    MAGIC = 'BRNWEB01'.b.freeze
    HEADER = 44
    FRAME = 468

    def initialize(device)
      @device = device
      @device.guard_drm!
      @version = Integer(@device.attribute('bcdDevice'), 16)
      @token = SecureRandom.random_bytes(8)
      @sequence = 0
    end

    def exchange(id, kind, offset: 0, total: 0, status: 0, body: ''.b)
      raise 'Frame too large' if body.bytesize > FRAME - HEADER
      frame = MAGIC + @token + [@sequence, id, kind, offset, total, status, body.bytesize].pack('V7') + body
      frame = frame.ljust(FRAME, "\0")
      packet = "\0".b * 512
      packet[0, 16] = BrainUsb::MAGIC
      packet[0x10, 28] = [0x2c, 0x210 | (@version << 16), 1, FRAME, 0xffff0000, FRAME, @sequence].pack('V7')
      packet[0x2c, FRAME] = frame
      sum = packet.bytes.sum & 0xffff
      packet[0x20, 4] = [sum | ((0xffff - sum) << 16)].pack('V')
      @device.sector(0xffd, packet)
      deadline = Process.clock_gettime(Process::CLOCK_MONOTONIC) + 30
      loop do
        raise 'USB reply timed out; restart Browser and bridge' if Process.clock_gettime(Process::CLOCK_MONOTONIC) >= deadline
        @device.sector(0xfec, BrainUsb.packet(3, @version))
        status_packet = BrainUsb.decode(@device.sector(0xfec), 0x34)
        raise 'Invalid USB status' unless status_packet.bytesize == 4
        flags = status_packet.unpack1('V')
        raise 'USB disabled or packet rejected' if flags & 1 == 0 || flags & 0x200 != 0
        @device.sector(0xfec, BrainUsb.packet(2, @version)) if flags & 0x10 == 0
        raw = @device.sector(0xffd)
        unless raw == "\0".b * 512
          reply = BrainUsb.decode(raw, 0x2c)
          if reply.bytesize == FRAME && reply.start_with?(MAGIC) && reply.byteslice(8, 8) == @token
            raise 'Invalid USB total length' unless raw.byteslice(0x24, 4).unpack1('V') == FRAME
            seq, rid, type, position, size, result, length = reply.byteslice(16, 28).unpack('V7')
            if seq == @sequence
              raise 'Invalid browser response' unless length <= FRAME - HEADER && [10, 11].include?(type) && result <= 1
              raise 'Mismatched browser acknowledgement' unless kind == 0 ? type == 10 : type == 11 && rid == id && position == offset
              @sequence = (@sequence + 1) & 0xffffffff
              return [rid, position, size, result, reply.byteslice(HEADER, length)]
            end
          end
        end
        sleep 0.005
      end
    end

    def next_url
      id, offset, total, _, bytes = exchange(0, 0, total: Time.now.to_i)
      return nil if id == 0
      raise 'Invalid URL response' unless offset == 0 && total.between?(1, 2048)
      url = bytes
      while url.bytesize < total
        next_id, offset, size, _, bytes = exchange(id, 0, offset: url.bytesize)
        raise 'URL changed during transfer' unless next_id == id && offset == url.bytesize && size == total && !bytes.empty?
        url << bytes
      end
      raise 'URL length mismatch' unless url.bytesize == total
      [id, url.force_encoding(Encoding::UTF_8)]
    end

    def run
      loop do
        request = next_url
        unless request
          sleep 0.02
          next
        end
        id, url = request
        warn "GET #{url}"
        offset = 0
        BrowserHttp.fetch(url) do |kind, status, bytes|
          _, _, _, result, = exchange(id, kind, offset: offset, total: offset, status: status, body: bytes)
          offset += bytes.bytesize if kind == 2 && result == 0
          result == 0
        end
      end
    end
  end
end

if $PROGRAM_NAME == __FILE__
  abort 'Usage: sudo ruby scripts/usb-browser.rb /dev/sgN' unless ARGV.size == 1
  device = nil
  begin
    device = BrainUsb::Device.new(ARGV[0], write: true)
    BrainUsb::BrowserBridge.new(device).run
  rescue StandardError => error
    warn error.message
    exit 1
  ensure
    device&.close
  end
end
