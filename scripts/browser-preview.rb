#!/usr/bin/env ruby
require 'fiddle/import'
require_relative 'browser-http'

abort 'Usage: ruby scripts/browser-preview.rb INPUTS OUTPUT.ppm [URL]' unless (2..3).cover?(ARGV.size)
module BrowserPreview
  extend Fiddle::Importer
  dlload File.join(File.expand_path(ARGV[0]), 'host/libbrain-browser.so')
  extern 'int brain_init(const char*)'
  extern 'int brain_tick(unsigned long long)'
  extern 'int brain_draw(void*)'
  extern 'unsigned int brain_request(void*)'
  extern 'int brain_response(unsigned int, int, int, void*, size_t)'
  extern 'void brain_stop()'
  extern 'const char *brain_status()'
end
url = ARGV.fetch(2, 'http://abehiroshi.la.coocan.jp/')
result = BrowserPreview.brain_init(url)
abort "NetSurf init failed: #{result}" unless result == 0
begin
  started = Process.clock_gettime(Process::CLOCK_MONOTONIC)
  idle = 0
  fetched = 0
  url_pointer = Fiddle::Pointer.malloc(Fiddle::SIZEOF_VOIDP, Fiddle::RUBY_FREE)
  loop do
    now = Process.clock_gettime(Process::CLOCK_MONOTONIC)
    raise 'Preview timed out' if now - started > 120
    BrowserPreview.brain_tick(((now - started) * 1000).to_i)
    id = BrowserPreview.brain_request(url_pointer)
    if id == 0
      idle += 1
      break if idle >= 50
      sleep 0.02
      next
    end
    idle = 0
    address = Fiddle::Pointer.new(url_pointer[0, Fiddle::SIZEOF_VOIDP].unpack1('J')).to_s
    warn "GET #{address}"
    fetched += 1
    BrowserHttp.fetch(address) do |kind, status, bytes|
      warn "HTTP #{status}: #{bytes}" if kind == 4
      BrowserPreview.brain_response(id, kind, status, bytes, bytes.bytesize) == 0
    end
  end
  pixels = Fiddle::Pointer.malloc(854 * 448 * 2, Fiddle::RUBY_FREE)
  raise 'NetSurf did not render a page' unless BrowserPreview.brain_draw(pixels) != 0
  values = pixels[0, 854 * 448 * 2].unpack('S<*')
  File.open(ARGV[1], 'wb') do |out|
    out.write("P6\n854 448\n255\n")
    values.each { |v| out.write([(v >> 11) * 255 / 31, ((v >> 5) & 63) * 255 / 63, (v & 31) * 255 / 31].pack('C3')) }
  end
  raise "Rendered an empty page (#{BrowserPreview.brain_status.to_s})" if values.uniq.size < 8
  puts "Rendered #{fetched} resources: #{ARGV[1]}"
ensure
  BrowserPreview.brain_stop
end
