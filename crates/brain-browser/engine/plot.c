// SPDX-License-Identifier: GPL-2.0-only
#include "brain.h"
#include "netsurf/types.h"
#include <math.h>
#include <stdlib.h>

static int min(int a, int b) { return a < b ? a : b; }
static int max(int a, int b) { return a > b ? a : b; }
static uint16_t rgb565(uint32_t c) {
  return ((c & 0xf8) << 8) | ((c & 0xfc00) >> 5) | ((c & 0xf80000) >> 19);
}
void brain_pixel(const struct redraw_context *ctx, int x, int y, uint32_t c) {
  struct brain_canvas *p = ctx->priv;
  if (x >= p->x0 && x < p->x1 && y >= p->y0 && y < p->y1)
    p->pixels[y * BRAIN_WIDTH + x] = rgb565(c);
}
static nserror clip(const struct redraw_context *ctx, const struct rect *r) {
  struct brain_canvas *p = ctx->priv;
  p->x0 = max(0, r->x0);
  p->y0 = max(0, r->y0);
  p->x1 = min(BRAIN_WIDTH, r->x1);
  p->y1 = min(BRAIN_HEIGHT, r->y1);
  return NSERROR_OK;
}
static void fill(const struct redraw_context *ctx, int x0, int y0, int x1,
                 int y1, colour c) {
  struct brain_canvas *p = ctx->priv;
  for (int y = max(y0, p->y0); y < min(y1, p->y1); y++)
    for (int x = max(x0, p->x0); x < min(x1, p->x1); x++)
      p->pixels[y * BRAIN_WIDTH + x] = rgb565(c);
}
static nserror line(const struct redraw_context *ctx, const plot_style_t *s,
                    const struct rect *r) {
  if (s->stroke_type == PLOT_OP_TYPE_NONE)
    return NSERROR_OK;
  int thickness = max(1, plot_style_fixed_to_int(s->stroke_width));
  if (r->x0 == r->x1 || r->y0 == r->y1) {
    fill(ctx, min(r->x0, r->x1), min(r->y0, r->y1),
         max(r->x0, r->x1) + thickness, max(r->y0, r->y1) + thickness,
         s->stroke_colour);
    return NSERROR_OK;
  }
  int dx = abs(r->x1 - r->x0), dy = -abs(r->y1 - r->y0);
  if (dx > 32768 || dy < -32768)
    return NSERROR_BAD_SIZE;
  int x = r->x0, y = r->y0, sx = x < r->x1 ? 1 : -1, sy = y < r->y1 ? 1 : -1,
      e = dx + dy;
  for (;;) {
    fill(ctx, x, y, x + thickness, y + thickness, s->stroke_colour);
    if (x == r->x1 && y == r->y1)
      break;
    int e2 = e * 2;
    if (e2 >= dy) {
      e += dy;
      x += sx;
    }
    if (e2 <= dx) {
      e += dx;
      y += sy;
    }
  }
  return NSERROR_OK;
}
static nserror rectangle(const struct redraw_context *ctx,
                         const plot_style_t *s, const struct rect *r) {
  if (s->fill_type != PLOT_OP_TYPE_NONE)
    fill(ctx, r->x0, r->y0, r->x1, r->y1, s->fill_colour);
  if (s->stroke_type != PLOT_OP_TYPE_NONE) {
    int w = max(1, plot_style_fixed_to_int(s->stroke_width));
    fill(ctx, r->x0, r->y0, r->x1, r->y0 + w, s->stroke_colour);
    fill(ctx, r->x0, r->y1 - w, r->x1, r->y1, s->stroke_colour);
    fill(ctx, r->x0, r->y0, r->x0 + w, r->y1, s->stroke_colour);
    fill(ctx, r->x1 - w, r->y0, r->x1, r->y1, s->stroke_colour);
  }
  return NSERROR_OK;
}
static nserror polygon(const struct redraw_context *ctx, const plot_style_t *s,
                       const int *v, unsigned n) {
  struct brain_canvas *p = ctx->priv;
  if (n < 3 || n > 1024)
    return NSERROR_BAD_SIZE;
  int x0 = v[0], x1 = v[0], y0 = v[1], y1 = v[1];
  for (unsigned i = 1; i < n; i++) {
    x0 = min(x0, v[2 * i]);
    x1 = max(x1, v[2 * i]);
    y0 = min(y0, v[2 * i + 1]);
    y1 = max(y1, v[2 * i + 1]);
  }
  for (int y = max(y0, p->y0); y < min(y1, p->y1); y++)
    for (int x = max(x0, p->x0); x < min(x1, p->x1); x++) {
      int winding = 0;
      for (unsigned i = 0, j = n - 1; i < n; j = i++) {
        int ax = v[2 * j], ay = v[2 * j + 1], bx = v[2 * i], by = v[2 * i + 1];
        int64_t cross =
            (int64_t)(bx - ax) * (y - ay) - (int64_t)(x - ax) * (by - ay);
        if (ay <= y && by > y && cross > 0)
          winding++;
        if (ay > y && by <= y && cross < 0)
          winding--;
      }
      if (winding)
        brain_pixel(ctx, x, y, s->fill_colour);
    }
  return NSERROR_OK;
}
static nserror disc(const struct redraw_context *ctx, const plot_style_t *s,
                    int cx, int cy, int radius) {
  struct brain_canvas *p = ctx->priv;
  if (radius < 0 || radius > 32768)
    return NSERROR_BAD_SIZE;
  int inner = max(0, radius - max(1, plot_style_fixed_to_int(s->stroke_width)));
  for (int y = max(cy - radius, p->y0); y < min(cy + radius + 1, p->y1); y++)
    for (int x = max(cx - radius, p->x0); x < min(cx + radius + 1, p->x1);
         x++) {
      int64_t d = (int64_t)(x - cx) * (x - cx) + (int64_t)(y - cy) * (y - cy);
      if (d <= (int64_t)radius * radius) {
        if (s->stroke_type != PLOT_OP_TYPE_NONE && d >= (int64_t)inner * inner)
          brain_pixel(ctx, x, y, s->stroke_colour);
        else if (s->fill_type != PLOT_OP_TYPE_NONE)
          brain_pixel(ctx, x, y, s->fill_colour);
      }
    }
  return NSERROR_OK;
}
static nserror arc(const struct redraw_context *ctx, const plot_style_t *s,
                   int x, int y, int r, int a, int b) {
  if (r < 0 || r > 32768)
    return NSERROR_BAD_SIZE;
  while (b < a)
    b += 360;
  b = min(b, a + 360);
  for (int angle = a; angle < b; angle++) {
    double t = angle * 3.141592653589793 / 180.0,
           u = (angle + 1) * 3.141592653589793 / 180.0;
    struct rect segment = {x + (int)(r * cos(t)), y - (int)(r * sin(t)),
                           x + (int)(r * cos(u)), y - (int)(r * sin(u))};
    line(ctx, s, &segment);
  }
  return NSERROR_OK;
}
static nserror path(const struct redraw_context *ctx, const plot_style_t *s,
                    const float *v, unsigned n, const float transform[6]) {
  // shortcut: vector paths are unsupported; add Bezier rasterisation when
  // enabling SVG.
  return NSERROR_NOT_IMPLEMENTED;
}
struct bitmap {
  int width, height;
  bool opaque;
  unsigned char data[];
};
static void *bitmap_create(int w, int h, enum gui_bitmap_flags flags) {
  if (w <= 0 || h <= 0 || (uint64_t)w * h > 1024 * 1024)
    return NULL;
  struct bitmap *b = calloc(1, sizeof(*b) + (size_t)w * h * 4);
  if (b) {
    b->width = w;
    b->height = h;
    b->opaque = (flags & BITMAP_OPAQUE) != 0;
  }
  return b;
}
static void bitmap_destroy(void *b) { free(b); }
static void set_opaque(void *b, bool v) { ((struct bitmap *)b)->opaque = v; }
static bool get_opaque(void *b) { return ((struct bitmap *)b)->opaque; }
static unsigned char *get_buffer(void *b) { return ((struct bitmap *)b)->data; }
static size_t get_rowstride(void *b) { return ((struct bitmap *)b)->width * 4; }
static int get_width(void *b) { return ((struct bitmap *)b)->width; }
static int get_height(void *b) { return ((struct bitmap *)b)->height; }
static void modified(void *b) {}
static nserror render(struct bitmap *b, struct hlcache_handle *c) {
  return NSERROR_NOT_IMPLEMENTED;
}
static nserror bitmap(const struct redraw_context *ctx, struct bitmap *b, int x,
                      int y, int w, int h, colour bg, bitmap_flags_t flags) {
  if (w <= 0 || h <= 0)
    return NSERROR_OK;
  struct brain_canvas *p = ctx->priv;
  int x0 = (flags & BITMAPF_REPEAT_X) ? p->x0 : max(x, p->x0),
      x1 = (flags & BITMAPF_REPEAT_X) ? p->x1 : min(x + w, p->x1);
  int y0 = (flags & BITMAPF_REPEAT_Y) ? p->y0 : max(y, p->y0),
      y1 = (flags & BITMAPF_REPEAT_Y) ? p->y1 : min(y + h, p->y1);
  for (int dy = y0; dy < y1; dy++)
    for (int dx = x0; dx < x1; dx++) {
      int sx = (int)((((int64_t)dx - x) % w + w) % w * b->width / w),
          sy = (int)((((int64_t)dy - y) % h + h) % h * b->height / h);
      const unsigned char *c = b->data + ((size_t)sy * b->width + sx) * 4;
      unsigned a = b->opaque ? 255 : c[3];
      if (!a)
        continue;
      uint16_t old = p->pixels[dy * BRAIN_WIDTH + dx];
      unsigned red = (c[0] * a + ((old >> 11) * 255 / 31) * (255 - a)) / 255;
      unsigned green =
          (c[1] * a + (((old >> 5) & 63) * 255 / 63) * (255 - a)) / 255;
      unsigned blue = (c[2] * a + ((old & 31) * 255 / 31) * (255 - a)) / 255;
      brain_pixel(ctx, dx, dy, red | (green << 8) | (blue << 16));
    }
  return NSERROR_OK;
}
struct gui_bitmap_table brain_bitmap = {.create = bitmap_create,
                                        .destroy = bitmap_destroy,
                                        .set_opaque = set_opaque,
                                        .get_opaque = get_opaque,
                                        .get_buffer = get_buffer,
                                        .get_rowstride = get_rowstride,
                                        .get_width = get_width,
                                        .get_height = get_height,
                                        .modified = modified,
                                        .render = render};
const struct plotter_table brain_plotters = {.clip = clip,
                                             .arc = arc,
                                             .disc = disc,
                                             .line = line,
                                             .rectangle = rectangle,
                                             .polygon = polygon,
                                             .path = path,
                                             .bitmap = bitmap,
                                             .text = brain_text};
