#![no_std]
#![no_main]
use brain_doom::{doom_key, engine, help, paint};
use core::{arch::global_asm, ffi::c_void, ptr};
use reflex::{
    Action, Application, Event, Window, display,
    peg::{self, raw},
    runtime,
};
mod audio;
global_asm!(include_str!("engine-stack.S"), options(raw));
const TIMER: u32 = 0x444f;
#[repr(C, align(16))]
struct Stack([u8; 128 * 1024]);
static mut STACK: Stack = Stack([0; 128 * 1024]);

#[derive(Default)]
struct Doom {
    window: *mut c_void,
    started: bool,
    start_us: u64,
    last_frame_us: u64,
    fps_started_us: u64,
    fps_frames: u32,
    fps: u32,
    failed: bool,
    help: bool,
    help_started_us: u64,
    help_draw_us: u64,
    audio_reset_held: bool,
    audio_reset_done: bool,
    function_keys: [i32; 6],
    volume_keys: u8,
    redraw: bool,
}
#[reflex::main]
fn main() -> Doom {
    Doom::default()
}
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

impl Doom {
    unsafe fn message(&mut self, msg: &peg::Message) -> Option<Action> {
        unsafe {
            let window = self.window;
            match msg.kind {
                peg::PM_KEY | peg::PM_KEY_DOWN | peg::PM_KEY_UP => {
                    if matches!(msg.param, peg::key::PAGE_UP | peg::key::PAGE_DOWN) {
                        let bit = if msg.param == peg::key::PAGE_UP { 1 } else { 2 };
                        if msg.kind == peg::PM_KEY_UP {
                            self.volume_keys &= !bit;
                        } else if self.volume_keys & bit == 0 {
                            self.volume_keys |= bit;
                            let level = reflex::audio::volume();
                            let next = if bit == 1 {
                                (level + 1).min(9)
                            } else {
                                level.saturating_sub(1)
                            };
                            if !reflex::audio::set_volume(next) {
                                self.failed = true;
                                audio::shutdown();
                                self.redraw = true;
                            }
                        }
                        return Some(Action::None);
                    }
                    let pressed = msg.kind != peg::PM_KEY_UP;
                    let reset_key = matches!(msg.param, 0x52 | 0x72);
                    if reset_key && !pressed {
                        self.audio_reset_held = false;
                    }
                    if pressed
                        && self.failed
                        && (msg.param == b'q' as u16
                            || msg.param == b'Q' as u16
                            || matches!(
                                msg.param,
                                peg::key::HOME | peg::key::SELECT_FROM_LIST | peg::key::ESCAPE
                            ))
                    {
                        return Some(Action::Exit);
                    }
                    if self.started
                        && !self.failed
                        && (peg::key::HOME..=peg::key::SELECT_FROM_LIST).contains(&msg.param)
                    {
                        let index = (msg.param - peg::key::HOME) as usize;
                        if pressed {
                            if self.function_keys[index] == 0 {
                                let shifted = msg.data.ext_params[0] as u32 & peg::KMOD_SHIFT != 0;
                                let number = index + if shifted { 6 } else { 0 };
                                let key = [
                                    0xbb, 0xbc, 0xbd, 0xbe, 0xbf, 0xc0, 0xc1, 0xc2, 0xc3, 0xc4,
                                    0xd7, 0xd8,
                                ][number];
                                self.function_keys[index] = key;
                                if number == 11 {
                                    self.toggle_help();
                                } else if !self.help && !matches!(number, 1 | 2 | 5 | 8) {
                                    engine::key(key, true);
                                }
                            }
                        } else {
                            let key = self.function_keys[index];
                            self.function_keys[index] = 0;
                            if key != 0 && key != 0xd8 {
                                engine::key(key, false);
                            }
                        }
                        return Some(Action::None);
                    }
                    if self.help && pressed {
                        if matches!(msg.param, peg::key::BACKSPACE | peg::key::ESCAPE) {
                            self.toggle_help();
                        } else if reset_key && !self.audio_reset_held {
                            self.audio_reset_held = true;
                            if !audio::reset(window) {
                                self.failed = true;
                                audio::shutdown();
                                self.redraw = true;
                                return Some(Action::None);
                            }
                            self.audio_reset_done = true;
                            self.redraw = true;
                        }
                        return Some(Action::None);
                    }
                    if self.started
                        && !self.failed
                        && let Some(key) = doom_key(msg.param)
                    {
                        engine::key(key, pressed);
                    }
                    return Some(Action::None);
                }
                reflex::audio::PM_AUDIO => {
                    if !self.failed
                        && msg.param == reflex::audio::BUFFER_CONSUMED
                        && !audio::refill(msg.data.ext_params[0])
                    {
                        self.failed = true;
                        audio::shutdown();
                        self.redraw = true;
                    }
                    return Some(Action::None);
                }
                peg::PM_TIMER if msg.param == TIMER as u16 => {
                    if self.failed {
                        return Some(Action::None);
                    }
                    if self.started && !audio::maintain(window) {
                        self.failed = true;
                        audio::shutdown();
                        self.redraw = true;
                        return Some(Action::None);
                    }
                    if self.help {
                        let now = runtime::micros();
                        if now.wrapping_sub(self.help_draw_us) >= 1_000_000 {
                            self.help_draw_us = now;
                            self.redraw = true;
                        }
                        return Some(Action::None);
                    }
                    let elapsed = runtime::micros().wrapping_sub(self.start_us);
                    if self.started && elapsed.wrapping_sub(self.last_frame_us) < 1_000_000 / 35 {
                        return Some(Action::None);
                    }
                    engine::set_time(elapsed);
                    let result = if self.started {
                        self.last_frame_us = elapsed;
                        run(engine::step)
                    } else {
                        self.started = true;
                        let result = run(engine::init);
                        let result = if result == 0 && !audio::init(window) {
                            -1
                        } else {
                            result
                        };
                        // Loading is not game time; avoid simulating the entire startup delay.
                        self.start_us = runtime::micros();
                        self.last_frame_us = 0;
                        result
                    };
                    if result == 1 {
                        return Some(Action::Exit);
                    }
                    if result != 0 {
                        self.failed = true;
                        audio::shutdown();
                        self.redraw = true;
                        return Some(Action::None);
                    }
                    let now = runtime::micros();
                    if self.fps_started_us == 0 {
                        self.fps_started_us = now;
                    } else {
                        self.fps_frames += 1;
                        let duration = now.wrapping_sub(self.fps_started_us);
                        if duration >= 1_000_000 {
                            self.fps = ((u64::from(self.fps_frames) * 1_000_000 + duration / 2)
                                / duration) as u32;
                            self.fps_frames = 0;
                            self.fps_started_us = now;
                        }
                    }
                    self.redraw = true;
                    return Some(Action::None);
                }
                _ => {}
            }
            None
        }
    }
    unsafe fn toggle_help(&mut self) {
        unsafe {
            self.help = !self.help;
            if self.help {
                self.help_started_us = runtime::micros();
                self.help_draw_us = self.help_started_us;
                self.audio_reset_done = false;
            } else {
                self.start_us = self
                    .start_us
                    .wrapping_add(runtime::micros().wrapping_sub(self.help_started_us));
                self.fps_started_us = 0;
                self.fps_frames = 0;
            }
            self.redraw = true;
        }
    }
    unsafe fn failure_screen(&self, pixels: &mut [u16]) {
        unsafe {
            for y in 0..display::HEIGHT {
                for x in 0..display::WIDTH {
                    pixels
                        .as_mut_ptr()
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
                                    pixels
                                        .as_mut_ptr()
                                        .add(py * display::WIDTH + px)
                                        .write(0xffff);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
impl Application for Doom {
    fn mounted(&mut self, window: Window) -> Action {
        self.window = window.raw;
        unsafe {
            self.start_us = runtime::micros();
            raw::set_timer(self.window, TIMER, 1, 1);
        }
        Action::Redraw
    }
    fn raw_event(&mut self, message: &peg::Message) -> Option<Action> {
        self.redraw = false;
        let action = unsafe { self.message(message) };
        if self.redraw && action != Some(Action::Exit) {
            Some(Action::Redraw)
        } else {
            action
        }
    }
    fn event(&mut self, _: Event) -> Action {
        Action::None
    }
    fn can_exit(&mut self) -> bool {
        unsafe {
            if !audio::shutdown() {
                self.failed = true;
                return false;
            }
            raw::kill_timer(self.window, TIMER);
            engine::shutdown();
        }
        true
    }
    fn draw(&self, pixels: &mut [u16]) {
        unsafe {
            if self.failed {
                self.failure_screen(pixels);
                return;
            }
            if !self.started {
                pixels.fill(0);
                return;
            }
            paint(engine::frame(), |x, y, color| {
                pixels[y * display::WIDTH + x] = color
            });
            if self.help {
                help::paint(pixels);
                let (mix_ms, state, submits) = audio::diagnostics();
                help::audio_status(pixels, mix_ms, state, submits);
                help::audio_reset(pixels, self.audio_reset_done);
            } else {
                help::performance(pixels, self.fps, audio::max_fill_ms());
            }
        }
    }
}
