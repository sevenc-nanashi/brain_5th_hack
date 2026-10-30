// SPDX-License-Identifier: GPL-2.0-only
#include "brain.h"
#include "misaki.h"
#include "utils/utf8.h"

static int scale(const plot_font_style_t *style) {
  int px = (style->size * 4 / 3 + PLOT_STYLE_SCALE / 2) / PLOT_STYLE_SCALE;
  int value = (px + 4) / 8;
  return value < 1 ? 1 : value > 8 ? 8 : value;
}
static const struct glyph *lookup(uint32_t code) {
  if (code == 0xa0)
    code = ' ';
  size_t low = 0, high = sizeof(glyphs) / sizeof(glyphs[0]);
  while (low < high) {
    size_t mid = low + (high - low) / 2;
    if (glyphs[mid].code < code)
      low = mid + 1;
    else
      high = mid;
  }
  return low < sizeof(glyphs) / sizeof(glyphs[0]) && glyphs[low].code == code
             ? &glyphs[low]
             : NULL;
}
static int advance(uint32_t code) {
  const struct glyph *g = lookup(code);
  return g ? g->advance : 8;
}
static nserror width(const plot_font_style_t *s, const char *text,
                     size_t length, int *result) {
  int w = 0;
  for (size_t p = 0; p < length; p = utf8_next(text, length, p))
    w += advance(utf8_to_ucs4(text + p, length - p)) * scale(s);
  *result = w;
  return NSERROR_OK;
}
static nserror position(const plot_font_style_t *s, const char *text,
                        size_t length, int x, size_t *offset, int *actual) {
  size_t p = 0;
  int w = 0;
  while (p < length) {
    int a = advance(utf8_to_ucs4(text + p, length - p)) * scale(s);
    if (w + a / 2 > x)
      break;
    w += a;
    p = utf8_next(text, length, p);
  }
  *offset = p;
  *actual = w;
  return NSERROR_OK;
}
static nserror split(const plot_font_style_t *s, const char *text,
                     size_t length, int x, size_t *offset, int *actual) {
  size_t p = 0, boundary = 0;
  int w = 0, boundary_width = 0;
  while (p < length) {
    uint32_t code = utf8_to_ucs4(text + p, length - p);
    int a = advance(code) * scale(s);
    if (code == ' ' || code >= 0x3000) {
      if (p > 0) {
        boundary = p;
        boundary_width = w;
      }
    }
    if (w + a > x && boundary)
      break;
    w += a;
    p = utf8_next(text, length, p);
    if (code >= 0x3000 && w > x) {
      boundary = p;
      boundary_width = w;
      break;
    }
  }
  *offset = p == length ? p : boundary;
  *actual = p == length ? w : boundary_width;
  return NSERROR_OK;
}
nserror brain_text(const struct redraw_context *ctx, const plot_font_style_t *s,
                   int x, int y, const char *text, size_t length) {
  int factor = scale(s);
  for (size_t p = 0; p < length; p = utf8_next(text, length, p)) {
    uint32_t code = utf8_to_ucs4(text + p, length - p);
    const struct glyph *g = lookup(code);
    for (int row = 0; row < 8; row++) {
      unsigned bits = g ? g->rows[row] : (row == 0 || row == 7 ? 0xff : 0x81);
      for (int col = 0; col < 8; col++)
        if (bits & (0x80 >> col))
          for (int dy = 0; dy < factor; dy++)
            for (int dx = 0; dx < factor; dx++)
              brain_pixel(ctx, x + col * factor + dx,
                          y - 6 * factor + row * factor + dy, s->foreground);
    }
    x += advance(code) * factor;
  }
  return NSERROR_OK;
}
struct gui_layout_table brain_layout = {
    .width = width, .position = position, .split = split};
