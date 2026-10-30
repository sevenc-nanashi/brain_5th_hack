#![no_std]
#![no_main]
use brain_tetris::Tetris;
use core::ffi::c_void;
use reflex::{
    Action, Application, Event, Window,
    peg::{self, raw},
    runtime,
};

const TIMER: u32 = 0x5445;
struct App {
    game: Tetris,
    window: *mut c_void,
    last_us: u64,
}
#[reflex::main]
fn main() -> App {
    App {
        game: Tetris::new(unsafe { runtime::micros() } as u32),
        window: core::ptr::null_mut(),
        last_us: 0,
    }
}
impl App {
    fn advance(&mut self) -> Action {
        let now = unsafe { runtime::micros() };
        let elapsed = now.wrapping_sub(self.last_us) / 1000;
        self.last_us = self.last_us.wrapping_add(elapsed * 1000);
        self.game.advance(elapsed)
    }
}
impl Application for App {
    fn mounted(&mut self, window: Window) -> Action {
        self.window = window.raw;
        unsafe {
            self.last_us = runtime::micros();
            raw::set_timer(self.window, TIMER, 1, 1);
        }
        Action::Redraw
    }
    fn raw_event(&mut self, message: &peg::Message) -> Option<Action> {
        if message.kind != peg::PM_TIMER || message.param != TIMER as u16 {
            return None;
        }
        Some(self.advance())
    }
    fn event(&mut self, event: Event) -> Action {
        let elapsed = self.advance();
        match self.game.event(event) {
            Action::None => elapsed,
            action => action,
        }
    }
    fn draw(&self, pixels: &mut [u16]) {
        self.game.draw(pixels);
    }
    fn can_exit(&mut self) -> bool {
        unsafe {
            raw::kill_timer(self.window, TIMER);
        }
        true
    }
}
