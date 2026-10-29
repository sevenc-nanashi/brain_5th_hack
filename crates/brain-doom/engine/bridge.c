// PureDOOM is fetched separately; its original license remains in the header.
#define DOOM_IMPLEMENTATION
#include "PureDOOM.h"

_Static_assert((char)-1 < 0, "Doom movement requires signed char; compile with -fsigned-char");

#ifdef BRAIN_ARM
static unsigned int escape[10];
extern int brain_setjmp(unsigned int *) __attribute__((returns_twice));
extern void brain_longjmp(unsigned int *) __attribute__((noreturn));
#define CHECKPOINT() brain_setjmp(escape)
#define ESCAPE() brain_longjmp(escape)
#else
#include <setjmp.h>
static jmp_buf escape;
#define CHECKPOINT() setjmp(escape)
#define ESCAPE() longjmp(escape, 1)
#endif

extern void brain_print(const char *);
extern void *brain_alloc(int);
extern void brain_free(void *);
extern void *brain_open(const char *, const char *);
extern void brain_close(void *);
extern int brain_read(void *, void *, int);
extern int brain_seek(void *, int, doom_seek_t);
extern int brain_tell(void *);
extern int brain_eof(void *);
extern void brain_time(int *, int *);
static int exit_status;

// The nonlocal exit stays within C; Rust callbacks always return first.
static void stop(int code) {
    exit_status = code == 0 ? 1 : -1;
    ESCAPE();
}
static void *allocate(int size) {
    void *ptr = brain_alloc(size);
    if (!ptr) {
        brain_print("Out of memory");
        stop(-1);
    }
    return ptr;
}
static int read_only(void *handle, const void *data, int count) {
    (void)handle; (void)data; (void)count;
    return -1;
}
static char *environment(const char *name) {
    (void)name;
    return ".";
}

int brain_doom_init(void) {
    if (CHECKPOINT()) return exit_status;
    doom_set_print(brain_print);
    doom_set_malloc(allocate, brain_free);
    doom_set_file_io(brain_open, brain_close, brain_read, read_only,
                     brain_seek, brain_tell, brain_eof);
    doom_set_gettime(brain_time);
    doom_set_exit(stop);
    doom_set_getenv(environment);
    doom_set_default_int("key_fire", DOOM_KEY_Z);
    doom_set_default_int("key_use", DOOM_KEY_SPACE);
    doom_set_default_int("key_strafeleft", DOOM_KEY_A);
    doom_set_default_int("key_straferight", DOOM_KEY_D);
    doom_set_default_int("key_speed", DOOM_KEY_X);
    static char *args[] = {"doom", "-nomusic"};
    doom_init(2, args, DOOM_FLAG_HIDE_MOUSE_OPTIONS | DOOM_FLAG_HIDE_MUSIC_OPTIONS);
    singletics = 1; // The host drives 35 Hz; never wait for network tics inside PEG.
    return 0;
}
int brain_doom_step(void) {
    if (CHECKPOINT()) return exit_status;
    doom_update();
    return 0;
}
const unsigned char *brain_doom_frame(void) { return doom_get_framebuffer(3); }
_Static_assert(SAMPLECOUNT == 512 && DOOM_SAMPLERATE == 11025, "PCM adapter format changed");
int brain_doom_mix(void) {
    if (CHECKPOINT()) return exit_status;
    doom_get_sound_buffer();
    return 0;
}
const short *brain_doom_sound(void) { return mixbuffer; }
void brain_doom_key(int code, int pressed) {
    if (pressed) doom_key_down((doom_key_t)code);
    else doom_key_up((doom_key_t)code);
}
