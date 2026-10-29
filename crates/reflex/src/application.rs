//! Host-independent, single-window application interface.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    None,
    Redraw,
    Exit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TouchPhase {
    Down,
    Move,
    Up,
}

#[derive(Clone, Copy, Debug)]
pub enum Event {
    Key { code: u16, down: bool, flags: u32 },
    Touch { x: u16, y: u16, phase: TouchPhase },
}

/// State owned by Reflex until Exit or host window destruction.
/// Callbacks run on the PEG event thread. Return Exit instead of calling raw exit.
pub trait Application: 'static {
    fn event(&mut self, event: Event) -> Action;
    /// Draw a complete WIDTH × HEIGHT row-major RGB565 frame.
    fn draw(&self, pixels: &mut [u16]);
}
