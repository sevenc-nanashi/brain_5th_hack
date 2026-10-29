#![no_std]
extern crate alloc;

pub mod eval;
mod paint;
use alloc::{
    collections::VecDeque,
    format,
    string::{String, ToString},
};
use eval::{Angle, Error};
use reflex::{Action, Application, Event, TouchPhase, peg};

pub const BUTTONS: [&str; 40] = [
    "sin", "cos", "tan", "asin", "acos", "atan", "DEG", "HIST", "ln", "log", "sqrt", "^", "(", ")",
    "DEL", "AC", "7", "8", "9", "/", "pi", "e", "<", ">", "4", "5", "6", "*", "Ans", "exp", "(-)",
    "EXIT", "1", "2", "3", "-", "0", ".", "+", "=",
];
const UI_WIDTH: usize = 800;
const BUTTON_PITCH: usize = (UI_WIDTH - 32) / 8;
const BUTTON_WIDTH: usize = BUTTON_PITCH - 6;
const HISTORY_LIMIT: usize = 32;
struct Entry {
    expression: String,
    result: f64,
    angle: Angle,
}

pub struct Calculator {
    expression: String,
    cursor: usize,
    result: Option<f64>,
    ans: Option<f64>,
    error: Option<Error>,
    angle: Angle,
    history: VecDeque<Entry>,
    history_selection: Option<usize>,
    show_history: bool,
    focus: usize,
    pressed: Option<usize>,
    keys_down: [u32; 32],
}
impl Default for Calculator {
    fn default() -> Self {
        Self::new()
    }
}
impl Calculator {
    pub fn new() -> Self {
        Self {
            expression: String::new(),
            cursor: 0,
            result: None,
            ans: None,
            error: None,
            angle: Angle::Deg,
            history: VecDeque::new(),
            history_selection: None,
            show_history: false,
            focus: 32,
            pressed: None,
            keys_down: [0; 32],
        }
    }
    fn insert(&mut self, value: &str) {
        if self.expression.len() + value.len() > eval::MAX_EXPRESSION {
            self.error = Some(Error {
                position: self.cursor,
                message: "Expression too long",
            });
            return;
        }
        self.expression.insert_str(self.cursor, value);
        self.cursor += value.len();
        self.error = None;
        self.result = None;
    }
    fn calculate(&mut self) {
        match eval::evaluate(&self.expression, self.angle, self.ans) {
            Ok((result, expression)) => {
                self.expression = expression;
                self.cursor = self.expression.len();
                self.result = Some(result);
                self.ans = Some(result);
                self.error = None;
                if self.history.len() == HISTORY_LIMIT {
                    self.history.pop_back();
                }
                self.history.push_front(Entry {
                    expression: self.expression.clone(),
                    result,
                    angle: self.angle,
                });
            }
            Err(error) => {
                self.error = Some(error);
                self.result = None;
                self.cursor = error.position.min(self.expression.len());
            }
        }
    }
    fn history(&mut self) {
        self.show_history = !self.show_history;
        self.history_selection = if self.history.is_empty() {
            None
        } else {
            Some(0)
        };
        self.pressed = None;
    }
    fn restore(&mut self) {
        if let Some(index) = self.history_selection {
            let entry = &self.history[index];
            self.expression = entry.expression.clone();
            self.cursor = self.expression.len();
            self.angle = entry.angle;
            self.result = Some(entry.result);
            self.error = None;
            self.show_history = false;
        }
    }
    fn activate(&mut self, index: usize) -> Action {
        match index {
            6 => {
                self.angle = self.angle.toggle();
                self.error = None;
                self.result = None;
            }
            7 => self.history(),
            14 => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                    self.expression.remove(self.cursor);
                }
                self.error = None;
                self.result = None;
            }
            15 => {
                self.expression.clear();
                self.cursor = 0;
                self.error = None;
                self.result = None;
            }
            22 => self.cursor = self.cursor.saturating_sub(1),
            23 => self.cursor = (self.cursor + 1).min(self.expression.len()),
            30 => self.insert("-"),
            31 => return Action::Exit,
            39 => self.calculate(),
            0..=5 | 8..=10 | 29 => self.insert(&format!("{}(", BUTTONS[index])),
            _ => self.insert(BUTTONS[index]),
        }
        Action::Redraw
    }
    fn key(&mut self, code: u16) -> Action {
        use peg::key;
        if code == key::ESCAPE {
            if self.show_history {
                self.show_history = false;
                return Action::Redraw;
            }
            return Action::Exit;
        }
        if code == key::HISTORY {
            self.history();
            return Action::Redraw;
        }
        if self.show_history {
            if let Some(index) = self.history_selection {
                self.history_selection = Some(match code {
                    key::UP => index.saturating_sub(1),
                    key::DOWN => (index + 1).min(self.history.len() - 1),
                    key::PAGE_UP => index.saturating_sub(5),
                    key::PAGE_DOWN => (index + 5).min(self.history.len() - 1),
                    _ => index,
                });
            }
            if code == key::ENTER {
                self.restore();
            }
            return Action::Redraw;
        }
        match code {
            key::LEFT => self.focus = self.focus / 8 * 8 + (self.focus % 8 + 7) % 8,
            key::RIGHT => self.focus = self.focus / 8 * 8 + (self.focus % 8 + 1) % 8,
            key::UP => self.focus = (self.focus + 32) % 40,
            key::DOWN => self.focus = (self.focus + 8) % 40,
            key::ENTER => return self.activate(self.focus),
            key::BACKSPACE => return self.activate(14),
            0x7f => return self.activate(14),
            0x3d => return self.activate(39),
            0x20..=0x7e => {
                let byte = code as u8;
                if byte.is_ascii_alphanumeric() || b".+-*/^() ".contains(&byte) {
                    let bytes = [byte];
                    self.insert(core::str::from_utf8(&bytes).unwrap());
                }
            }
            _ => return Action::None,
        }
        Action::Redraw
    }
    fn hit(&self, x: usize, y: usize) -> Option<usize> {
        if self.show_history {
            if (16..58).contains(&y) {
                if (354..472).contains(&x) {
                    return Some(41);
                }
                if (498..616).contains(&x) {
                    return Some(42);
                }
            }
            if (666..UI_WIDTH - 16).contains(&x) && (16..58).contains(&y) {
                return Some(40);
            }
            if (16..UI_WIDTH - 16).contains(&x) && (84..444).contains(&y) {
                let page = self.history_selection.map_or(0, |i| i / 5 * 5);
                let index = page + (y - 84) / 72;
                if index < self.history.len() {
                    return Some(index);
                }
            }
            return None;
        }
        if !(16..UI_WIDTH - 16).contains(&x) || !(164..474).contains(&y) {
            return None;
        }
        let column = (x - 16) / BUTTON_PITCH;
        let row = (y - 164) / 62;
        if column < 8 && row < 5 && (x - 16) % BUTTON_PITCH < BUTTON_WIDTH && (y - 164) % 62 < 56 {
            Some(row * 8 + column)
        } else {
            None
        }
    }
}
impl Application for Calculator {
    fn event(&mut self, event: Event) -> Action {
        match event {
            Event::Key { code, down, .. } => {
                let word = usize::from(code) / 32;
                if word >= self.keys_down.len() {
                    return Action::None;
                }
                let mask = 1 << (code % 32);
                if !down {
                    self.keys_down[word] &= !mask;
                    return Action::None;
                }
                if self.keys_down[word] & mask != 0 {
                    return Action::None;
                }
                self.keys_down[word] |= mask;
                self.key(code)
            }
            Event::Touch { x, y, phase } => {
                let hit = self.hit(usize::from(x), usize::from(y));
                match phase {
                    TouchPhase::Down => {
                        self.pressed = hit;
                        if !self.show_history
                            && let Some(index) = hit
                        {
                            self.focus = index;
                        }
                    }
                    TouchPhase::Move => {
                        if self.pressed != hit {
                            self.pressed = None;
                        }
                    }
                    TouchPhase::Up => {
                        let pressed = self.pressed.take();
                        if let Some(index) = pressed
                            && pressed == hit
                        {
                            if self.show_history {
                                if index == 40 {
                                    self.show_history = false;
                                } else if index == 41 || index == 42 {
                                    if let Some(selected) = self.history_selection {
                                        self.history_selection = Some(if index == 41 {
                                            selected.saturating_sub(5)
                                        } else {
                                            (selected + 5).min(self.history.len() - 1)
                                        });
                                    }
                                } else {
                                    self.history_selection = Some(index);
                                    self.restore();
                                }
                            } else {
                                return self.activate(index);
                            }
                        }
                    }
                }
                Action::Redraw
            }
        }
    }
    fn draw(&self, pixels: &mut [u16]) {
        paint::draw(self, pixels);
    }
}

pub fn format_result(value: f64) -> String {
    if value == 0.0 {
        return String::from("0");
    }
    let scientific = format!("{value:.11e}");
    let (mantissa, exponent) = scientific.split_once('e').unwrap();
    let exponent: i32 = exponent.parse().unwrap();
    if !(-4..12).contains(&exponent) {
        format!(
            "{}E{exponent}",
            mantissa.trim_end_matches('0').trim_end_matches('.')
        )
    } else {
        let precision = (11 - exponent) as usize;
        let fixed = format!("{value:.precision$}");
        if fixed.contains('.') {
            fixed
                .trim_end_matches('0')
                .trim_end_matches('.')
                .to_string()
        } else {
            fixed
        }
    }
}
