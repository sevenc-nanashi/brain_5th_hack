// SPDX-License-Identifier: GPL-2.0-only
#ifndef BRAIN_NETSURF_H
#define BRAIN_NETSURF_H
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include "utils/errors.h"
#include "netsurf/bitmap.h"
#include "netsurf/layout.h"
#include "netsurf/plotters.h"
#define BRAIN_WIDTH 854
#define BRAIN_HEIGHT 448
extern struct gui_layout_table brain_layout;
extern struct gui_bitmap_table brain_bitmap;
extern const struct plotter_table brain_plotters;
struct brain_canvas {
  uint16_t *pixels;
  int x0, y0, x1, y1;
};
void brain_pixel(const struct redraw_context *ctx, int x, int y,
                 uint32_t colour);
nserror brain_text(const struct redraw_context *, const plot_font_style_t *,
                   int, int, const char *, size_t);
nserror brain_fetch_register(void);
int brain_init(const char *url);
int brain_tick(uint64_t milliseconds);
int brain_draw(uint16_t *pixels);
void brain_click(int x, int y);
void brain_scroll(int x, int y, int delta);
void brain_back(void);
void brain_stop(void);
const char *brain_status(void);
uint32_t brain_request(const char **url);
int brain_response(uint32_t id, int kind, int status, const uint8_t *data,
                   size_t length);
#endif
