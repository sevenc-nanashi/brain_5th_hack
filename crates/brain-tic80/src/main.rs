#![no_std]
#![no_main]
extern crate alloc;

use alloc::{ffi::CString, string::String, vec::Vec};
use brain_explorer::{
    Explorer,
    device::{Device, path},
};
use brain_tic80::{Input, debug, engine, paint};
use core::{
    ffi::{CStr, c_void},
    ptr,
};
use reflex::{
    Action, Application, Event, TouchPhase, Window, display, fs,
    peg::{self, raw},
};

mod runtime;
mod stack;

const TIMER: u32 = 0x5449;
struct Player {
    window: *mut c_void,
    started: bool,
    failed: bool,
    error: String,
    input: Input,
    next_frame_us: u64,
    explorer: Explorer<Device>,
    browsing: bool,
    list_down: bool,
    resume: bool,
    cartridge_path: CString,
    debug_menu: bool,
    show_fps: bool,
    debug_keys: u8,
    reset_down: bool,
    performance: debug::Performance,
}

#[reflex::main]
fn main() -> Player {
    Player {
        window: ptr::null_mut(),
        started: false,
        failed: false,
        error: String::new(),
        input: Input::default(),
        next_frame_us: 0,
        explorer: Explorer::picker(Device, b"tic"),
        browsing: false,
        list_down: false,
        resume: false,
        cartridge_path: c"SD0:\\APP\\TIC80\\cart.tic".into(),
        debug_menu: false,
        show_fps: false,
        debug_keys: 0,
        reset_down: false,
        performance: debug::Performance::default(),
    }
}

fn cartridge(path: &CStr) -> Result<Vec<u8>, &'static str> {
    let mut file = unsafe { fs::File::open(path) }.ok_or("Cannot open cartridge")?;
    let mut data = Vec::new();
    let mut buffer = [0; 4096];
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|_| "Cartridge read failed")?;
        if count == 0 {
            break;
        }
        if data.len() + count > 1024 * 1024 {
            return Err("Cartridge exceeds 1 MiB");
        }
        data.try_reserve(count)
            .map_err(|_| "Cartridge allocation failed")?;
        data.extend_from_slice(&buffer[..count]);
    }
    file.close().map_err(|_| "Cartridge close failed")?;
    Ok(data)
}

impl Player {
    fn failure(&mut self, message: &str) -> Action {
        self.failed = true;
        self.error = String::from(message);
        Action::Redraw
    }
    fn engine_failure(&mut self) -> Action {
        let result = unsafe { engine::brain_tic_result() };
        if result == 1 {
            return Action::Exit;
        }
        if result < 0 {
            let message = unsafe { CStr::from_ptr(engine::brain_tic_error()) }
                .to_string_lossy()
                .into_owned();
            return self.failure(&message);
        }
        self.failure("Game execution failed")
    }
    fn start(&mut self) -> Action {
        self.started = true;
        let data = match cartridge(&self.cartridge_path) {
            Ok(data) => data,
            Err(error) => return self.failure(error),
        };
        let result = unsafe {
            stack::run(|| {
                engine::brain_tic_close();
                engine::brain_tic_init(data.as_ptr(), data.len())
            })
        };
        if result != 0 {
            return self.engine_failure();
        }
        self.reset_performance();
        self.next_frame_us = unsafe { reflex::runtime::micros() };
        Action::Redraw
    }
    fn reset_performance(&mut self) {
        self.performance
            .reset(unsafe { reflex::runtime::micros() }, unsafe {
                engine::brain_tic_frames()
            });
    }
    fn toggle_debug(&mut self) -> Action {
        self.debug_menu = !self.debug_menu;
        self.reset_down = false;
        self.input = Input::default();
        self.send_input();
        Action::Redraw
    }
    fn toggle_browser(&mut self) -> Action {
        if self.browsing {
            self.browsing = false;
            self.resume = self.started && !self.failed;
        } else {
            self.browsing = true;
            self.debug_menu = false;
            self.resume = false;
        }
        self.input = Input::default();
        self.send_input();
        self.explorer.reset_input();
        Action::Redraw
    }

    fn browse(&mut self, event: Event) -> Action {
        let action = self.explorer.event(event);
        if let Some(selected) = self.explorer.take_picked_file() {
            match path(&selected) {
                Ok(path) => self.cartridge_path = path,
                Err(error) => {
                    self.browsing = false;
                    return self.failure(error);
                }
            }
            self.browsing = false;
            self.started = false;
            self.failed = false;
            self.error.clear();
            self.resume = false;
            self.input = Input::default();
            self.send_input();
            // Load outside the key/touch handler.
            return Action::Redraw;
        }
        if action == Action::Exit {
            self.toggle_browser()
        } else {
            action
        }
    }
    fn send_input(&self) {
        unsafe {
            engine::brain_tic_input(
                self.input.buttons(),
                self.input.keyboard(),
                self.input.x,
                self.input.y,
                self.input.touch,
            );
        }
    }
}

