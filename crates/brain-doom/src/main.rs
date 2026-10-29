#![no_std]
#![no_main]
use brain_doom::{doom_key, engine, help, paint};
use core::{arch::global_asm, ffi::c_void, panic::PanicInfo, ptr};
use reflex::{
    app::{self, AppContext},
    display,
    peg::{self, raw},
    runtime,
};

mod audio;

// Copy .data to PCConnect's RAM base and clear .bss before entering Rust/C.
global_asm!(include_str!("startup.S"), options(raw));

const TIMER: u32 = 0x444f;
#[repr(C, align(16))]
struct Stack([u8; 128 * 1024]);
static mut STACK: Stack = Stack([0; 128 * 1024]);
static mut VTABLE: [usize; 80] = [0; 80];
static mut ORIGINAL_VTABLE: *const usize = ptr::null();
static mut WINDOW: *mut c_void = ptr::null_mut();
static mut CONTEXT: AppContext = AppContext {
    reserved: ptr::null(),
    exec_path: ptr::null(),
    presentation: ptr::null_mut(),
    reserved2: ptr::null(),
};
static mut PREVIOUS_FOCUS: *mut c_void = ptr::null_mut();
static mut BASE_MESSAGE: Option<peg::MessageFn> = None;
static mut READY: bool = false;
static mut STARTED: bool = false;
static mut START_US: u64 = 0;
static mut LAST_FRAME_US: u64 = 0;
static mut FPS_STARTED_US: u64 = 0;
static mut FPS_FRAMES: u32 = 0;
static mut FPS: u32 = 0;
static mut FAILED: bool = false;
static mut HELP: bool = false;
static mut HELP_STARTED_US: u64 = 0;
static mut FUNCTION_KEYS: [i32; 6] = [0; 6];
static mut VOLUME_KEYS: u8 = 0;
static mut FRAME: [u16; display::WIDTH * display::HEIGHT] = [0; display::WIDTH * display::HEIGHT];

