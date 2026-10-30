// SPDX-License-Identifier: GPL-2.0-only
#include "arpa/inet.h"
#include "utils/utsname.h"
#include <dirent.h>
#include <errno.h>
#include <reent.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <sys/time.h>
#include <time.h>
extern void *browser_alloc(size_t);
extern void browser_free(void *);
extern void *browser_realloc(void *, size_t);
extern void browser_panic(void) __attribute__((noreturn));
extern uint64_t browser_micros(void);
extern uint64_t browser_wall_micros(void);
void *malloc(size_t n) { return browser_alloc(n); }
void free(void *p) { browser_free(p); }
void *realloc(void *p, size_t n) { return browser_realloc(p, n); }
void *calloc(size_t n, size_t s) {
  if (s && n > SIZE_MAX / s) {
    errno = ENOMEM;
    return NULL;
  }
  void *p = malloc(n * s);
  if (p)
    memset(p, 0, n * s);
  return p;
}
void *_malloc_r(struct _reent *r, size_t n) { return malloc(n); }
void _free_r(struct _reent *r, void *p) { free(p); }
void *_realloc_r(struct _reent *r, void *p, size_t n) { return realloc(p, n); }
void *_calloc_r(struct _reent *r, size_t n, size_t s) { return calloc(n, s); }
void _exit(int code) { browser_panic(); }
void abort(void) { browser_panic(); }
void exit(int code) { browser_panic(); }
struct exit_callback {
  struct exit_callback *next;
  void (*call)(void);
};
static struct exit_callback *exit_callbacks;
int atexit(void (*call)(void)) {
  struct exit_callback *entry = malloc(sizeof(*entry));
  if (!entry)
    return -1;
  *entry = (struct exit_callback){exit_callbacks, call};
  exit_callbacks = entry;
  return 0;
}
void brain_exit_callbacks(void) {
  while (exit_callbacks) {
    struct exit_callback *entry = exit_callbacks;
    exit_callbacks = entry->next;
    void (*call)(void) = entry->call;
    free(entry);
    call();
  }
}
void __assert_func(const char *file, int line, const char *function,
                   const char *expression) {
  browser_panic();
}
int _gettimeofday(struct timeval *tv, void *tz) {
  uint64_t us = browser_wall_micros();
  tv->tv_sec = us / 1000000;
  tv->tv_usec = us % 1000000;
  return 0;
}
// No disk, process or socket access: all page fetches go through the USB
// fetcher.
int _open(const char *p, int flags, ...) {
  errno = ENOSYS;
  return -1;
}
int _close(int fd) {
  errno = EBADF;
  return -1;
}
int _read(int fd, void *p, size_t n) {
  errno = EBADF;
  return -1;
}
int _write(int fd, const void *p, size_t n) {
  errno = EBADF;
  return -1;
}
int _lseek(int fd, int offset, int whence) {
  errno = EBADF;
  return -1;
}
int _fstat(int fd, struct stat *s) {
  errno = EBADF;
  return -1;
}
int _stat(const char *p, struct stat *s) {
  errno = ENOENT;
  return -1;
}
int _isatty(int fd) { return 0; }
int _unlink(const char *p) {
  errno = ENOSYS;
  return -1;
}
int _getpid(void) { return 1; }
int _kill(int pid, int signal) {
  errno = ENOSYS;
  return -1;
}
int mkdir(const char *path, mode_t mode) {
  errno = ENOSYS;
  return -1;
}
int rmdir(const char *path) {
  errno = ENOSYS;
  return -1;
}
int access(const char *path, int mode) {
  errno = ENOENT;
  return -1;
}
DIR *opendir(const char *p) {
  errno = ENOSYS;
  return NULL;
}
struct dirent *readdir(DIR *p) {
  errno = ENOSYS;
  return NULL;
}
int closedir(DIR *p) {
  errno = ENOSYS;
  return -1;
}
int inet_aton(const char *s, struct in_addr *a) {
  return inet_pton(AF_INET, s, a) == 1;
}