impl Application for Player {
    fn mounted(&mut self, window: Window) -> Action {
        self.window = window.raw;
        unsafe {
            raw::set_timer(self.window, TIMER, 1, 1);
        }
        Action::Redraw
    }
    fn raw_event(&mut self, message: &peg::Message) -> Option<Action> {
        if matches!(
            message.kind,
            peg::PM_KEY | peg::PM_KEY_DOWN | peg::PM_KEY_UP
        ) && (self.debug_menu
            || matches!(
                message.param,
                peg::key::SELECT_FROM_LIST | peg::key::HISTORY | peg::key::MY_DICTIONARY
            ))
        {
            // Consume repeats/releases too, rather than forwarding the List key to the host.
            return Some(self.event(Event::Key {
                code: message.param,
                down: message.kind != peg::PM_KEY_UP,
                flags: 0,
            }));
        }
        if message.kind == peg::PM_TIMER && message.param == TIMER as u16 {
            return Some(if self.browsing || self.failed {
                Action::None
            } else if !self.started {
                self.start()
            } else {
                if core::mem::take(&mut self.resume) {
                    self.reset_performance();
                    self.next_frame_us = unsafe { reflex::runtime::micros() };
                }
                let now = unsafe { reflex::runtime::micros() };
                if now < self.next_frame_us {
                    Action::None
                } else {
                    // Preserve the 60 Hz phase across timer jitter; cap catch-up to keep input responsive.
                    for _ in 0..4 {
                        if unsafe { stack::run(|| engine::brain_tic_step()) } != 0 {
                            return Some(self.engine_failure());
                        }
                        self.next_frame_us += 1_000_000 / 60;
                        if self.next_frame_us > now {
                            break;
                        }
                    }
                    if self.next_frame_us <= now {
                        self.next_frame_us = now + 1_000_000 / 60;
                    }
                    Action::Redraw
                }
            });
        }
        None
    }
    fn event(&mut self, event: Event) -> Action {
        if let Event::Key { code, down, .. } = event
            && matches!(code, peg::key::HISTORY | peg::key::MY_DICTIONARY)
        {
            let bit = if code == peg::key::HISTORY { 1 } else { 2 };
            let toggle = down && self.debug_keys & bit == 0;
            if down {
                self.debug_keys |= bit;
            } else {
                self.debug_keys &= !bit;
            }
            if !toggle {
                return Action::None;
            }
            if code == peg::key::MY_DICTIONARY {
                self.show_fps = !self.show_fps;
                return Action::Redraw;
            }
            return if self.browsing {
                Action::None
            } else {
                self.toggle_debug()
            };
        }
        if let Event::Key {
            code: peg::key::SELECT_FROM_LIST,
            down,
            ..
        } = event
        {
            let toggle = down && !self.list_down;
            self.list_down = down;
            return if toggle {
                self.toggle_browser()
            } else {
                Action::None
            };
        }
        if matches!(
            event,
            Event::Key {
                code: peg::key::HOME,
                down: true,
                ..
            }
        ) {
            return Action::Exit;
        }
        if self.browsing {
            return self.browse(event);
        }
        if self.debug_menu {
            match event {
                Event::Key {
                    code: peg::key::ESCAPE,
                    down: true,
                    ..
                } => return self.toggle_debug(),
                Event::Key {
                    code: 0x52 | 0x72,
                    down,
                    ..
                } => {
                    let reset = down && !self.reset_down;
                    self.reset_down = down;
                    if reset {
                        self.reset_performance();
                        return Action::Redraw;
                    }
                    return Action::None;
                }
                _ => return Action::None,
            }
        }
        match event {
            Event::Key {
                code: peg::key::ESCAPE,
                down: true,
                ..
            } => return Action::Exit,
            Event::Key { code, down, .. } => self.input.key(code, down),
            Event::Touch { x, y, phase } => self.input.pointer(x, y, phase != TouchPhase::Up),
        }
        self.send_input();
        Action::None
    }
    fn draw(&self, pixels: &mut [u16]) {
        if self.browsing {
            self.explorer.draw(pixels);
            return;
        }
        if self.failed || !self.started {
            pixels.fill(0);
        }
        if self.failed {
            let message =
                alloc::format!("TIC-80\n\n{}\n\nList  Select game\nEsc  Exit", self.error);
            for (line, chunk) in message
                .lines()
                .flat_map(|s| s.as_bytes().chunks(62))
                .enumerate()
            {
                let text = String::from_utf8_lossy(chunk);
                reflex::text::draw(
                    |x, y, c| pixels[y * display::WIDTH + x] = c,
                    24,
                    24 + line * 26,
                    &text,
                    0xffff,
                    0,
                    1,
                );
            }
        } else if self.started {
            let screen = unsafe { engine::brain_tic_screen() };
            if !screen.is_null() {
                paint(
                    unsafe {
                        core::slice::from_raw_parts(
                            screen,
                            brain_tic80::WIDTH * brain_tic80::HEIGHT,
                        )
                    },
                    pixels,
                );
            }
        }
        if self.show_fps && !self.failed {
            debug::hud(pixels, &self.performance);
        }
        if self.debug_menu {
            debug::menu(
                pixels,
                &self.performance,
                &debug::Audio::default(),
                self.show_fps,
            );
        }
    }
    fn before_draw(&mut self, _: peg::Rect) {
        if self.started && !self.failed && !self.browsing && !self.resume {
            let frame = unsafe { engine::brain_tic_frames() };
            self.performance.rendered(frame);
            // Count actual presentations, including redraws requested outside the timer.
            self.performance
                .sample(unsafe { reflex::runtime::micros() }, frame);
        }
    }
    fn can_exit(&mut self) -> bool {
        unsafe {
            raw::kill_timer(self.window, TIMER);
            stack::run(|| engine::brain_tic_close());
        }
        true
    }
}
