#!/usr/bin/env ruby
require 'fileutils'
require 'shellwords'

root = File.expand_path('..', __dir__)
inputs = File.expand_path(ARGV.fetch(0), root)
unless File.directory?(File.join(inputs, 'netsurf-all-3.11'))
  abort "NetSurf sources are missing. Run: #{['ruby', File.join(root, 'scripts/fetch-netsurf.rb'), inputs].shelljoin}"
end
lock = File.open(File.join(inputs, 'build.lock'), 'w')
lock.flock(File::LOCK_EX)
target = ARGV.fetch(1)
abort 'Expected host or arm' unless %w[host arm].include?(target)
source = File.join(inputs, 'netsurf-all-3.11')
prefix = File.join(inputs, target)
port = File.join(root, 'crates/brain-browser/engine')
FileUtils.mkdir_p(prefix)
arm = target == 'arm'
flags = %w[-Os -ffunction-sections -fdata-sections -fno-strict-aliasing -Wno-error]
flags << "-I#{prefix}/include"
flags << '-fPIC' unless arm
cc = ['clang-20']
if arm
  cc += %w[--target=armv7a-none-eabi -mcpu=cortex-a7 -mfloat-abi=soft -marm]
  cc += ['-isystem', File.join(inputs, 'newlib/usr/include/newlib')]
end
compiler = File.join(prefix, 'cc')
File.write(compiler, "#!/bin/sh\nexec #{cc.shelljoin} \"$@\"\n")
File.chmod(0755, compiler)
cc = [compiler]
env = {
  'CC' => cc.shelljoin, 'AR' => 'llvm-ar-20', 'RANLIB' => 'llvm-ranlib-20',
  'CFLAGS' => flags.shelljoin, 'PKG_CONFIG_LIBDIR' => File.join(prefix, 'lib/pkgconfig')
}
env['PATH'] = "#{inputs}/tools/usr/bin:#{ENV.fetch('PATH')}"
def run(env, dir, *args)
  abort "Failed: #{args.shelljoin}" unless system(env, *args, chdir: dir)
end

%w[zlib-1.3.1 jpeg-9f libiconv-1.18].each do |name|
  dir = File.join(prefix, "build-#{name}")
  FileUtils.mkdir_p(dir)
  configure = File.join(inputs, name, 'configure')
  unless File.file?(File.join(dir, 'Makefile'))
    options = ["--prefix=#{prefix}"]
    dep_env = env.dup
    if name.start_with?('zlib')
      options << '--static'
      dep_env['CHOST'] = 'arm-none-eabi' if arm
    else
      options += %w[--disable-shared --enable-static]
      if arm
        options << '--host=arm-none-eabi'
        dep_env['LDFLAGS'] = '-nostdlib -Wl,--unresolved-symbols=ignore-all'
      end
    end
    run(dep_env, dir, configure, *options)
  end
  # Only the libraries are needed, not the utilities or their OS interfaces.
  case name
  when /^zlib/
    run(env, dir, 'make', '-j4', 'libz.a')
    FileUtils.mkdir_p(File.join(prefix, 'include'))
    FileUtils.mkdir_p(File.join(prefix, 'lib'))
    FileUtils.cp(File.join(dir, 'libz.a'), File.join(prefix, 'lib'))
    %w[zlib.h zconf.h].each { |f| FileUtils.cp(File.join(inputs, name, f), File.join(prefix, 'include')) }
  when /^jpeg/
    run(env, dir, 'make', '-j4', 'libjpeg.la')
    FileUtils.cp(File.join(dir, '.libs/libjpeg.a'), File.join(prefix, 'lib'))
    %w[jpeglib.h jmorecfg.h jerror.h].each { |f| FileUtils.cp(File.join(inputs, name, f), File.join(prefix, 'include')) }
    FileUtils.cp(File.join(dir, 'jconfig.h'), File.join(prefix, 'include'))
  else
    run(env, File.join(dir, 'libcharset'), 'make', '-j4', 'install-lib')
    FileUtils.cp(File.join(dir, 'libcharset/include/localcharset.h'), File.join(dir, 'include'))
    run(env, File.join(dir, 'lib'), 'make', '-j4', 'install-lib')
    FileUtils.cp(File.join(dir, 'include/iconv.h'), File.join(prefix, 'include'))
  end
end

%w[libwapcaplet libparserutils libhubbub libcss libdom libnsutils libnsbmp libnsgif].each do |name|
  run(env, File.join(source, name), 'make', '-j4', 'install',
      "PREFIX=#{prefix}", "NSSHARED=#{source}/buildsystem", "HOST=#{arm ? 'arm-none-eabi' : 'x86_64-linux-gnu'}",
      "CC=#{cc.shelljoin}", 'AR=llvm-ar-20', 'RANLIB=llvm-ranlib-20',
      'WARNFLAGS=-Wall -Wno-error', 'WITH_EXPAT_BINDING=no', 'DESTDIR=')
end

destination = File.join(source, 'netsurf/frontends/brain')
FileUtils.mkdir_p(destination)
FileUtils.cp_r(Dir.glob(File.join(port, '*')), destination, preserve: true)
run(env, root, 'ruby', 'scripts/netsurf-resources.rb', inputs, destination)
# The released core has no frontend registration hook for additional fetchers.
fetch_path = File.join(source, 'netsurf/content/fetch.c')
fetch_source = File.read(fetch_path)
unless fetch_source.include?('brain_fetch_register')
  old = "\tret = fetch_file_register();"
  raise 'NetSurf fetch registration changed' unless fetch_source.include?(old)
  fetch_source.sub!(old, "\textern nserror brain_fetch_register(void);\n\tret = brain_fetch_register();")
  File.write(fetch_path, fetch_source)
end
curl_header = File.join(source, 'netsurf/content/fetchers/curl.h')
curl_source = File.read(curl_header)
curl_source.sub!("#include <curl/curl.h>", "#ifdef WITH_CURL\n#include <curl/curl.h>\n#endif") unless curl_source.include?("#ifdef WITH_CURL\n#include")
curl_source.sub!("extern CURLM *fetch_curl_multi;", "#ifdef WITH_CURL\nextern CURLM *fetch_curl_multi;\n#endif") unless curl_source.include?("#ifdef WITH_CURL\nextern")
File.write(curl_header, curl_source)
nsflags = flags + ["-I#{prefix}/include", '-Wno-error']
if arm
  nsflags += ['-DBRAIN_ARM', "-I#{destination}/compat", '-include', "#{destination}/compat/brain_config.h"]
end
run(env, File.join(source, 'netsurf'), 'make', '-j4', 'brain-archive', 'TARGET=brain',
    'VLDTARGET=brain',
    "HOST=#{arm ? 'arm-none-eabi' : 'x86_64-linux-gnu'}", "PREFIX=#{prefix}",
    "CC=#{compiler}", 'AR=llvm-ar-20', "BRAIN_CFLAGS=#{nsflags.join(' ')}",
    'NETSURF_USE_LIBICONV_PLUG=NO')
unless arm
  libraries = %w[brain-netsurf dom css hubbub parserutils wapcaplet nsutils nsbmp nsgif jpeg iconv z]
  run(env, prefix, compiler, '-shared', '-Wl,-z,defs', '-o', 'libbrain-browser.so',
      '-Wl,--whole-archive', 'lib/libbrain-netsurf.a', '-Wl,--no-whole-archive',
      '-Llib', '-Wl,--start-group', *libraries.drop(1).map { |lib| "-l#{lib}" }, '-Wl,--end-group', '-lm')
end
puts prefix
