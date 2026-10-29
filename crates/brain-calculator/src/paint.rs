use crate::{BUTTON_PITCH, BUTTON_WIDTH, BUTTONS, Calculator, UI_WIDTH, format_result};
use alloc::format;
use reflex::{
    display::{HEIGHT, WIDTH},
    text,
};
const BG: u16 = 0xf7bd;
const INK: u16 = 0x18e3;
const MUTED: u16 = 0x4a69;
const ACCENT: u16 = 0x1b89;
const ERROR: u16 = 0xa900;
const FUNCTION: u16 = 0xe734;
const OPERATOR: u16 = 0xdedb;

struct Canvas<'a>(&'a mut [u16]);
impl Canvas<'_> {
    fn rect(&mut self, x: usize, y: usize, w: usize, h: usize, color: u16) {
        for row in y..(y + h).min(HEIGHT) {
            if x < UI_WIDTH {
                self.0[row * WIDTH + x..row * WIDTH + (x + w).min(UI_WIDTH)].fill(color);
            }
        }
    }
    fn text(&mut self, x: usize, y: usize, value: &str, fg: u16, bg: u16, scale: usize) {
        text::draw(
            |x, y, c| {
                if x < UI_WIDTH {
                    self.0[y * WIDTH + x] = c;
                }
            },
            x,
            y,
            value,
            fg,
            bg,
            scale,
        );
    }
}

pub fn draw(app: &Calculator, pixels: &mut [u16]) {
    assert_eq!(pixels.len(), WIDTH * HEIGHT);
    pixels.fill(BG);
    let mut c = Canvas(pixels);
    c.text(
        24,
        16,
        if app.show_history {
            "History"
        } else {
            "Scientific"
        },
        INK,
        BG,
        1,
    );
    if app.show_history {
        c.rect(666, 16, 118, 42, ACCENT);
        c.text(701, 24, "Back", 0xffff, ACCENT, 1);
        for (x, label) in [(354, "Prev"), (498, "Next")] {
            c.rect(x, 16, 118, 42, OPERATOR);
            c.text(x + 35, 24, label, INK, OPERATOR, 1);
        }
        if app.history.is_empty() {
            c.text(24, 100, "No calculations", MUTED, BG, 1);
            return;
        }
        let selected = app.history_selection.unwrap();
        let start = selected / 5 * 5;
        c.text(
            240,
            16,
            &format!("{} / {}", selected + 1, app.history.len()),
            MUTED,
            BG,
            1,
        );
        for (row, entry) in app.history.iter().skip(start).take(5).enumerate() {
            let y = 84 + row * 72;
            let bg = if start + row == selected {
                ACCENT
            } else {
                0xffff
            };
            let fg = if start + row == selected { 0xffff } else { INK };
            c.rect(16, y, UI_WIDTH - 32, 66, bg);
            let count = entry.expression.len().min(46);
            c.text(28, y + 4, &entry.expression[..count], fg, bg, 1);
            if count < entry.expression.len() {
                c.text(580, y + 4, "...", fg, bg, 1);
            }
            c.text(690, y + 4, entry.angle.label(), fg, bg, 1);
            c.text(
                28,
                y + 34,
                &format!("= {}", format_result(entry.result)),
                fg,
                bg,
                1,
            );
        }
        return;
    }
    c.text(606, 16, app.angle.label(), ACCENT, BG, 1);
    c.text(666, 16, &format!("H{:02}", app.history.len()), MUTED, BG, 1);
    c.rect(16, 54, UI_WIDTH - 32, 100, 0xffff);
    let start = app.cursor.saturating_sub(60);
    let end = (start + 62).min(app.expression.len());
    c.text(28, 58, &app.expression[start..end], INK, 0xffff, 1);
    c.rect(28 + (app.cursor - start) * 12, 84, 10, 2, ACCENT);
    if let Some(error) = app.error {
        c.text(
            28,
            110,
            &format!("{}  @{}", error.message, error.position + 1),
            ERROR,
            0xffff,
            1,
        );
    } else if let Some(result) = app.result {
        let result = format_result(result);
        c.text(760 - result.len() * 24, 96, &result, INK, 0xffff, 2);
    } else {
        c.text(736, 110, "0", MUTED, 0xffff, 1);
    }
    for (index, label) in BUTTONS.iter().enumerate() {
        let x = 16 + (index % 8) * BUTTON_PITCH;
        let y = 164 + (index / 8) * 62;
        let bg = if index == 39 || app.pressed == Some(index) {
            ACCENT
        } else if index < 14 {
            FUNCTION
        } else if label.as_bytes().iter().all(u8::is_ascii_digit) || *label == "." {
            0xffff
        } else {
            OPERATOR
        };
        let fg = if bg == ACCENT { 0xffff } else { INK };
        c.rect(x, y, BUTTON_WIDTH, 56, bg);
        if app.focus == index {
            c.rect(x, y, BUTTON_WIDTH, 56, INK);
            c.rect(x + 2, y + 2, BUTTON_WIDTH - 4, 52, 0xffff);
            c.rect(x + 4, y + 4, BUTTON_WIDTH - 8, 48, bg);
        }
        let label = if index == 6 { app.angle.label() } else { label };
        c.text(
            x + (BUTTON_WIDTH - label.len() * 12) / 2,
            y + 15,
            label,
            fg,
            bg,
            1,
        );
    }
}
