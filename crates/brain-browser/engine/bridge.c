// SPDX-License-Identifier: GPL-2.0-only
#include "brain.h"
#include "desktop/browser_history.h"
#include "netsurf/browser_window.h"
#include "netsurf/fetch.h"
#include "netsurf/misc.h"
#include "netsurf/netsurf.h"
#include "netsurf/types.h"
#include "netsurf/window.h"
#include "resources.h"
#include "utils/nsoption.h"
#include "utils/nsurl.h"
#include "utils/utf8.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

struct gui_window {
  struct browser_window *bw;
  int x, y;
};
static struct gui_window *window;
static uint64_t now;
static bool dirty;
static char status_text[256];
struct timer {
  struct timer *next;
  uint64_t due;
  void (*callback)(void *);
  void *context;
};
static struct timer *timers;

static nserror schedule(int delay, void (*callback)(void *), void *context) {
  struct timer **p = &timers;
  while (*p) {
    struct timer *t = *p;
    if (t->callback == callback && t->context == context) {
      *p = t->next;
      free(t);
      break;
    }
    p = &t->next;
  }
  if (delay < 0)
    return NSERROR_OK;
  struct timer *t = malloc(sizeof(*t));
  if (!t)
    return NSERROR_NOMEM;
  *t = (struct timer){.next = timers,
                      .due = now + delay,
                      .callback = callback,
                      .context = context};
  timers = t;
  return NSERROR_OK;
}
static struct gui_window *create(struct browser_window *bw,
                                 struct gui_window *existing,
                                 gui_window_create_flags flags) {
  if (window)
    return NULL;
  window = calloc(1, sizeof(*window));
  if (window)
    window->bw = bw;
  return window;
}
static void destroy(struct gui_window *w) {
  free(w);
  window = NULL;
}
static nserror invalidate(struct gui_window *w, const struct rect *r) {
  dirty = true;
  return NSERROR_OK;
}
static bool get_scroll(struct gui_window *w, int *x, int *y) {
  *x = w->x;
  *y = w->y;
  return true;
}
static nserror set_scroll(struct gui_window *w, const struct rect *r) {
  int width, height;
  nserror err = browser_window_get_extents(w->bw, true, &width, &height);
  if (err != NSERROR_OK)
    return err;
  int xmax = width > BRAIN_WIDTH ? width - BRAIN_WIDTH : 0;
  int ymax = height > BRAIN_HEIGHT ? height - BRAIN_HEIGHT : 0;
  w->x = r->x0 < 0 ? 0 : r->x0 > xmax ? xmax : r->x0;
  w->y = r->y0 < 0 ? 0 : r->y0 > ymax ? ymax : r->y0;
  dirty = true;
  return NSERROR_OK;
}
static nserror dimensions(struct gui_window *w, int *width, int *height) {
  *width = BRAIN_WIDTH;
  *height = BRAIN_HEIGHT;
  return NSERROR_OK;
}
static nserror event(struct gui_window *w, enum gui_window_event event) {
  dirty = true;
  return NSERROR_OK;
}
static void set_status(struct gui_window *w, const char *s) {
  size_t length = strlen(s);
  if (length >= sizeof(status_text))
    length = utf8_prev(s, sizeof(status_text));
  memcpy(status_text, s, length);
  status_text[length] = 0;
  dirty = true;
}
static const char *filetype(const char *path) {
  const char *extension = strrchr(path, '.');
  if (extension && !strcmp(extension, ".css"))
    return "text/css";
  if (extension && !strcmp(extension, ".html"))
    return "text/html";
  return "application/octet-stream";
}
static nserror resource(const char *path, const uint8_t **data,
                        size_t *length) {
  for (size_t i = 0; i < sizeof(resources) / sizeof(resources[0]); i++)
    if (!strcmp(path, resources[i].name)) {
      *data = resources[i].data;
      *length = resources[i].length;
      return NSERROR_OK;
    }
  return NSERROR_NOT_FOUND;
}
static nserror release_resource(const uint8_t *data) { return NSERROR_OK; }
static struct gui_misc_table misc = {.schedule = schedule};
static struct gui_window_table windows = {.create = create,
                                          .destroy = destroy,
                                          .invalidate = invalidate,
                                          .get_scroll = get_scroll,
                                          .set_scroll = set_scroll,
                                          .get_dimensions = dimensions,
                                          .event = event,
                                          .set_status = set_status};
