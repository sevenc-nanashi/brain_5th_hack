//! Host-independent, single-window application interface.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    None,
    Redraw,
    /// Redraw, then let the host process the original message (e.g. clipping gestures).
    RedrawAndBubble,
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
    /// Called after window registration. Raw pointers are valid until application exit.
    /// Raw APIs still require their documented firmware and safety conditions.
    fn mounted(&mut self, _window: Window) -> Action {
        Action::Redraw
    }
    /// Inspect PEG messages before typed input dispatch. None uses normal dispatch;
    /// Some(Action::None) consumes the message without drawing.
    fn raw_event(&mut self, _message: &crate::peg::Message) -> Option<Action> {
        None
    }
    /// Returning false keeps the application loaded and redraws it, allowing a retry.
    /// Stop external services and timers before returning true.
    fn can_exit(&mut self) -> bool {
        true
    }
    /// Called before drawing, including redraws requested by the host.
    fn before_draw(&mut self, _invalid: crate::peg::Rect) {}
    fn event(&mut self, event: Event) -> Action;
    /// Draw a complete WIDTH × HEIGHT row-major RGB565 frame.
    fn draw(&self, pixels: &mut [u16]);
}

#[derive(Clone, Copy)]
pub struct Window {
    pub raw: *mut core::ffi::c_void,
    pub presentation: *mut core::ffi::c_void,
}
