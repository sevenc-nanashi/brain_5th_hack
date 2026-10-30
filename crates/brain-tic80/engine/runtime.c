#include <errno.h>
#include <reent.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

extern void *tic_alloc(size_t);
extern void tic_free(void *);
extern void *tic_realloc(void *, size_t);
extern void tic_panic(void) __attribute__((noreturn));

// TIC-80 has unchecked allocations; stop safely instead of dereferencing NULL.
void *malloc(size_t size) {
    void *p = tic_alloc(size);
    if (!p) tic_panic();
    return p;
}
void free(void *p) { tic_free(p); }
void *realloc(void *p, size_t size) { return tic_realloc(p, size); }
void *calloc(size_t n, size_t size) {
    if (size && n > SIZE_MAX / size) tic_panic();
    void *p = malloc(n * size);
    memset(p, 0, n * size);
    return p;
}
void *_malloc_r(struct _reent *r, size_t n) { return malloc(n); }
void _free_r(struct _reent *r, void *p) { free(p); }
void *_realloc_r(struct _reent *r, void *p, size_t n) { return realloc(p, n); }
void *_calloc_r(struct _reent *r, size_t n, size_t s) { return calloc(n, s); }
void abort(void) { tic_panic(); }
void _exit(int code) { tic_panic(); }
void __assert_func(const char *file, int line, const char *function, const char *expression) { tic_panic(); }
// The application discards ARM unwind tables and must never unwind through Rust.
void __aeabi_unwind_cpp_pr0(void) { tic_panic(); }