unsafe extern "C" {
    fn on_engine_stack(call: unsafe extern "C" fn() -> i32, top: *mut u8) -> i32;
}
unsafe fn run(call: unsafe extern "C" fn() -> i32) -> i32 {
    unsafe {
        on_engine_stack(
            call,
            ptr::addr_of_mut!(STACK)
                .cast::<u8>()
                .add(size_of::<Stack>()),
        )
    }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn doom_main(context: *const AppContext) {
    unsafe {
        CONTEXT = context.read();
        let window = runtime::alloc(0x108);
        if window.is_null() {
            app::exit(ptr::addr_of!(CONTEXT));
            return;
        }
        ptr::write_bytes(window.cast::<u8>(), 0, 0x108);
        raw::dec_window_ctor(window, 0x10);
        raw::rect_set(window.cast::<u8>().add(0x0c).cast(), 0, 0, 853, 479);
        raw::adjust_client(window);
        ORIGINAL_VTABLE = *window.cast::<*const usize>();
        ptr::copy_nonoverlapping(ORIGINAL_VTABLE, ptr::addr_of_mut!(VTABLE).cast(), 80);
        BASE_MESSAGE = Some(core::mem::transmute::<usize, peg::MessageFn>(
            *ORIGINAL_VTABLE.add(peg::VFUNC_MESSAGE),
        ));
        VTABLE[peg::VFUNC_MESSAGE] = message as *const () as usize;
        VTABLE[peg::VFUNC_DRAW] = draw as *const () as usize;
        *window.cast::<*const usize>() = ptr::addr_of!(VTABLE).cast();
        WINDOW = window;
        let pm = CONTEXT.presentation;
        PREVIOUS_FOCUS = pm.cast::<u8>().add(0x18c).cast::<*mut c_void>().read();
        let pm_vtable = *pm.cast::<*const usize>();
        let add: unsafe extern "C" fn(*mut c_void, *mut c_void, i32) =
            core::mem::transmute(*pm_vtable.add(3));
        add(pm, window, 1);
        raw::set_current(pm, window);
        START_US = runtime::micros();
        READY = true;
        raw::set_timer(window, TIMER, 1, 1);
    }
}

unsafe fn close() {
    unsafe {
        // Keep the image and PCM buffers alive if the driver has not stopped.
        if !audio::shutdown() {
            FAILED = true;
            failure_screen();
            return;
        }
        READY = false;
        raw::kill_timer(WINDOW, TIMER);
        raw::set_current(CONTEXT.presentation, PREVIOUS_FOCUS);
        // Any queued destruction must dispatch through ROM, not our unloaded code.
        *WINDOW.cast::<*const usize>() = ORIGINAL_VTABLE;
        raw::destroy(WINDOW, WINDOW);
        WINDOW = ptr::null_mut();
        engine::shutdown();
        app::exit(ptr::addr_of!(CONTEXT));
    }
}

unsafe extern "C" fn message(window: *mut c_void, message: *const peg::Message) -> i32 {
    unsafe {
        let msg = &*message;
        if !READY {
            return BASE_MESSAGE.unwrap()(window, message);
        }
        match msg.kind {
            peg::PM_DESTROY => {
                close();
                return 0;
            }
            peg::PM_KEY | peg::PM_KEY_DOWN | peg::PM_KEY_UP => {
                if matches!(msg.param, peg::key::PAGE_UP | peg::key::PAGE_DOWN) {
                    let bit = if msg.param == peg::key::PAGE_UP { 1 } else { 2 };
                    if msg.kind == peg::PM_KEY_UP {
                        VOLUME_KEYS &= !bit;
                    } else if VOLUME_KEYS & bit == 0 {
                        VOLUME_KEYS |= bit;
                        let level = reflex::audio::volume();
                        let next = if bit == 1 {
                            (level + 1).min(9)
                        } else {
                            level.saturating_sub(1)
                        };
                        if !reflex::audio::set_volume(next) {
                            FAILED = true;
                            audio::shutdown();
                            failure_screen();
                        }
                    }
                    return 0;
                }
                let pressed = msg.kind != peg::PM_KEY_UP;
                if pressed
                    && FAILED
                    && (msg.param == b'q' as u16
                        || msg.param == b'Q' as u16
                        || matches!(
                            msg.param,
                            peg::key::HOME | peg::key::SELECT_FROM_LIST | peg::key::ESCAPE
                        ))
                {
                    close();
                    return 0;
                }
                if STARTED
                    && !FAILED
                    && (peg::key::HOME..=peg::key::SELECT_FROM_LIST).contains(&msg.param)
                {
                    let index = (msg.param - peg::key::HOME) as usize;
                    if pressed {
                        if FUNCTION_KEYS[index] == 0 {
                            let shifted = msg.data.ext_params[0] as u32 & peg::KMOD_SHIFT != 0;
                            let number = index + if shifted { 6 } else { 0 };
                            let key = [
                                0xbb, 0xbc, 0xbd, 0xbe, 0xbf, 0xc0, 0xc1, 0xc2, 0xc3, 0xc4, 0xd7,
                                0xd8,
                            ][number];
                            FUNCTION_KEYS[index] = key;
                            if number == 11 {
                                toggle_help();
                            } else if !HELP && !matches!(number, 1 | 2 | 5 | 8) {
                                engine::key(key, true);
                            }
                        }
                    } else {
                        let key = FUNCTION_KEYS[index];
                        FUNCTION_KEYS[index] = 0;
                        if key != 0 && key != 0xd8 {
                            engine::key(key, false);
                        }
                    }
                    return 0;
                }
                if HELP && pressed {
                    if matches!(msg.param, peg::key::BACKSPACE | peg::key::ESCAPE) {
                        toggle_help();
                    }
                    return 0;
                }
                if STARTED
                    && !FAILED
                    && let Some(key) = doom_key(msg.param)
                {
                    engine::key(key, pressed);
                }
                return 0;
            }
            reflex::audio::PM_AUDIO => {
                if !FAILED
                    && msg.param == reflex::audio::BUFFER_CONSUMED
                    && !audio::refill(msg.data.ext_params[0])
                {
                    FAILED = true;
                    audio::shutdown();
                    failure_screen();
                }
                return 0;
            }
            peg::PM_TIMER if msg.param == TIMER as u16 => {
                if FAILED || HELP {
                    return 0;
                }
                let elapsed = runtime::micros().wrapping_sub(START_US);
                if STARTED && elapsed.wrapping_sub(LAST_FRAME_US) < 1_000_000 / 35 {
                    return 0;
                }
                engine::set_time(elapsed);
                let result = if STARTED {
                    LAST_FRAME_US = elapsed;
                    run(engine::step)
                } else {
                    STARTED = true;
                    let result = run(engine::init);
                    let result = if result == 0 && !audio::init(window) {
                        -1
                    } else {
                        result
                    };
                    // Loading is not game time; avoid simulating the entire startup delay.
                    START_US = runtime::micros();
                    LAST_FRAME_US = 0;
                    result
                };
                if result == 1 {
                    close();
                    return 0;
                }
                if result != 0 {
                    FAILED = true;
                    audio::shutdown();
                    failure_screen();
                    return 0;
                }
                let now = runtime::micros();
                if FPS_STARTED_US == 0 {
                    FPS_STARTED_US = now;
                } else {
                    FPS_FRAMES += 1;
                    let duration = now.wrapping_sub(FPS_STARTED_US);
                    if duration >= 1_000_000 {
                        FPS =
                            ((u64::from(FPS_FRAMES) * 1_000_000 + duration / 2) / duration) as u32;
                        FPS_FRAMES = 0;
                        FPS_STARTED_US = now;
                    }
                }
                render_frame();
                draw(window, window.cast::<u8>().add(0x0c).cast());
                return 0;
            }
            _ => {}
        }
        BASE_MESSAGE.unwrap()(window, message)
    }
}

unsafe fn render_frame() {
    unsafe {
        let frame = core::slice::from_raw_parts_mut(
            ptr::addr_of_mut!(FRAME).cast::<u16>(),
            display::WIDTH * display::HEIGHT,
        );
        paint(engine::frame(), |x, y, color| {
            frame[y * display::WIDTH + x] = color
        });
        if HELP {
            help::paint(frame);
            let (mix_ms, state, submits) = audio::diagnostics();
            help::audio_status(frame, mix_ms, state, submits);
        } else {
            help::performance(frame, FPS, audio::max_fill_ms());
        }
    }
}

unsafe fn toggle_help() {
    unsafe {
        HELP = !HELP;
        if HELP {
            HELP_STARTED_US = runtime::micros();
        } else {
            START_US = START_US.wrapping_add(runtime::micros().wrapping_sub(HELP_STARTED_US));
            FPS_STARTED_US = 0;
            FPS_FRAMES = 0;
        }
        render_frame();
        draw(WINDOW, WINDOW.cast::<u8>().add(0x0c).cast());
    }
}

unsafe extern "C" fn draw(window: *mut c_void, invalid: *const peg::Rect) {
    unsafe {
        let mut bitmap = peg::Bitmap {
            bpp: 16,
            flags: 0,
            width: display::WIDTH as u16,
            height: display::HEIGHT as u16,
            reserved: 0,
            refcount: 1,
            data: ptr::addr_of_mut!(FRAME).cast(),
        };
        raw::blit(window, invalid, &mut bitmap);
    }
}

unsafe fn failure_screen() {
    unsafe {
        for y in 0..display::HEIGHT {
            for x in 0..display::WIDTH {
                ptr::addr_of_mut!(FRAME)
                    .cast::<u16>()
                    .add(y * display::WIDTH + x)
                    .write(0x4000);
            }
        }
        // A small fixed message also works when engine initialization failed.
        for (i, c) in b"DOOM ERROR  Q EXIT".iter().enumerate() {
            let rows: [u8; 7] = match c {
                b'D' => [30, 17, 17, 17, 17, 17, 30],
                b'O' => [14, 17, 17, 17, 17, 17, 14],
                b'M' => [17, 27, 21, 21, 17, 17, 17],
                b'E' => [31, 16, 16, 30, 16, 16, 31],
                b'R' => [30, 17, 17, 30, 20, 18, 17],
                b'Q' => [14, 17, 17, 17, 21, 18, 13],
                b'X' => [17, 17, 10, 4, 10, 17, 17],
                b'I' => [14, 4, 4, 4, 4, 4, 14],
                b'T' => [31, 4, 4, 4, 4, 4, 4],
                _ => [0; 7],
            };
            for (y, row) in rows.iter().enumerate() {
                for x in 0..5 {
                    if row & (1 << (4 - x)) != 0 {
                        for dy in 0..4 {
                            for dx in 0..4 {
                                let px = 220 + i * 24 + x * 4 + dx;
                                let py = 220 + y * 4 + dy;
                                ptr::addr_of_mut!(FRAME)
                                    .cast::<u16>()
                                    .add(py * display::WIDTH + px)
                                    .write(0xffff);
                            }
                        }
                    }
                }
            }
        }
        draw(WINDOW, WINDOW.cast::<u8>().add(0x0c).cast());
    }
}

#[panic_handler]
fn panic(_: &PanicInfo<'_>) -> ! {
    loop {
        core::hint::spin_loop();
    }
}
