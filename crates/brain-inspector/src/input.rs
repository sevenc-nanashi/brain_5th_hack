use reflex::{
    display::{HEIGHT, WIDTH},
    peg::{self, key},
};

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Text,
    Keys,
    Touch,
    Pixels,
    Settings,
    Audio,
    Fpu,
    Bar,
}

#[derive(Clone, Copy, Default)]
pub struct KeyEvent {
    pub key: u16,
    pub kind: u16,
    pub flags: i32,
}

#[derive(Clone, Copy)]
pub struct Input {
    pub mode: Mode,
    pub locked: bool,
    pub window_clip: bool,
    pub panic_confirm: bool,
    pub fpu_requested: bool,
    fpu_enter_down: bool,
    pub history: [KeyEvent; 16],
    pub count: usize,
    pub cursor: (usize, usize),
    shift_since: Option<u64>,
    shift_used: bool,
}

impl Default for Input {
    fn default() -> Self {
        Self {
            mode: Mode::Text,
            locked: false,
            window_clip: true,
            panic_confirm: false,
            fpu_requested: false,
            fpu_enter_down: false,
            history: [KeyEvent::default(); 16],
            count: 0,
            cursor: (400, 240),
            shift_since: None,
            shift_used: false,
        }
    }
}

impl Input {
    /// Some(true) confirms a panic; Some(false) consumes the confirmation input.
    pub fn panic_test(&mut self, event: KeyEvent) -> Option<bool> {
        if self.mode != Mode::Settings || self.locked {
            self.panic_confirm = false;
            return None;
        }
        let down = event.kind != peg::PM_KEY_UP;
        if self.panic_confirm {
            if down && matches!(event.key, 0x59 | 0x79) {
                self.panic_confirm = false;
                return Some(true);
            }
            if down && matches!(event.key, 0x4e | 0x6e | key::ESCAPE | key::BACKSPACE) {
                self.panic_confirm = false;
            }
            return Some(false);
        }
        if down && matches!(event.key, 0x50 | 0x70) && event.flags as u32 & peg::KMOD_SHIFT != 0 {
            self.panic_confirm = true;
            return Some(false);
        }
        None
    }

    /// Returns whether the event is an app control, including its release.
    pub fn key(&mut self, event: KeyEvent, now: u64) -> bool {
        self.history.copy_within(0..15, 1);
        self.history[0] = event;
        self.count = (self.count + 1).min(self.history.len());
        let down = event.kind != peg::PM_KEY_UP;
        if event.key == key::ENTER && !down {
            self.fpu_enter_down = false;
        }
        if event.key == key::SHIFT {
            if down {
                if self.shift_since.is_none() {
                    self.shift_since = Some(now);
                    self.shift_used = false;
                }
            } else if let Some(start) = self.shift_since.take()
                && !self.shift_used
                && now.saturating_sub(start) >= 1_000_000
            {
                self.locked = !self.locked;
            }
        } else if self.shift_since.is_some() {
            // A Shift chord must never toggle the lock on release.
            self.shift_used = true;
        }
        let mode = match event.key {
            key::HOME => Some(Mode::Text),
            key::JAPANESE_DICTIONARY => Some(Mode::Keys),
            key::ENGLISH_DICTIONARY => Some(Mode::Touch),
            key::MY_DICTIONARY => Some(Mode::Pixels),
            key::HISTORY => Some(Mode::Audio),
            key::SELECT_FROM_LIST => Some(Mode::Settings),
            _ => None,
        };
        if let Some(mode) = mode {
            if down && !self.locked {
                self.mode = mode;
                self.fpu_requested = false;
            }
            return true;
        }
        if self.mode == Mode::Settings && matches!(event.key, 0x46 | 0x66 | 0x42 | 0x62) {
            if down && !self.locked && !self.panic_confirm {
                self.mode = if matches!(event.key, 0x42 | 0x62) {
                    Mode::Bar
                } else {
                    Mode::Fpu
                };
            }
            return true;
        }
        if self.mode == Mode::Fpu && event.key == key::ENTER {
            if down {
                if !self.fpu_enter_down && !self.locked {
                    self.fpu_requested = true;
                }
                self.fpu_enter_down = true;
            }
            return true;
        }
        if self.mode == Mode::Settings && matches!(event.key, key::LEFT | key::RIGHT) {
            if down && !self.locked {
                self.window_clip = event.key == key::RIGHT;
            }
            return true;
        }
        if self.mode == Mode::Pixels
            && matches!(event.key, key::LEFT | key::RIGHT | key::UP | key::DOWN)
        {
            if down {
                let step = if event.flags as u32 & peg::KMOD_SHIFT != 0 {
                    10
                } else {
                    1
                };
                let (x, y) = &mut self.cursor;
                match event.key {
                    key::LEFT => *x = x.saturating_sub(step),
                    key::RIGHT => *x = (*x + step).min(WIDTH - 1),
                    key::UP => *y = y.saturating_sub(step),
                    key::DOWN => *y = (*y + step).min(HEIGHT - 1),
                    _ => unreachable!(),
                }
            }
            return true;
        }
        event.key == key::ESCAPE
    }

    pub fn touch(&mut self, x: u16, y: u16) {
        if self.shift_since.is_some() {
            self.shift_used = true;
        }
        if self.mode == Mode::Pixels {
            self.cursor = (
                usize::from(x).min(WIDTH - 1),
                usize::from(y).min(HEIGHT - 1),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fpu_controls_require_unlock_and_a_fresh_press() {
        let mut input = Input::default();
        let event = |key, kind| KeyEvent {
            key,
            kind,
            flags: 0,
        };
        input.key(event(key::SELECT_FROM_LIST, peg::PM_KEY), 0);
        input.panic_confirm = true;
        input.key(event(0x46, peg::PM_KEY), 0);
        assert!(input.mode == Mode::Settings);
        input.panic_confirm = false;
        input.locked = true;
        input.key(event(0x46, peg::PM_KEY), 0);
        assert!(input.mode == Mode::Settings);
        input.locked = false;
        input.key(event(0x66, peg::PM_KEY), 0);
        assert!(input.mode == Mode::Fpu);
        input.key(event(key::ENTER, peg::PM_KEY), 0);
        assert!(core::mem::take(&mut input.fpu_requested));
        input.key(event(key::ENTER, peg::PM_KEY), 0);
        assert!(!input.fpu_requested);
        input.key(event(key::ENTER, peg::PM_KEY_UP), 0);
        input.locked = true;
        input.key(event(key::ENTER, peg::PM_KEY), 0);
        assert!(!input.fpu_requested);
        input.key(event(key::ENTER, peg::PM_KEY_UP), 0);
        input.locked = false;
        input.key(event(key::ENTER, peg::PM_KEY), 0);
        assert!(input.fpu_requested);
        input.key(event(key::HOME, peg::PM_KEY), 0);
        assert!(!input.fpu_requested);
    }
}
