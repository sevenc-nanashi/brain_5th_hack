#![no_std]
#![no_main]
use brain_hello_world::render;
use core::ffi::c_void;
use reflex::{
    Action, Application, Event, Window, display,
    peg::{self, raw},
    runtime,
};

const TIMER: u32 = 0x4845;
#[derive(Default)]
struct HelloWorld {
    window: *mut c_void,
    started_us: u64,
}

#[reflex::main]
fn main() -> HelloWorld {
    HelloWorld::default()
}

impl Application for HelloWorld {
    fn mounted(&mut self, window: Window) -> Action {
        self.window = window.raw;
        unsafe {
            self.started_us = runtime::micros();
            raw::set_timer(self.window, TIMER, 10, 10);
        }
        Action::Redraw
    }
    fn raw_event(&mut self, message: &peg::Message) -> Option<Action> {
        if message.kind == peg::PM_TIMER && message.param == TIMER as u16 {
            Some(
                if unsafe { runtime::micros() }.wrapping_sub(self.started_us) >= 3_000_000 {
                    Action::Exit
                } else {
                    Action::None
                },
            )
        } else {
            None
        }
    }
    fn event(&mut self, event: Event) -> Action {
        if matches!(event, Event::Key { down: true, .. }) {
            Action::Exit
        } else {
            Action::None
        }
    }
    fn can_exit(&mut self) -> bool {
        unsafe {
            raw::kill_timer(self.window, TIMER);
        }
        true
    }
    fn draw(&self, pixels: &mut [u16]) {
        render(|x, y, color| pixels[y * display::WIDTH + x] = color);
    }
}
