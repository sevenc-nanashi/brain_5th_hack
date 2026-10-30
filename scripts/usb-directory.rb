# BRNTREE1 directory stream. Paths are raw filename bytes, relative to a private staging directory.
module BrainUsb
  class DirectoryWriter
    def initialize(root)
      @root = root.b
      @buffer = ''.b
      @entries = nil
      @root_seen = false
      @file = nil
      @remaining = 0
    end

    def write(data)
      @buffer << data
      if @entries.nil?
        return if @buffer.bytesize < 12
        raise 'Not a folder transfer; select a folder in Explorer' unless @buffer.start_with?('BRNTREE1')
        @entries = @buffer.byteslice(8, 4).unpack1('V')
        raise 'Invalid entry count' if @entries.zero?
        @buffer.slice!(0, 12)
      end
      loop do
        if @file
          count = [@remaining, @buffer.bytesize].min
          @file.write(@buffer.slice!(0, count))
          @remaining -= count
          if @remaining == 0
            @file.flush
            @file.fsync
            @file.close
            @file = nil
          else
            return
          end
        end
        if @entries == 0
          raise 'Trailing directory data' unless @buffer.empty?
          return
        end
        return if @buffer.bytesize < 9
        kind, length, size = @buffer.unpack('CV2')
        raise 'Invalid directory entry' unless [1, 2].include?(kind) && length <= 259
        raise 'Directory has file data' if kind == 1 && size != 0
        return if @buffer.bytesize < 9 + length
        @buffer.slice!(0, 9)
        name = @buffer.slice!(0, length)
        unless @root_seen
          raise 'Missing root folder' unless kind == 1 && name.empty?
          @root_seen = true
        else
          parts = name.split('/'.b, -1)
          raise 'Unsafe relative path' if name.empty? || name.include?("\0") || name.include?('\\') || parts.any? { |part| ['', '.', '..'].include?(part) }
          parent = name.rpartition('/').first
          raise 'Missing parent folder' unless File.lstat(File.join(@root, parent)).directory?
        end
        @entries -= 1
        target = name.empty? ? @root : File.join(@root, name)
        if kind == 1
          Dir.mkdir(target, 0o700) unless name.empty?
        else
          @file = File.open(target, File::WRONLY | File::CREAT | File::EXCL, 0o600)
          @file.binmode
          @remaining = size
        end
        if !name.empty? && Process.uid.zero? && ENV['SUDO_UID'] && ENV['SUDO_GID']
          File.chown(Integer(ENV.fetch('SUDO_UID')), Integer(ENV.fetch('SUDO_GID')), target)
        end
      end
    end

    def finish
      raise 'Incomplete folder transfer' unless @entries == 0 && @file.nil? && @buffer.empty?
    end

    def close
      @file&.close
      @file = nil
    end
  end
end
