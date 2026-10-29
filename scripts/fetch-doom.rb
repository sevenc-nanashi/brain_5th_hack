#!/usr/bin/env ruby
require 'digest'
require 'fileutils'

root = File.expand_path('..', __dir__)
destination = File.expand_path(ARGV.fetch(0, 'target/doom-inputs'), root)
FileUtils.mkdir_p(destination)
opl = 'https://raw.githubusercontent.com/Wohlstand/libADLMIDI/3e9ddb13ad39e2de0ca7eb003e622d4741aa50b7/src/chips/opal'
files = {
  'opal.c' => ["#{opl}/opal.c", '1538eb96af39b3549f8bb487c3ff680da23cc0e19dd4ed7abca2eed46d97ccf0'],
  'opal.h' => ["#{opl}/opal.h", '7878746c9c1a53f6bef23cce23af5732fdddbcafba084d246b23573ef449bda0'],
  'OPAL-LICENSE.txt' => ["#{opl}/LICENSE.txt", '0838567096038054ff567932f6b6cfd9c22732a2339a5b1a93f3fd42aee8f253'],
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
