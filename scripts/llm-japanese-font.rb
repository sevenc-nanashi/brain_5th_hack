# Generate a 16x16 monochrome atlas from Noto Sans CJK (SIL OFL), using FreeType.
require 'open3'
require 'tmpdir'
font = ARGV.fetch(0) { abort 'Usage: ruby scripts/llm-japanese-font.rb NotoSansCJK-Regular.ttc' }
source = <<~C
  #include <ft2build.h>
  #include FT_FREETYPE_H
  #include <assert.h>
  #include <stdio.h>
  int main(int argc, char **argv) {
    assert(argc == 2);
    FT_Library lib; FT_Face face;
    assert(!FT_Init_FreeType(&lib));
    assert(!FT_New_Face(lib, argv[1], 0, &face));
    assert(!FT_Set_Pixel_Sizes(face, 0, 16));
    const unsigned ranges[][2] = {{0x3000,0x30ff},{0x3400,0x9fff},{0xff00,0xffef}};
    for (unsigned r=0; r<3; r++) for (unsigned c=ranges[r][0]; c<=ranges[r][1]; c++) {
      assert(!FT_Load_Char(face, c, FT_LOAD_RENDER));
      FT_GlyphSlot g=face->glyph;
      assert(g->bitmap.pixel_mode == FT_PIXEL_MODE_GRAY || !g->bitmap.width);
      for (int y=0; y<16; y++) {
        unsigned row=0;
        for (int x=0; x<16; x++) {
          int bx=x-g->bitmap_left, by=y-(14-g->bitmap_top);
          int on=bx>=0 && bx<(int)g->bitmap.width && by>=0 && by<(int)g->bitmap.rows && g->bitmap.buffer[by*g->bitmap.pitch+bx]>=96;
          row=(row<<1)|on;
        }
        putchar(row>>8); putchar(row&255);
      }
    }
    FT_Done_Face(face); FT_Done_FreeType(lib);
    return ferror(stdout) ? 1 : 0;
  }
C
Dir.mktmpdir('llm-font') do |dir|
  flags, error, status = Open3.capture3('pkg-config','--cflags','--libs','freetype2')
  raise error unless status.success?
  exe = File.join(dir,'font')
  _, error, status = Open3.capture3('cc','-x','c','-','-o',exe,*flags.split,stdin_data:source)
  raise error unless status.success?
  data, error, status = Open3.capture3(exe,font,binmode:true)
  raise error unless status.success? && data.bytesize == (256+0x6c00+240)*32
  File.binwrite(File.expand_path('../crates/brain-llm/assets/japanese.bin',__dir__),data)
end
