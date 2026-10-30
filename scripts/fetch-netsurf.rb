#!/usr/bin/env ruby
require 'digest'
require 'fileutils'

root = File.expand_path('..', __dir__)
destination = File.expand_path(ARGV.fetch(0, 'target/netsurf-inputs'), root)
FileUtils.mkdir_p(destination)
files = [
  ['netsurf-all-3.11.tar.gz', 'https://download.netsurf-browser.org/netsurf/releases/source-full/netsurf-all-3.11.tar.gz', '4dea880ff3c2f698bfd62c982b259340f9abcd7f67e6c8eb2b32c61f71644b7b', 'netsurf-all-3.11'],
  ['zlib-1.3.1.tar.gz', 'https://zlib.net/fossils/zlib-1.3.1.tar.gz', '9a93b2b7dfdac77ceba5a558a580e74667dd6fede4585b91eefb60f03b72df23', 'zlib-1.3.1'],
  ['jpegsrc.v9f.tar.gz', 'https://www.ijg.org/files/jpegsrc.v9f.tar.gz', '04705c110cb2469caa79fb71fba3d7bf834914706e9641a4589485c1f832565b', 'jpeg-9f'],
  ['libiconv-1.18.tar.gz', 'https://ftp.gnu.org/pub/gnu/libiconv/libiconv-1.18.tar.gz', '3b08f5f4f9b4eb82f151a7040bfd6fe6c6fb922efe4b1659c66ea933276965e8', 'libiconv-1.18'],
  ['misaki.zip', 'https://littlelimit.net/arc/misaki/misaki_bdf_2021-05-05.zip', 'a275f173cf5935890f84d3e65d05b1bf73028e4d4bf41cb3de0ef3b5ebe8e217', 'misaki'],
  ['newlib-dev.deb', 'https://archive.ubuntu.com/ubuntu/pool/universe/n/newlib/libnewlib-dev_4.4.0.20231231-2_all.deb', 'f945b12c3abc844e76eebc28cf7d99e9aefd7caec39583116ef29556b3c52414', 'newlib/usr/include/newlib'],
  ['newlib-arm.deb', 'https://archive.ubuntu.com/ubuntu/pool/universe/n/newlib/libnewlib-arm-none-eabi_4.4.0.20231231-2_all.deb', '9af508599193d07c9460897d01a36af46a2707765bd6941e27c6d0747859b0eb', %w[libc libm].map { |lib| "newlib/usr/lib/arm-none-eabi/newlib/thumb/v7-a/nofp/#{lib}.a" }],
  ['gperf.deb', 'https://archive.ubuntu.com/ubuntu/pool/universe/g/gperf/gperf_3.1-1build1_amd64.deb', 'f6701027304592fe819d219a5ee00d8b56ba4b2a05a7623dd923f1884d595042', 'tools/usr/bin/gperf']
]
files.each do |name, url, sha, extracted|
  path = File.join(destination, name)
  unless File.file?(path)
    temporary = "#{path}.part"
    begin
      abort "Download failed: #{url}" unless system('curl', '--fail', '--location', '--max-time', '120', url, '-o', temporary)
      abort "Checksum mismatch: #{name}" unless Digest::SHA256.file(temporary).hexdigest == sha
      File.rename(temporary, path)
    ensure
      FileUtils.rm_f(temporary)
    end
  end
  abort "Checksum mismatch: #{name}" unless Digest::SHA256.file(path).hexdigest == sha
  expected_paths = Array(extracted).map { |entry| File.join(destination, entry) }
  next if expected_paths.all? { |entry| File.exist?(entry) }
  command = if name.end_with?('.deb')
    ['dpkg-deb', '-x', path, File.join(destination, name == 'gperf.deb' ? 'tools' : 'newlib')]
  elsif name.end_with?('.zip')
    ['unzip', '-q', path, '-d', File.join(destination, extracted)]
  else
    ['tar', 'xzf', path, '-C', destination]
  end
  abort "Extraction failed: #{name}" unless system(*command)
  abort "Extracted files missing: #{name}" unless expected_paths.all? { |entry| File.exist?(entry) }
end
licenses = Dir.glob(File.join(destination, 'netsurf-all-3.11/*/{COPYING*,LICENSE*}'))
licenses += Dir.glob(File.join(destination, 'libiconv-1.18/COPYING*'))
licenses += [File.join(destination, 'jpeg-9f/README'), File.join(destination, 'zlib-1.3.1/README'), File.join(destination, 'misaki/misaki.txt')]
licenses += Dir.glob(File.join(destination, 'newlib/usr/share/doc/*/copyright'))
File.open(File.join(destination, 'LICENSES.txt'), 'wb') do |output|
  output.write("NetSurf 3.11 Reflex port: GPL-2.0-only.\nSources and build scripts: crates/brain-browser and scripts/*netsurf*.rb in brain_5th_hack.\n\n")
  licenses.sort.each { |path| output.write("\n--- #{path.delete_prefix(destination + '/')} ---\n"); output.write(File.binread(path)) }
end
puts destination
