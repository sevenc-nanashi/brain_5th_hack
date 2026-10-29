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
            history: [KeyEvent::default(); 16],
            count: 0,
            cursor: (400, 240),
            shift_since: None,
            shift_used: false,
        }
    }
}

impl Input {
    /// Returns whether the event is an app control, including its release.
    pub fn key(&mut self, event: KeyEvent, now: u64) -> bool {
        self.history.copy_within(0..15, 1);
        self.history[0] = event;
        self.count = (self.count + 1).min(self.history.len());
        let down = event.kind != peg::PM_KEY_UP;
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
            key::HISTORY => Some(Mode::Settings),
            _ => None,
        };
        if let Some(mode) = mode {
            if down && !self.locked {
                self.mode = mode;
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
