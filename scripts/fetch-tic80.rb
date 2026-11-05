#!/usr/bin/env ruby
require 'digest'
require 'fileutils'

root = File.expand_path('..', __dir__)
destination = File.expand_path(ARGV.fetch(0, 'target/tic80-inputs'), root)
FileUtils.mkdir_p(destination)
archives = [
  ['tic80', 'nesbox/TIC-80', 'be42d6f146cfa520b9b1050feba10cc8c14fb3bd', 'f8fd28bfdfeaa28b7964bd88f4bdab29b6e20e2425bd5ee4f09863edf3ede866'],
  ['lua', 'lua/lua', '75ea9ccbea7c4886f30da147fb67b693b2624c26', '9f2f625653b8a6ce9e4489385370256831e71fe2fc15801d9b738aadde45e98d'],
  ['luajit', 'LuaJIT/LuaJIT', 'ea9eb7edb5eac4a9d175d74523db34fac0fa5180', '338a5640be00b89fd0a2a027a7433fcbbb4d006be840d216d84151386b5bcdf7'],
  ['blip', 'nesbox/blip-buf', '330226d9b55ecbeea644e17b5e0f096a165ca07e', 'e86870ecb4b7c3feac6afbf5ddd0aff809e14b2d16d9bde251b712fd4ecd6d99'],
  ['zlib', 'madler/zlib', 'cacf7f1d4e3d44d871b605da3b647f07d718623f', '6d4d6640ca3121620995ee255945161821218752b551a1a180f4215f7d124d45']
]
files = archives.map { |name, repo, commit, sha| ["#{name}.tar.gz", "https://codeload.github.com/#{repo}/tar.gz/#{commit}", sha, name] }
files += [
  ['newlib-dev.deb', 'https://archive.ubuntu.com/ubuntu/pool/universe/n/newlib/libnewlib-dev_4.4.0.20231231-2_all.deb', 'f945b12c3abc844e76eebc28cf7d99e9aefd7caec39583116ef29556b3c52414', 'newlib/usr/include/newlib'],
  ['newlib-arm.deb', 'https://archive.ubuntu.com/ubuntu/pool/universe/n/newlib/libnewlib-arm-none-eabi_4.4.0.20231231-2_all.deb', '9af508599193d07c9460897d01a36af46a2707765bd6941e27c6d0747859b0eb', 'newlib/usr/lib/arm-none-eabi/newlib/thumb/v7-a/nofp']
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
  output = File.join(destination, extracted)
  next if File.directory?(output)
  if name.end_with?('.deb')
    abort "Extraction failed: #{name}" unless system('dpkg-deb', '-x', path, File.join(destination, 'newlib'))
  else
    FileUtils.mkdir_p(output)
    abort "Extraction failed: #{name}" unless system('tar', 'xzf', path, '--strip-components=1', '-C', output)
  end
end
licenses = %w[tic80/LICENSE lua/lua.h luajit/COPYRIGHT blip/license.txt zlib/README].map { |f| File.join(destination, f) }
licenses += Dir.glob(File.join(destination, 'newlib/usr/share/doc/*/copyright'))
File.open(File.join(destination, 'LICENSES.txt'), 'wb') do |out|
  out.write("TIC-80 v1.1.2837 Reflex player. Source revisions and patches: scripts/{fetch,build}-tic80.rb.\n")
  licenses.each { |path| out.write("\n--- #{path.delete_prefix(destination + '/')} ---\n"); out.write(File.binread(path)) }
end
puts destination
