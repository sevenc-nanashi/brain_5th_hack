#!/usr/bin/env ruby
# V690_H1 experimental transport probe. Linux SG_IO avoids block-cache reads.
require 'fiddle/import'
require 'json'
require 'pathname'

module BrainUsb
  HELLO = "REFLEX USB PROBE v1\n".b.freeze
  MAGIC = 'SHDIC COMM KEYWD'.b.freeze
  module Native
    extend Fiddle::Importer
    dlload Fiddle.dlopen(nil)
    extern 'int ioctl(int, unsigned long, void*)'
    Header = struct ['int interface_id', 'int direction', 'unsigned char cmd_len',
      'unsigned char sense_len', 'unsigned short iovec_count', 'unsigned int length',
      'void *data', 'void *command', 'void *sense', 'unsigned int timeout',
      'unsigned int flags', 'int pack_id', 'void *user', 'unsigned char status',
      'unsigned char masked_status', 'unsigned char msg_status', 'unsigned char sense_written',
      'unsigned short host_status', 'unsigned short driver_status', 'int residual',
      'unsigned int duration', 'unsigned int info']
  end

  def self.packet(command, version)
    result = "\0".b * 512
    result[0, 16] = MAGIC
    result[0x10, 20] = [0x34, 0x210 | (version << 16), 0, 4, 0xffff0000].pack('V*')
    result[0x34, 4] = [command].pack('V')
    sum = result.bytes.sum & 0xffff
    result[0x20, 4] = [sum | ((0xffff - sum) << 16)].pack('V')
    result
  end

  def self.decode(data, header_size)
    raise 'Invalid sector length or magic' unless data.bytesize == 512 && data.start_with?(MAGIC)
    header, _, kind, length, checksum = data.byteslice(0x10, 20).unpack('V*')
    raise 'Invalid packet header/length' unless header == header_size && length <= 512 - header
    raise 'Invalid packet kind' unless kind == (header == 0x2c ? 1 : 0)
    sum = data.bytes.sum & 0xffff
    raise 'Invalid checksum' unless (checksum & 0xffff) == sum && (checksum >> 16) == 0xffff - sum
    data.byteslice(header, length)
  end

  class Device
    def initialize(path, write:)
      raise 'Expected /dev/sgN' unless path.match?(%r{\A/dev/sg[0-9]+\z})
      @sys = Pathname.new("/sys/class/scsi_generic/#{File.basename(path)}/device").realpath
      @usb = @sys.ascend.detect { |dir| (dir + 'idVendor').file? }
      raise 'No USB parent' unless @usb
      raise 'Not a Sharp 04dd:92e1 device' unless attribute('idVendor') == '04dd' && attribute('idProduct') == '92e1'
      @io = File.open(path, write ? File::RDWR : File::RDONLY)
    end

    def close = @io.close
    def attribute(name) = (@usb + name).read.strip

    def transfer(cdb, data, write: false)
      sense = "\0".b * 64
      header = Native::Header.malloc(Fiddle::RUBY_FREE)
      header.to_ptr[0, Native::Header.size] = "\0" * Native::Header.size
      header.interface_id = 'S'.ord
      header.direction = write ? -2 : -3
      header.cmd_len = cdb.bytesize
      header.sense_len = sense.bytesize
      header.length = data.bytesize
      header.data = Fiddle::Pointer[data]
      header.command = Fiddle::Pointer[cdb]
      header.sense = Fiddle::Pointer[sense]
      header.timeout = 3000
      result = Native.ioctl(@io.fileno, 0x2285, header.to_ptr)
      raise SystemCallError.new('SG_IO', Fiddle.last_error) if result < 0
      unless header.status == 0 && header.host_status == 0 && header.driver_status == 0 && (header.info & 1) == 0 && header.residual == 0
        raise "SCSI failure: status=#{header.status} host=#{header.host_status} driver=#{header.driver_status} residual=#{header.residual} sense=#{sense.byteslice(0, header.sense_written).unpack1('H*')}"
      end
      data
    end

    def capacity
      transfer([0x25, 0, 0, 0, 0, 0, 0, 0, 0, 0].pack('C*'), "\0".b * 8).unpack('N2')
    end

    def guard_drm!
      raise 'Device revision is not DRM 0103' unless attribute('bcdDevice') == '0103'
      raise 'Device capacity is not the DRM 4096 x 512 layout' unless capacity == [4095, 512]
      nodes = (@sys + 'block').children.flat_map { |block| [block] + block.children.select { |entry| (entry + 'partition').file? } }
      numbers = nodes.map { |node| (node + 'dev').read.strip }
      raise 'Device is mounted; unmount before probing' if File.readlines('/proc/self/mountinfo').any? { |line| numbers.include?(line.split[2]) }
    end

    def sector(lba, data = nil)
      write = !data.nil?
      cdb = [write ? 0x2a : 0x28, 0, lba, 0, 1, 0].pack('CCNCnC')
      transfer(cdb, write ? data : "\0".b * 512, write: write)
    end

    def receive
      guard_drm!
      version = Integer(attribute('bcdDevice'), 16)
      sector(0xfec, BrainUsb.packet(3, version))
      status = BrainUsb.decode(sector(0xfec), 0x34)
      raise 'Invalid status length' unless status.bytesize == 4
      flags = status.unpack1('V')
      raise 'DRM communication is disabled' if flags & 1 == 0
      retained = flags & 0x10 == 0
      # The pending bit excludes the last packet retained for retransmission.
      sector(0xfec, BrainUsb.packet(2, version)) if retained
      data = sector(0xffd)
      payload = BrainUsb.decode(data, 0x2c)
      copies, remainder = payload.bytesize.divmod(HELLO.bytesize)
      raise 'Unexpected payload (run UsbProbe again)' unless copies > 0 && remainder == 0 && payload == HELLO * copies
      raise 'Unexpected total length' unless data.byteslice(0x24, 4).unpack1('V') == payload.bytesize
      sector(0xfec, BrainUsb.packet(2, version))
      retry_data = sector(0xffd)
      BrainUsb.decode(retry_data, 0x2c)
      raise 'Retransmission differs' unless retry_data == data
      puts JSON.pretty_generate(payload: payload, hello_copies: copies, retained: retained,
        sequence: data.byteslice(0x28, 4).unpack1('V'), retransmission: 'identical')
    end
  end
end

if $PROGRAM_NAME == __FILE__
  abort 'Usage: ruby scripts/usb-probe.rb probe|receive /dev/sgN' unless ARGV.size == 2 && %w[probe receive].include?(ARGV[0])
  device = nil
  begin
    device = BrainUsb::Device.new(ARGV[1], write: ARGV[0] == 'receive')
    if ARGV[0] == 'probe'
      last, size = device.capacity
      puts JSON.pretty_generate(vid: device.attribute('idVendor'), pid: device.attribute('idProduct'), revision: device.attribute('bcdDevice'), sectors: last + 1, sector_size: size)
    else
      device.receive
    end
  rescue StandardError => e
    warn e.message
    exit 1
  ensure
    device&.close
  end
end
