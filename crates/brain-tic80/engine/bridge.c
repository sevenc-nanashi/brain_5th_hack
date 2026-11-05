#include "core/core.h"
#include "cart.h"
#include <stdlib.h>
#include <string.h>

static tic_mem *memory;
static tic80_input input;
static tic_tick_data tick_data;
static char error_text[512];
static int result;
static unsigned budget;
volatile int brain_tic_trace_budget;
extern tic_script_config brain_lua_LuaSyntaxConfig, LuaSyntaxConfig;
static unsigned sample_offset;
static uint64_t frames;

static void error(void *unused, const char *text) {
    if (!result) {
        strncpy(error_text, text, sizeof(error_text) - 1);
        error_text[sizeof(error_text) - 1] = 0;
        result = -1;
    }
}
static void trace(void *unused, const char *text, u8 color) {}
static void exit_cart(void *unused) { result = 1; }
static uint64_t counter(void *unused) { return frames; }
static uint64_t frequency(void *unused) { return 60; }

bool brain_tic_budget(void) {
    if (budget == 0 || brain_tic_trace_budget == 0) return false;
    --budget;
    return true;
}

int brain_tic_backend(bool jit) {
    if (memory) return -1;
    Languages[0] = jit ? &LuaSyntaxConfig : &brain_lua_LuaSyntaxConfig;
    return 0;
}

// The upstream chunk loader assumes complete input and NUL-terminated code.
static bool valid_cart(const u8 *data, size_t length) {
    if (length < 4 || length > 1024 * 1024) return false;
    size_t position = 0, code_size = 0;
    unsigned code_banks = 0;
    while (position < length) {
        if (length - position < 4) return false;
        unsigned type = data[position] & 31, bank = data[position] >> 5;
        size_t size = data[position + 1] | (data[position + 2] << 8);
        position += 4;
        if (type == 5 && size == 0) size = TIC_BANK_SIZE;
        if (size > length - position) return false;
        switch (type) {
        case 5:
            if (code_banks & (1u << bank)) return false;
            code_banks |= 1u << bank;
            code_size += size;
            break;
        case 17:
            if (size != 0) return false;
            break;
        case 20:
            if (size != 1 || (data[position] != 0 && data[position] != 10)) return false;
            break;
        case 1: case 2: case 4: case 6: case 9: case 10:
        case 12: case 14: case 15: case 18:
            if (size == 0) return false;
            break;
        default:
            return false;
        }
        position += size;
    }
    return code_size > 0 && code_size < TIC_CODE_SIZE;
}

int brain_tic_init(const u8 *cart, size_t length) {
    result = 0;
    error_text[0] = 0;
    if (memory) { error(NULL, "Player already running"); return result; }
    if (!valid_cart(cart, length)) {
        error(NULL, "Invalid or unsupported Lua .tic cartridge");
        return result;
    }
    memory = tic_core_create(44100, TIC80_PIXEL_COLOR_RGBA8888);
    tic_cart_load(&memory->cart, cart, (s32)length);
    if ((u8)memory->cart.code.data[0] == 0x1b) error(NULL, "Compiled Lua bytecode is unsupported");
    char *script = tic_tool_metatag(memory->cart.code.data, "script", "--");
    if (script && strcmp(script, "lua") != 0) error(NULL, "Only Lua cartridges are supported");
    free(script);
    tic_api_reset(memory);
    memset(&input, 0, sizeof input);
    memset(memory->product.screen, 0, TIC80_FULLWIDTH * TIC80_FULLHEIGHT * sizeof(u32));
    tick_data = (tic_tick_data){.trace = trace, .error = error, .exit = exit_cart,
        .counter = counter, .freq = frequency};
    sample_offset = memory->product.samples.count;
    frames = 0;
    return result;
}

int brain_tic_step(void) {
    if (!memory || result) return result ? result : -1;
    budget = 2000; // shortcut: 20M instructions allow Quest's map initialization; tune after device profiling.
    brain_tic_trace_budget = 2000000;
    memory->ram->input = input;
    tic_core_tick_start(memory);
    tic_core_tick(memory, &tick_data);
    if (result) return result;
    tic_core_tick_end(memory);
    tic_core_blit(memory);
    if (!result) ++frames;
    return result;
}

int brain_tic_fill(s16 *output, size_t count) {
    if (!memory || result) return result ? result : -1;
    while (count) {
        if (sample_offset == (unsigned)memory->product.samples.count) {
            if (brain_tic_step()) return result;
            tic_core_synth_sound(memory);
            sample_offset = 0;
        }
        size_t n = MIN(count, memory->product.samples.count - sample_offset);
        memcpy(output, memory->product.samples.buffer + sample_offset, n * sizeof(s16));
        sample_offset += n;
        output += n;
        count -= n;
    }
    return result;
}

void brain_tic_input(u8 buttons, u32 keys, u8 x, u8 y, bool touch) {
    input.gamepads.first.data = buttons;
    input.keyboard.data = keys;
    input.mouse.x = x;
    input.mouse.y = y;
    input.mouse.left = touch;
}
const u32 *brain_tic_screen(void) { return memory ? memory->product.screen : NULL; }
const char *brain_tic_error(void) { return error_text; }
int brain_tic_result(void) { return result; }
uint64_t brain_tic_frames(void) { return frames; }
u32 *brain_tic_pmem(void) { return memory ? memory->ram->persistent.data : NULL; }
void brain_tic_close(void) {
    if (memory) tic_core_close(memory);
    memory = NULL;
}
