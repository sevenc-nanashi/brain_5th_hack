#![no_std]
#![no_main]
extern crate alloc;
mod engine;
mod runtime;
mod usb;

use core::{
    ffi::{CStr, c_void},
    ptr,
};
use reflex::{
    Action, Application, Event, TouchPhase, Window, display,
    peg::{self, key, raw},
    runtime::micros,
    text,
};

const TIMER: u32 = 0x4257;
#[derive(Clone, Copy, PartialEq)]
enum State {
    Idle,
    Detaching,
    Connecting,
    Ready,
    Stopping,
    Closed,
    Error,
}
struct Browser {
    window: *mut c_void,
    state: State,
    since: u64,
    activity: u64,
    message: &'static str,
    started: bool,
    touched: bool,
    transport: usb::Transport,
    pointer: (i32, i32),
    last_key: Option<(u16, u64)>,
}
#[reflex::main]
fn main() -> Browser {
    Browser {
        window: ptr::null_mut(),
        state: State::Idle,
        since: 0,
        activity: 0,
        message: "Enter: connect USB   Esc: exit",
        started: false,
        touched: false,
        transport: usb::Transport::new(),
        pointer: (600, 200),
        last_key: None,
    }
}
impl Browser {
    fn set(&mut self, state: State, message: &'static str) {
        self.state = state;
        self.message = message;
        self.since = unsafe { micros() };
    }
    fn stop(&mut self) {
        self.set(State::Stopping, "Disconnecting...");
        let disabled = usb::command4(0x2003b);
        let detached = usb::command(0x2002c);
        if !disabled || !detached {
            self.set(State::Error, "USB disconnect failed. Esc: retry");
        }
    }
    fn tick(&mut self) -> Result<Action, &'static str> {
        let now = unsafe { micros() };
        let elapsed = now.wrapping_sub(self.since);
        if matches!(
            self.state,
            State::Detaching | State::Connecting | State::Stopping
        ) && elapsed > 120_000_000
        {
            return Err("USB timeout. Esc: disconnect");
        }
        match self.state {
            State::Detaching if elapsed >= 2_000_000 && !usb::connected() => {
                if !usb::connect() {
                    return Err("DRM connection failed");
                }
                self.set(State::Connecting, "Connect PC bridge");
            }
            State::Connecting if elapsed >= 2_000_000 && usb::connected() => {
                if !usb::command4(0x2003a) || !usb::command4(0x20036) {
                    return Err("USB enable failed");
                }
                self.activity = now;
                self.set(State::Ready, "Waiting for PC...");
            }
            State::Ready => {
                if !usb::connected() {
                    return Err("USB disconnected. Esc: exit");
                }
                if self.transport.poll()? {
                    self.activity = now;
                    if !self.started {
                        // Establish the PC clock before NetSurf creates cache timestamps.
                        let result = unsafe {
                            engine::run(|| {
                                engine::brain_tick(now / 1000);
                                engine::brain_init(c"http://abehiroshi.la.coocan.jp/".as_ptr())
                            })
                        };
                        if result != 0 {
                            return Err("NetSurf initialisation failed");
                        }
                        self.started = true;
                    }
                }
                if now.wrapping_sub(self.activity) > 120_000_000 {
                    return Err("PC timeout. Esc: disconnect");
                }
                let dirty = unsafe { engine::run(|| engine::brain_tick(now / 1000)) };
                return Ok(if dirty != 0 {
                    Action::Redraw
                } else {
                    Action::None
                });
            }
            State::Stopping if elapsed >= 2_000_000 && !usb::connected() => {
                self.set(State::Closed, "Disconnected");
                return Ok(Action::Exit);
            }
            _ => return Ok(Action::None),
        }
        Ok(Action::Redraw)
    }
}
impl Application for Browser {
    fn mounted(&mut self, window: Window) -> Action {
        self.window = window.raw;
        unsafe {
            raw::set_timer(self.window, TIMER, 1, 1);
        }
        Action::Redraw
    }
    fn raw_event(&mut self, message: &peg::Message) -> Option<Action> {
        if message.kind != peg::PM_TIMER || message.param != TIMER as u16 {
            return None;
        }
        Some(match self.tick() {
            Ok(a) => a,
            Err(e) => {
                self.set(State::Error, e);
                Action::Redraw
            }
        })
    }
    fn event(&mut self, event: Event) -> Action {
        match event {
            Event::Key {
                code, down: true, ..
            } => {
                let now = unsafe { micros() };
                if self
                    .last_key
                    .is_some_and(|(last, at)| last == code && now.wrapping_sub(at) < 100_000)
                {
                    return Action::None;
                }
                self.last_key = Some((code, now));
                if code == key::ESCAPE {
                    return Action::Exit;
                }
                if code == key::ENTER && self.state == State::Idle {
                    self.touched = true;
                    self.set(State::Detaching, "Switching USB...");
                    if !usb::command(0x2002c) {
                        self.set(State::Error, "USB disconnect failed. Esc: retry");
                    }
                } else if self.state == State::Ready {
                    unsafe {
                        engine::run(|| match code {
                            key::UP | key::PAGE_UP => {
                                engine::brain_scroll(self.pointer.0, self.pointer.1, -120)
                            }
                            key::DOWN | key::PAGE_DOWN => {
                                engine::brain_scroll(self.pointer.0, self.pointer.1, 120)
                            }
                            key::BACKSPACE => engine::brain_back(),
                            _ => (),
                        });
                    }
                }
                Action::Redraw
            }
            Event::Touch { x, y, phase } if self.state == State::Ready && y < 448 => {
                self.pointer = (x as i32, y as i32);
                if matches!(phase, TouchPhase::Up) {
                    unsafe {
                        engine::run(|| engine::brain_click(x as i32, y as i32));
                    }
                }
                Action::Redraw
            }
            _ => Action::None,
        }
    }
    fn can_exit(&mut self) -> bool {
        if self.touched && self.state != State::Closed {
            if self.state != State::Stopping {
                self.stop();
            }
            return false;
        }
        unsafe {
            raw::kill_timer(self.window, TIMER);
            if self.started {
                engine::run(|| engine::brain_stop());
                self.started = false;
            }
        }
        true
    }
    fn draw(&self, pixels: &mut [u16]) {
        pixels.fill(0xffff);
        if self.started {
            unsafe {
                engine::run(|| engine::brain_draw(pixels.as_mut_ptr()));
            }
        }
        let message = if self.state == State::Ready && self.started {
            unsafe { CStr::from_ptr(engine::brain_status()) }
                .to_str()
                .expect("NetSurf status must be UTF-8")
        } else {
            self.message
        };
        text::draw(
            |x, y, c| pixels[y * display::WIDTH + x] = c,
            4,
            452,
            message,
            0,
            0xffff,
            1,
        );
    }
}
