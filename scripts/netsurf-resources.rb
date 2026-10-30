#!/usr/bin/env ruby
require 'fileutils'
inputs, output = ARGV
abort 'Usage: netsurf-resources.rb INPUTS OUTPUT' unless inputs && output
FileUtils.mkdir_p(output)
glyphs = File.read(File.join(inputs, 'misaki/misaki_gothic.bdf')).scan(/STARTCHAR .*?\n(.*?)ENDCHAR/m).map do |(block)|
  code = Integer(block.match(/^ENCODING (\d+)$/)[1])
  advance = Integer(block.match(/^DWIDTH (\d+) 0$/)[1])
  width, height, x, y = block.match(/^BBX (\d+) (\d+) (-?\d+) (-?\d+)$/).captures.map(&:to_i)
  rows = block.split("BITMAP\n", 2).fetch(1).lines.map { |line| Integer(line.strip, 16) }
  raise 'Invalid 8x8 glyph' unless code <= 65535 && rows.size == height && width <= 8 && height <= 8 && x >= 0 && x + width <= 8
  bitmap = Array.new(8, 0)
  rows.each_with_index do |bits, i|
    row = 6 - y - height + i
    raise 'Glyph outside cell' unless row.between?(0, 7) && bits <= 255
    bitmap[row] = bits >> x
  end
  [code, advance, bitmap]
end.sort_by(&:first)
raise 'Missing Japanese glyphs' unless [0x963f, 0x90e8, 0x5bdb].all? { |c| glyphs.any? { |g| g[0] == c } }
raise 'Duplicate glyph' unless glyphs.map(&:first).uniq.size == glyphs.size
File.open(File.join(output, 'misaki.h'), 'w') do |f|
  f.puts '// Generated from Misaki Gothic, Copyright (C) 2002-2021 Num Kadoma.'
  f.puts 'static const struct glyph { uint16_t code; uint8_t advance; uint8_t rows[8]; } glyphs[] = {'
  glyphs.each { |code, advance, rows| f.puts "{#{code},#{advance},{#{rows.join(',')}}}," }
  f.puts '};'
end
File.open(File.join(output, 'resources.h'), 'w') do |f|
  %w[default.css quirks.css internal.css].each_with_index do |name, i|
    data = File.binread(File.join(inputs, 'netsurf-all-3.11/netsurf/resources', name))
    f.puts "static const uint8_t resource_#{i}[] = {#{data.bytes.join(',')}};"
  end
  f.puts 'static const struct { const char *name; const uint8_t *data; size_t length; } resources[] = {'
  %w[default.css quirks.css internal.css].each_with_index { |name, i| f.puts "{\"#{name}\",resource_#{i},sizeof(resource_#{i})}," }
  f.puts '};'
end
