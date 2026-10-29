#!/usr/bin/env ruby
# Pack the hand-drawn glyphs without rasterization or resampling.
root = File.expand_path('..', __dir__)
source = File.join(root, 'crates/reflex/assets/ascii-6x13.glyphs')
glyphs = {}
File.foreach(source).with_index(1) do |line, number|
  next if line.start_with?('#') || line.strip.empty?
  code, top, drawing = line.split
  raise "Invalid glyph at line #{number}" unless code&.match?(/\A[0-9A-F]{2}\z/) && top&.match?(/\A\d+\z/) && drawing
  code = code.to_i(16)
  top = top.to_i
  rows = drawing.split('/')
  raise "Invalid cell at line #{number}" unless (32..126).cover?(code) && top + rows.size <= 13 && rows.all? { |r| r.match?(/\A[.#]{5}\z/) }
  raise "Duplicate glyph #{code}" if glyphs.key?(code)
  bitmap = Array.new(13, 0)
  rows.each_with_index { |row, y| bitmap[top + y] = row.tr('.#', '01').to_i(2) << 1 }
  raise "Empty glyph #{code}" if code != 32 && bitmap.all?(&:zero?)
  glyphs[code] = bitmap
end
raise 'Expected ASCII 32 through 126' unless glyphs.keys.sort == (32..126).to_a
File.binwrite(File.join(root, 'crates/reflex/assets/ascii-6x13.bin'), (32..126).flat_map { |code| glyphs.fetch(code) }.pack('C*'))
