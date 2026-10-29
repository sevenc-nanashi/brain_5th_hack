# Regenerate the embedded ASCII atlas with ImageMagick and DejaVuSansMono.ttf.
require 'open3'
font = ARGV.fetch(0) { abort 'Usage: ruby scripts/doom-help-font.rb /path/to/DejaVuSansMono.ttf' }
draw = (32..126).map do |code|
  char = code.chr.gsub(/[\\']/) { |c| "\\#{c}" }
  "text 0,#{(code - 32) * 26 + 20} '#{char}'"
end.join("\n")
data, error, status = Open3.capture3('convert', '-size', '12x2470', 'xc:black',
  '-font', font, '-pointsize', '20', '-fill', 'white', '-draw', draw,
  '-colorspace', 'Gray', '-depth', '8', 'gray:-')
abort error unless status.success?
raise 'Unexpected atlas size' unless data.bytesize == 95 * 12 * 26
File.binwrite(File.expand_path('../crates/brain-doom/assets/help-font.gray', __dir__), data)

File.binwrite(File.expand_path('../crates/reflex/assets/ascii.gray', __dir__), data)
