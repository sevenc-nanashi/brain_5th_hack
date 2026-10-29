#!/usr/bin/env ruby
require 'digest'
require 'fileutils'

root = File.expand_path('..', __dir__)
destination = File.expand_path(ARGV.fetch(0, 'target/doom-inputs'), root)
FileUtils.mkdir_p(destination)
files = {
  'PureDOOM.h' => [
    'https://raw.githubusercontent.com/Daivuk/PureDOOM/355cfbd16fac119718879239336ee2ea408886bd/PureDOOM.h',
    'ec16ce721436bf3fa4c338320afedd56a93c0b563a94ebf283718503c71ca780'
  ],
  'DOOM1.WAD' => [
    'https://github.com/nneonneo/universal-doom/raw/refs/heads/main/DOOM1.WAD',
    '1d7d43be501e67d927e415e0b8f3e29c3bf33075e859721816f652a526cac771'
  ]
}
files.each do |name, (url, sha)|
  path = File.join(destination, name)
  if File.exist?(path)
    abort "Checksum mismatch: #{path}" unless Digest::SHA256.file(path).hexdigest == sha
    next
  end
  temporary = "#{path}.part"
  begin
    abort "Download failed: #{url}" unless system('curl', '--fail', '--location', '--max-time', '120', url, '--output', temporary)
    abort "Checksum mismatch: #{url}" unless Digest::SHA256.file(temporary).hexdigest == sha
    File.rename(temporary, path)
  ensure
    FileUtils.rm_f(temporary)
  end
end
puts destination
