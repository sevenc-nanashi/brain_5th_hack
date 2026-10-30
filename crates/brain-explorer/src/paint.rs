use super::*;

const BG: u16 = 0xf7bd;
const INK: u16 = 0x18e3;
const MUTED: u16 = 0x4a69;
const ACCENT: u16 = 0x1b89;
const ERROR: u16 = 0xa900;
const WHITE: u16 = 0xffff;
const LINE: u16 = 0xdedb;

struct Canvas<'a>(&'a mut [u16]);
impl Canvas<'_> {
    fn rect(&mut self, x: usize, y: usize, w: usize, h: usize, color: u16) {
        for row in y..(y + h).min(HEIGHT) {
            self.0[row * WIDTH + x..row * WIDTH + (x + w).min(800)].fill(color);
        }
    }
    fn text(&mut self, x: usize, y: usize, value: &str, color: u16, scale: usize) {
        for (column, ch) in value.chars().enumerate() {
            let mut encoded = [0; 4];
            // Use the shared font's native bitmap, as Inspector does for dense views.
            reflex::text::draw(
                |dx, dy, pixel| {
                    if dx % 2 == 0 && dy % 2 == 0 && pixel == color {
                        let px = x + (column * 6 + dx / 2) * scale;
                        let py = y + dy / 2 * scale;
                        if px + scale <= 800 && py + scale <= HEIGHT {
                            self.rect(px, py, scale, scale, color);
                        }
                    }
                },
                0,
                0,
                ch.encode_utf8(&mut encoded),
                color,
                color ^ 0xffff,
                1,
            );
        }
    }
}

fn shorten(text: &str, columns: usize) -> String {
    if text.len() <= columns {
        text.into()
    } else {
        format!("{}...", &text[..columns - 3])
    }
}

pub(super) fn draw<F: Filesystem>(app: &Explorer<F>, pixels: &mut [u16]) {
    pixels.fill(BG);
    let mut c = Canvas(pixels);
    c.text(16, 14, "Explorer", INK, 2);
    c.text(620, 20, "R  Refresh", MUTED, 1);
    c.rect(16, 50, 768, 1, LINE);
    let path = if app.folders.is_empty() {
        String::from("Storage")
    } else {
        escaped(&app.path(app.viewer.as_ref().map(|v| v.name.as_slice())))
    };
    let tail = if path.len() > 125 {
        format!("...{}", &path[path.len() - 122..])
    } else {
        path
    };
    c.text(16, 64, &tail, MUTED, 1);

    let (previous, next, last_label) = if let Some(viewer) = &app.viewer {
        let mode = if viewer.hex { "HEX" } else { "TEXT" };
        c.text(
            156,
            20,
            &format!("{mode}    {} bytes", viewer.size),
            MUTED,
            1,
        );
        if viewer.lines.is_empty() {
            c.text(16, 104, "Empty file", MUTED, 2);
        }
        for (row, line) in viewer.lines.iter().enumerate() {
            c.text(
                16,
                86 + row * if viewer.hex { 18 } else { 26 },
                line,
                INK,
                if viewer.hex { 1 } else { 2 },
            );
        }
        if app.error.is_none() {
            c.text(
                16,
                408,
                &format!(
                    "Offset {:08X}    {}",
                    viewer.offset,
                    if viewer.next.is_some() {
                        "More"
                    } else {
                        "End of file"
                    }
                ),
                MUTED,
                1,
            );
        }
        (
            !viewer.history.is_empty(),
            viewer.next.is_some(),
            if viewer.hex { "T  Text" } else { "T  Hex" },
        )
    } else {
        c.text(156, 20, &format!("{} items", app.entries.len()), MUTED, 1);
        if app.entries.is_empty() {
            c.text(16, 104, "Empty folder", MUTED, 2);
        }
        for (row, entry) in app.entries.iter().enumerate().skip(app.top).take(LIST_ROWS) {
            let y = 92 + (row - app.top) * 32;
            let selected = row == app.selected;
            let pressed = app.pressed == Some(Control::Row(row));
            let color = if selected || pressed { WHITE } else { INK };
            c.rect(
                16,
                y,
                760,
                31,
                if selected || pressed { ACCENT } else { WHITE },
            );
            if entry.directory {
                c.rect(26, y + 7, 10, 4, color);
                c.rect(26, y + 11, 18, 13, color);
                c.rect(
                    28,
                    y + 13,
                    14,
                    9,
                    if selected || pressed { ACCENT } else { WHITE },
                );
            } else {
                c.rect(28, y + 6, 14, 20, color);
                c.rect(
                    30,
                    y + 8,
                    10,
                    16,
                    if selected || pressed { ACCENT } else { WHITE },
                );
            }
            c.text(56, y + 3, &shorten(&escaped(&entry.name), 43), color, 2);
            c.text(
                600,
                y + 10,
                &if entry.directory {
                    String::from("Folder")
                } else {
                    format!("{} B", entry.size)
                },
                color,
                1,
            );
        }
        if app.entries.len() > LIST_ROWS {
            c.rect(780, 92, 4, 288, LINE);
            let height = (288 * LIST_ROWS / app.entries.len()).max(8);
            let y = 92 + (288 - height) * app.top / (app.entries.len() - LIST_ROWS);
            c.rect(780, y, 4, height, ACCENT);
        }
        if let Some(entry) = app.entries.get(app.selected) {
            c.text(16, 390, &shorten(&escaped(&entry.name), 127), MUTED, 1);
        }
        (
            app.selected > 0,
            app.selected + 1 < app.entries.len(),
            "Enter  Open",
        )
    };
    if let Some(error) = app.error {
        c.text(16, 408, error, ERROR, 1);
    }
    let controls = [
        (
            Control::Back,
            if app.folders.is_empty() {
                "Esc  Exit"
            } else {
                "Esc  Back"
            },
            true,
        ),
        (Control::Previous, "PgUp  Prev", previous),
        (Control::Next, "PgDn  Next", next),
        (
            if app.viewer.is_some() {
                Control::Toggle
            } else {
                Control::Open
            },
            last_label,
            app.viewer.is_some() || !app.entries.is_empty(),
        ),
    ];
    for (index, (control, label, enabled)) in controls.iter().enumerate() {
        let x = 16 + index * 192;
        let pressed = app.pressed == Some(*control) && *enabled;
        c.rect(x, 426, 184, 48, if pressed { ACCENT } else { LINE });
        c.text(
            x + 8,
            437,
            label,
            if pressed {
                WHITE
            } else if *enabled {
                INK
            } else {
                MUTED
            },
            2,
        );
    }
}
