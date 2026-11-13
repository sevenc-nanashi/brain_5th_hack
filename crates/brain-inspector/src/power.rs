//! Experimental V690_H1 power requests; event meanings are not established.
use crate::input::KeyEvent;
use reflex::peg::{self, key};
pub const EVENTS: [(u32, &str); 8] = [
    (0, "DEFAULT (KERNEL USES 00000102)"),
    (0x102, "EXPLICIT DEFAULT"),
    (0x80, "ALLOW MASK 1"),
    (0x08, "ALLOW MASK 2"),
    (0x10, "ALLOW MASK 4"),
    (0x04, "ALLOW MASK 8"),
    (0x20, "BYPASS ALLOW CHECK"),
    (0x40, "BYPASS / SEND TO BOTH CORES"),
];
#[derive(Clone, Copy, Default)]
pub struct Power {
    pub selected: usize,
    pub confirm: bool,
    pub pending: Option<u32>,
    pub issued: Option<u32>,
    pub result: Option<u32>,
    pub elapsed_us: u64,
    pub calls: u32,
    held: u8,
}
impl Power {
    pub fn cancel(&mut self) {
        self.confirm = false;
        self.pending = None;
    }
    /// Consume controls even while locked; require separate fresh Enter and Y presses.
    pub fn key(&mut self, e: KeyEvent, active: bool, locked: bool) -> bool {
        let bit = match e.key {
            key::ENTER => 1,
            0x59 | 0x79 => 2,
            _ => 0,
        };
        let down = e.kind != peg::PM_KEY_UP;
        let fresh = down && self.held & bit == 0;
        if down {
            self.held |= bit;
        } else {
            self.held &= !bit;
        }
        if !active || locked {
            self.cancel();
            return active
                && matches!(
                    e.key,
                    key::ENTER | key::UP | key::DOWN | 0x59 | 0x79 | 0x4e | 0x6e
                );
        }
        let consumed = matches!(
            e.key,
            key::ENTER | key::UP | key::DOWN | 0x59 | 0x79 | 0x4e | 0x6e | key::BACKSPACE
        ) || (e.key == key::ESCAPE && (self.confirm || self.pending.is_some()));
        if !down {
            return consumed;
        }
        if e.key == key::ESCAPE || e.key == key::BACKSPACE || matches!(e.key, 0x4e | 0x6e) {
            self.cancel();
        } else if self.pending.is_none() {
            match e.key {
                key::UP => {
                    self.selected = self.selected.saturating_sub(1);
                    self.confirm = false;
                }
                key::DOWN => {
                    self.selected = (self.selected + 1).min(EVENTS.len() - 1);
                    self.confirm = false;
                }
                key::ENTER if fresh => self.confirm = true,
                0x59 | 0x79 if fresh && self.confirm => {
                    self.pending = Some(EVENTS[self.selected].0);
                    self.confirm = false;
                }
                _ => {}
            }
        }
        consumed
    }
    pub fn take_request(&mut self) -> Option<u32> {
        let event = self.pending.take()?;
        self.issued = Some(event);
        self.result = None;
        self.elapsed_us = 0;
        self.calls = self.calls.wrapping_add(1);
        Some(event)
    }
}
