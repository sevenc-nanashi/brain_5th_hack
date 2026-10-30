// SPDX-License-Identifier: GPL-2.0-only
#include "brain.h"
#include "utils/nsurl.h"
#include "content/fetch.h"
#include "content/fetchers.h"
#include <stdlib.h>
#include <string.h>

struct request {
  struct request *next;
  struct fetch *parent;
  nsurl *url;
  uint32_t id;
  bool active, aborted, locked, post;
};
static struct request *requests;
static uint32_t serial;
static bool initialise(lwc_string *scheme) { return true; }
static bool acceptable(const nsurl *url) { return true; }
static void *setup(struct fetch *parent, nsurl *url, bool only2xx,
                   bool downgrade, const char *post,
                   const struct fetch_multipart_data *multipart,
                   const char **headers) {
  struct request *r = calloc(1, sizeof(*r));
  if (!r)
    return NULL;
  r->parent = parent;
  r->url = nsurl_ref(url);
  r->post = post || multipart;
  r->id = ++serial;
  r->next = requests;
  requests = r;
  return r;
}
static bool start(void *p) {
  ((struct request *)p)->active = true;
  return true;
}
static void cancel(void *p) { ((struct request *)p)->aborted = true; }
static void release(void *p) {
  struct request *r = p, **link = &requests;
  while (*link != r)
    link = &(*link)->next;
  *link = r->next;
  nsurl_unref(r->url);
  free(r);
}
static void finish(struct request *r) {
  fetch_remove_from_queues(r->parent);
  fetch_free(r->parent);
}
static void poll(lwc_string *scheme) {
  struct request *r = requests;
  while (r) {
    struct request *next = r->next;
    if (r->active && !r->locked) {
      if (r->aborted)
        finish(r);
      else if (r->post) {
        fetch_msg msg = {.type = FETCH_ERROR,
                         .data.error = "POST is not supported"};
        r->locked = true;
        fetch_send_callback(&msg, r->parent);
        finish(r);
      }
    }
    r = next;
  }
}
static void finalise(lwc_string *scheme) {}
nserror brain_fetch_register(void) {
  const struct fetcher_operation_table ops = {.initialise = initialise,
                                              .acceptable = acceptable,
                                              .setup = setup,
                                              .start = start,
                                              .abort = cancel,
                                              .free = release,
                                              .poll = poll,
                                              .finalise = finalise};
  const char *names[] = {"http", "https"};
  for (size_t i = 0; i < 2; i++) {
    lwc_string *scheme;
    if (lwc_intern_string(names[i], strlen(names[i]), &scheme) != lwc_error_ok)
      return NSERROR_NOMEM;
    nserror error = fetcher_add(scheme, &ops);
    if (error != NSERROR_OK)
      return error;
  }
  return NSERROR_OK;
}
uint32_t brain_request(const char **url) {
  for (struct request *r = requests; r; r = r->next) {
    if (r->active && !r->aborted && !r->post) {
      *url = nsurl_access(r->url);
      return r->id;
    }
  }
  *url = NULL;
  return 0;
}
// kind: 1 header, 2 body, 3 complete, 4 error, 5 redirect. Strings need no
// terminator.
int brain_response(uint32_t id, int kind, int status, const uint8_t *data,
                   size_t length) {
  struct request *r;
  for (r = requests; r && r->id != id; r = r->next) {
  }
  if (!r || !r->active || r->aborted || r->locked)
    return 1;
  if (length > 4096 || kind < 1 || kind > 5 || status < 0 || status > 599)
    return -1;
  fetch_msg msg = {0};
  char string[4097];
  if (status)
    fetch_set_http_code(r->parent, status);
  switch (kind) {
  case 1:
  case 2:
    msg.type = kind == 1 ? FETCH_HEADER : FETCH_DATA;
    msg.data.header_or_data.buf = data;
    msg.data.header_or_data.len = length;
    break;
  case 3:
    msg.type = FETCH_FINISHED;
    break;
  case 4:
  case 5:
    memcpy(string, data, length);
    string[length] = 0;
    msg.type = kind == 4 ? FETCH_ERROR : FETCH_REDIRECT;
    msg.data.error = string;
    break;
  }
  r->locked = true;
  fetch_send_callback(&msg, r->parent);
  r->locked = false;
  if (kind >= 3 || r->aborted)
    finish(r);
  return 0;
}
