#include <errno.h>
#include <reent.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <sys/time.h>
#include <time.h>

extern void *tic_alloc(size_t);
extern void tic_free(void *);
extern void *tic_realloc(void *, size_t);
extern void tic_panic(void) __attribute__((noreturn));

int _gettimeofday(struct timeval *tv, void *tz) {
    // V690_H1 control 0x30013 reads the local RTC; see docs/clock.md.
    struct {
        uint16_t year, month, weekday, day, hour, minute, second, millisecond;
    } rtc;
    _Static_assert(sizeof(rtc) == 16, "RTC ABI");
    int (*control)(uint32_t, const void *, uint32_t, void *, uint32_t, uint32_t *) =
        (void *)0x600063c0;
    if (control(0x30013, NULL, 0, &rtc, sizeof(rtc), NULL) != 1) {
        errno = EIO;
        return -1;
    }
    struct tm date = {
        .tm_year = rtc.year - 1900,
        .tm_mon = rtc.month - 1,
        .tm_mday = rtc.day,
        .tm_hour = rtc.hour,
        .tm_min = rtc.minute,
        .tm_sec = rtc.second,
        .tm_isdst = 0,
    };
    // Newlib's default zone is UTC. The Brain clock is set to Japanese local time.
    time_t seconds = mktime(&date);
    if (seconds == (time_t)-1) {
        errno = EOVERFLOW;
        return -1;
    }
    tv->tv_sec = seconds - 9 * 60 * 60;
    tv->tv_usec = rtc.millisecond * 1000;
    return 0;
}

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
