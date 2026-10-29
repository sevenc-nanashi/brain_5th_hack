#![no_std]
#![no_main]
use brain_inspector::{
    Measurements,
    input::{Input, KeyEvent, Mode},
    render,
};
use core::ffi::c_void;
use reflex::{
    Action, Application, Event, Window,
    peg::{self, raw},
    runtime,
};
mod audio;
const TIMER: u32 = 0x5052;
const USABLE_WIDTH: usize = 800;
#[derive(Default)]
struct Inspector {
    measurements: Measurements,
    input: Input,
    window: *mut c_void,
    presentation: *mut c_void,
    audio_keys: u8,
}
#[reflex::main]
fn main() -> Inspector {
    Inspector::default()
}
unsafe fn rectangle(object: *mut c_void, offset: usize) -> [i16; 4] {
    unsafe { object.cast::<u8>().add(offset).cast::<[i16; 4]>().read() }
}
impl Inspector {
    unsafe fn sample(&mut self) {
        unsafe {
            let m = &mut self.measurements;
            raw::screen_rect(self.presentation, m.screen.as_mut_ptr());
            m.pm_real = rectangle(self.presentation, 0x0c);
            m.pm_client = rectangle(self.presentation, 0x14);
            m.pm_clip = rectangle(self.presentation, 0x1c);
            m.window_real = rectangle(self.window, 0x0c);
            m.window_client = rectangle(self.window, 0x14);
            m.window_clip = rectangle(self.window, 0x1c);
        }
    }
    unsafe fn message(&mut self, msg: &peg::Message) -> Option<Action> {
        unsafe {
            let window = self.window;
            if msg.kind >= 0x100 && self.input.mode == Mode::Audio {
                let mut m = self.measurements;
                audio::event(msg, &mut m.audio);
                self.measurements = m;
                if msg.kind == reflex::audio::PM_AUDIO {
                    return Some(Action::None);
                }
            }
            match msg.kind {
                peg::PM_KEY if msg.param == peg::TOUCH_MOVE_PARAM => {
                    let mut m = self.measurements;
                    let p = msg.data.point;
                    m.touch = Some((p.x, p.y));
                    m.touch_down = true;
                    let mut input = self.input;
                    input.touch(p.x, p.y);
                    self.input = input;
                    self.measurements = m;
                    // shortcut: suppress bubbling like PCConnect; verify the system gesture on device.
                    if !input.window_clip {
                        return Some(Action::Redraw);
                    }
                }
                peg::PM_KEY | peg::PM_KEY_DOWN | peg::PM_KEY_UP => {
                    let mut m = self.measurements;
                    m.kind = msg.kind;
                    m.key = msg.param;
                    m.flags = msg.data.ext_params[0];
                    if msg.kind != peg::PM_KEY_UP {
                        m.pressed_key = Some(msg.param);
                        m.pressed_flags = msg.data.ext_params[0];
                    }
                    self.measurements = m;
                    let mut input = self.input;
                    let previous_mode = input.mode;
                    let consumed = input.key(
                        KeyEvent {
                            key: msg.param,
                            kind: msg.kind,
                            flags: m.flags,
                        },
                        runtime::micros(),
                    );
                    if previous_mode == Mode::Audio
                        && input.mode != Mode::Audio
                        && !audio::stop(&mut m.audio)
                    {
                        input.mode = Mode::Audio;
                    }
                    let audio_bit = match msg.param {
                        0x51 | 0x71 => 1, // Q/q: single tone
                        0x57 | 0x77 => 2, // W/w: stream
                        0x50 | 0x70 => 4, // P/p: stop
                        peg::key::PAGE_UP => 8,
                        peg::key::PAGE_DOWN => 16,
                        peg::key::LEFT => 32,
                        peg::key::RIGHT => 64,
                        _ => 0,
                    };
                    let audio_control = input.mode == Mode::Audio && audio_bit != 0;
                    if audio_control {
                        if msg.kind == peg::PM_KEY_UP {
                            self.audio_keys &= !audio_bit;
                        } else if self.audio_keys & audio_bit == 0 {
                            self.audio_keys |= audio_bit;
                            if audio_bit == 4 {
                                audio::stop(&mut m.audio);
                            } else if audio_bit >= 32 && !input.locked {
                                audio::change_buffer(audio_bit == 64, &mut m.audio);
                            } else if audio_bit >= 8 && !input.locked {
                                audio::change_volume(audio_bit == 8, &mut m.audio);
                            } else if !input.locked {
                                audio::play(window, audio_bit == 2, &mut m.audio);
                            }
                        }
                    } else if msg.kind == peg::PM_KEY_UP {
                        self.audio_keys &= !audio_bit;
                    }
                    self.measurements = m;
                    self.input = input;
                    if let Some(confirmed) = self.input.panic_test(KeyEvent {
                        key: msg.param,
                        kind: msg.kind,
                        flags: m.flags,
                    }) {
                        if confirmed {
                            panic!("Inspector: intentional panic test");
                        }
                        return Some(Action::Redraw);
                    }
                    if msg.kind != peg::PM_KEY_UP && msg.param == peg::key::ESCAPE && !input.locked
                    {
                        return Some(Action::Exit);
                    }
                    if consumed
                        || audio_control
                        || matches!(
                            msg.param,
                            peg::key::LEFT
                                | peg::key::RIGHT
                                | peg::key::UP
                                | peg::key::DOWN
                                | peg::key::ENTER
                        )
                    {
                        return Some(Action::Redraw);
                    }
                }
                peg::PM_LBUTTONDOWN | peg::PM_LBUTTONUP => {
                    let mut m = self.measurements;
                    let p = msg.data.point;
                    m.touch = Some((p.x, p.y));
                    m.touch_down = msg.kind == peg::PM_LBUTTONDOWN;
                    let mut input = self.input;
                    input.touch(p.x, p.y);
                    self.input = input;
                    self.measurements = m;
                    if !input.window_clip {
                        return Some(Action::Redraw);
                    }
                }
                peg::PM_TIMER if msg.param == TIMER as u16 => {
                    let mut m = self.measurements;
                    audio::maintain(window, &mut m.audio);
                    self.measurements = m;
                    self.sample();
                    return Some(Action::Redraw);
                }
                _ => return None,
            }
            Some(Action::RedrawAndBubble)
        }
    }
}
impl Application for Inspector {
    fn mounted(&mut self, window: Window) -> Action {
        self.window = window.raw;
        self.presentation = window.presentation;
        unsafe {
            self.sample();
            raw::set_timer(self.window, TIMER, 10, 10);
        }
        Action::Redraw
    }
    fn raw_event(&mut self, message: &peg::Message) -> Option<Action> {
        unsafe { self.message(message) }
    }
    fn event(&mut self, _: Event) -> Action {
        Action::None
    }
    fn can_exit(&mut self) -> bool {
        unsafe {
            if !audio::stop(&mut self.measurements.audio) {
                self.input.mode = Mode::Audio;
                return false;
            }
            raw::kill_timer(self.window, TIMER);
        }
        true
    }
    fn before_draw(&mut self, invalid: peg::Rect) {
        self.measurements.invalid = Some([
            invalid.left as i16,
            invalid.top as i16,
            invalid.right as i16,
            invalid.bottom as i16,
        ]);
        unsafe {
            self.sample();
        }
    }
    fn draw(&self, pixels: &mut [u16]) {
        render(pixels, &self.measurements, USABLE_WIDTH, &self.input);
    }
}