static struct gui_fetch_table fetch = {.filetype = filetype,
                                       .get_resource_data = resource,
                                       .release_resource_data =
                                           release_resource};
static struct netsurf_table gui = {.misc = &misc,
                                   .window = &windows,
                                   .fetch = &fetch,
                                   .bitmap = &brain_bitmap,
                                   .layout = &brain_layout};

int brain_init(const char *url) {
  nserror err = netsurf_register(&gui);
  if (err)
    return err;
  err = nsoption_init(NULL, &nsoptions, &nsoptions_default);
  if (err)
    return err;
  nsoption_set_int(max_fetchers, 1);
  nsoption_set_int(max_fetchers_per_host, 1);
  nsoption_set_int(memory_cache_size, 2 * 1024 * 1024);
  nsoption_set_bool(enable_javascript, false);
  nsoption_set_bool(animate_images, false);
  nsoption_set_bool(incremental_reflow, true);
  nsoption_set_int(font_size, 120);
  const bitmap_fmt_t format = {.layout = BITMAP_LAYOUT_R8G8B8A8, .pma = false};
  bitmap_set_format(&format);
  err = netsurf_init(NULL);
  if (err)
    return err;
  nsurl *address;
  err = nsurl_create(url, &address);
  if (err)
    return err;
  struct browser_window *bw;
  err = browser_window_create(BW_CREATE_HISTORY, address, NULL, NULL, &bw);
  nsurl_unref(address);
  return err;
}
int brain_tick(uint64_t milliseconds) {
  now = milliseconds;
  // Bound work per PEG callback; zero-delay reschedules continue on the next
  // tick.
  for (int n = 0; n < 64; n++) {
    struct timer **p = &timers;
    while (*p && (*p)->due > now)
      p = &(*p)->next;
    if (!*p)
      break;
    struct timer *t = *p;
    *p = t->next;
    void (*callback)(void *) = t->callback;
    void *context = t->context;
    free(t);
    callback(context);
  }
  return dirty;
}
int brain_draw(uint16_t *pixels) {
  for (size_t i = 0; i < BRAIN_WIDTH * BRAIN_HEIGHT; i++)
    pixels[i] = 0xffff;
  if (!window || !browser_window_redraw_ready(window->bw))
    return 0;
  struct brain_canvas canvas = {pixels, 0, 0, BRAIN_WIDTH, BRAIN_HEIGHT};
  struct redraw_context ctx = {.interactive = true,
                               .background_images = true,
                               .plot = &brain_plotters,
                               .priv = &canvas};
  struct rect clip = {0, 0, BRAIN_WIDTH, BRAIN_HEIGHT};
  dirty = false;
  return browser_window_redraw(window->bw, -window->x, -window->y, &clip, &ctx);
}
void brain_click(int x, int y) {
  if (!window)
    return;
  browser_window_mouse_click(window->bw, BROWSER_MOUSE_PRESS_1, x + window->x,
                             y + window->y);
  browser_window_mouse_click(window->bw, BROWSER_MOUSE_CLICK_1, x + window->x,
                             y + window->y);
}
void brain_scroll(int x, int y, int delta) {
  if (!window)
    return;
  if (!browser_window_scroll_at_point(window->bw, x + window->x, y + window->y,
                                      0, delta)) {
    struct rect r = {window->x, window->y + delta, window->x,
                     window->y + delta};
    set_scroll(window, &r);
  }
}
void brain_back(void) {
  if (window && browser_window_history_back_available(window->bw))
    browser_window_history_back(window->bw, false);
}
const char *brain_status(void) { return status_text; }
void brain_stop(void) {
  if (window)
    browser_window_destroy(window->bw);
  netsurf_exit();
  nsoption_finalise(nsoptions, nsoptions_default);
#ifdef BRAIN_ARM
  extern void brain_exit_callbacks(void);
  brain_exit_callbacks();
#endif
  while (timers) {
    struct timer *t = timers;
    timers = t->next;
    free(t);
  }
}
