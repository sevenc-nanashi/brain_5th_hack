#![no_std]
#![no_main]

use core::{ffi::c_void, ptr};
use reflex::{
    Action, Application, Event, Window, display,
    peg::{self, key, raw},
    runtime, text,
};

const TIMER: u32 = 0x5553;
const HELLO: &[u8] = b"REFLEX USB PROBE v1\n";

// V690_H1 only; keep the unverified ABI local to this probe.
unsafe fn control(
    command: u32,
    input: *const c_void,
    size: u32,
    output: *mut c_void,
    length: u32,
    count: *mut u32,
) -> u32 {
    let call: unsafe extern "C" fn(u32, *const c_void, u32, *mut c_void, u32, *mut u32) -> u32 =
        unsafe { core::mem::transmute(0x6000_63c0usize) };
    unsafe { call(command, input, size, output, length, count) }
}
fn request(command: u32) -> bool {
    unsafe { control(command, ptr::null(), 0, ptr::null_mut(), 0, ptr::null_mut()) != 0 }
}
fn enable(value: bool) -> bool {
    let dummy = 0u32;
    unsafe {
        control(
            if value { 0x2003a } else { 0x2003b },
            (&dummy as *const u32).cast(),
            4,
            ptr::null_mut(),
            0,
            ptr::null_mut(),
        ) != 0
    }
}
fn connected() -> bool {
    let mut value = 0u8;
    unsafe {
        control(
            0x20039,
            ptr::null(),
            0,
            (&mut value as *mut u8).cast(),
            1,
            ptr::null_mut(),
        );
    }
    value == 1
}
#[derive(Clone, Copy, PartialEq)]
enum State {
    Idle,
    Detaching,
    Connecting,
    Sent,
    Stopping,
    Closed,
    Error,
}
struct Probe {
    window: *mut c_void,
    state: State,
    since: u64,
    message: &'static str,
    touched: bool,
}
impl Probe {
    fn set(&mut self, state: State, message: &'static str) {
        self.state = state;
        self.message = message;
        self.since = unsafe { runtime::micros() };
    }
    fn stop(&mut self) {
        if enable(false) && request(0x2002c) {
            self.set(State::Stopping, "Disconnecting...");
        } else {
            self.set(State::Error, "Disconnect failed. Esc: retry");
        }
    }
    fn tick(&mut self) -> Action {
        let elapsed = unsafe { runtime::micros() }.wrapping_sub(self.since);
        if matches!(
            self.state,
            State::Detaching | State::Connecting | State::Stopping
        ) && elapsed > 30_000_000
        {
            self.set(State::Error, "USB timeout. Esc: disconnect");
            return Action::Redraw;
        }
        if elapsed < 2_000_000 {
            return Action::None;
        }
        match self.state {
            State::Detaching if !connected() => {
                let selected = unsafe {
                    control(
                        0x2002d,
                        c"DRM:".as_ptr().cast(),
                        4,
                        ptr::null_mut(),
                        0,
                        ptr::null_mut(),
                    )
                };
                if selected != 0 && request(0x2002b) {
                    self.set(State::Connecting, "Waiting for PC...");
                } else {
                    self.set(State::Error, "DRM selection failed. Esc: stop");
                }
            }
            State::Connecting if connected() => {
                if !enable(true) {
                    self.set(State::Error, "Enable failed. Esc: stop");
                } else {
                    let mut count = 0;
                    // The firmware copies from this argument despite its output-buffer position.
                    let ok = unsafe {
                        control(
                            0x20035,
                            ptr::null(),
                            0,
                            HELLO.as_ptr().cast_mut().cast(),
                            HELLO.len() as u32,
                            &mut count,
                        )
                    };
                    if ok != 0 && count == HELLO.len() as u32 {
                        self.set(State::Sent, "Hello queued. Read on PC.");
                    } else {
                        self.set(State::Error, "Send failed. Esc: stop");
                    }
                }
            }
            State::Stopping if !connected() => {
                self.set(State::Closed, "Disconnected");
                return Action::Exit;
            }
            _ => return Action::None,
        }
        Action::Redraw
    }
}
#[reflex::main]
fn main() -> Probe {
    Probe {
        window: ptr::null_mut(),
        state: State::Idle,
        since: 0,
        message: "Enter: connect   Esc: exit",
        touched: false,
    }
}
impl Application for Probe {
    fn mounted(&mut self, window: Window) -> Action {
        self.window = window.raw;
        unsafe {
            raw::set_timer(self.window, TIMER, 10, 10);
        }
        Action::Redraw
    }
    fn raw_event(&mut self, message: &peg::Message) -> Option<Action> {
        if message.kind == peg::PM_TIMER && message.param == TIMER as u16 {
            Some(self.tick())
        } else {
            None
        }
    }
    fn event(&mut self, event: Event) -> Action {
        match event {
            Event::Key {
                code: key::ENTER,
                down: true,
                ..
            } if self.state == State::Idle => {
                self.touched = true;
                if request(0x2002c) {
                    self.set(State::Detaching, "Switching to DRM...");
                } else {
                    self.set(State::Error, "Disconnect failed. Esc: retry");
                }
                Action::Redraw
            }
            Event::Key {
                code: key::ESCAPE,
                down: true,
                ..
            } => Action::Exit,
            _ => Action::None,
        }
    }
    fn can_exit(&mut self) -> bool {
        if !self.touched || self.state == State::Closed {
            unsafe {
                raw::kill_timer(self.window, TIMER);
            }
            true
        } else {
            if self.state != State::Stopping {
                self.stop();
            }
            false
        }
    }
    fn draw(&self, pixels: &mut [u16]) {
        pixels.fill(0xffff);
        for (y, line) in [
            (18, "USB Probe / V690_H1"),
            (62, self.message),
            (110, "Esc: disconnect and exit"),
        ] {
            text::draw(
                |x, y, color| pixels[y * display::WIDTH + x] = color,
                16,
                y,
                line,
                0,
                0xffff,
                1,
            );
        }
    }
}
