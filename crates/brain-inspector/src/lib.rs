#![no_std]
pub mod bar_image;
pub mod fpu;
pub mod input;
use core::fmt::{self, Write};
use input::{Input, Mode};
use reflex::display::{HEIGHT, WIDTH};

#[derive(Clone, Copy, Default)]
pub struct Measurements {
    pub bar: BarMeasurements,
    pub fpu: fpu::Measurements,
    pub audio: AudioMeasurements,
    pub screen: [i16; 4],
    pub pm_real: [i16; 4],
    pub pm_client: [i16; 4],
    pub pm_clip: [i16; 4],
    pub window_real: [i16; 4],
    pub window_client: [i16; 4],
    pub window_clip: [i16; 4],
    pub invalid: Option<[i16; 4]>,
    pub kind: u16,
    pub key: u16,
    pub flags: i32,
    pub pressed_key: Option<u16>,
    pub pressed_flags: i32,
    pub touch_down: bool,
    pub touch: Option<(u16, u16)>,
}

#[derive(Clone, Copy)]
pub enum BarImage {
    Resource(u32, u32),
    RustLogo,
}

use BarImage::{Resource, RustLogo};
pub const BAR_BUTTONS: [(&str, BarImage); reflex::operation_bar::image::ALL.len() + 1] = {
    let images = reflex::operation_bar::image::ALL;
    let mut buttons = [("RUST LOGO", RustLogo); reflex::operation_bar::image::ALL.len() + 1];
    let mut i = 0;
    while i < images.len() {
        let (name, normal, pressed) = images[i];
        let slot = if i < 9 { i } else { i + 1 };
        buttons[slot] = (name, Resource(normal, pressed));
        i += 1;
    }
    buttons
};
// SharpLib 0x110134 writes 16-byte rectangles into the 960-byte area at bar + 0x70.
pub const BAR_MAX_SLOTS: usize = 60;

#[derive(Clone, Copy)]
pub struct BarMeasurements {
    pub light: bool,
    pub two_columns: bool,
    pub selected: usize,
    pub slots: usize,
    pub buttons: [usize; BAR_MAX_SLOTS],
    pub handle: u32,
    pub items: [u32; BAR_MAX_SLOTS],
    pub call: &'static str,
    pub result: i32,
    pub flags: u32,
    pub events: u32,
    pub param: u16,
    pub data: [i32; 2],
}

impl Default for BarMeasurements {
    fn default() -> Self {
        Self {
            light: false,
            two_columns: false,
            selected: 0,
            slots: 3,
            buttons: core::array::from_fn(|i| i % BAR_BUTTONS.len()),
            handle: 0,
            items: [0; BAR_MAX_SLOTS],
            call: "IDLE",
            result: 0,
            flags: 0,
            events: 0,
            param: 0,
            data: [0; 2],
        }
    }
}

impl BarMeasurements {
    pub fn columns(&self) -> usize {
        if self.two_columns { 2 } else { 1 }
    }

    pub fn item_count(&self) -> usize {
        self.slots * self.columns()
    }

    pub fn toggle_columns(&mut self) {
        self.two_columns = !self.two_columns;
        self.slots = self.slots.min(BAR_MAX_SLOTS / self.columns());
        self.selected = self.selected.min(self.item_count() - 1);
    }

    pub fn toggle_light(&mut self) {
        self.light = !self.light;
        for button in &mut self.buttons {
            let name = BAR_BUTTONS[*button].0;
            let counterpart = BAR_BUTTONS.iter().position(|&(candidate, _)| {
                if self.light {
                    candidate.strip_prefix("LIGHT_") == Some(name)
                } else {
                    name.strip_prefix("LIGHT_") == Some(candidate)
                }
            });
            if let Some(index) = counterpart {
                *button = index;
            }
        }
    }

    pub fn shuffle(&mut self, all: bool, mut seed: u32) {
        let slots = if all {
            0..self.item_count()
        } else {
            self.selected..self.selected + 1
        };
        for slot in slots {
            if self.two_columns && slot == 0 {
                continue;
            }
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let step = 1 + (seed >> 16) as usize % (BAR_BUTTONS.len() - 1);
            self.buttons[slot] = (self.buttons[slot] + step) % BAR_BUTTONS.len();
        }
    }

    /// Returns true when a button image changes and the live bar needs rebuilding.
    pub fn edit_button(&mut self, key: u16) -> bool {
        use reflex::peg::key;
        match key {
            key::DOWN => {
                self.selected = (self.selected + self.item_count() - 1) % self.item_count()
            }
            key::UP => self.selected = (self.selected + 1) % self.item_count(),
            key::PAGE_UP | key::PAGE_DOWN => {
                let previous = self.slots;
                self.slots = if key == key::PAGE_UP {
                    (self.slots + 1).min(BAR_MAX_SLOTS / self.columns())
                } else {
                    self.slots.saturating_sub(1).max(1)
                };
                self.selected = self.selected.min(self.item_count() - 1);
                return self.slots != previous;
            }
            key::LEFT | key::RIGHT => {
                if self.two_columns && self.selected == 0 {
                    return false;
                }
                let step = if key == key::RIGHT {
                    1
                } else {
                    BAR_BUTTONS.len() - 1
                };
                self.buttons[self.selected] =
                    (self.buttons[self.selected] + step) % BAR_BUTTONS.len();
                return true;
            }
            _ => unreachable!(),
        }
        false
    }
}

#[derive(Clone, Copy, Default)]
pub struct AudioSnapshot {
    pub active: u32,
    pub cursors: [u32; 2],
    pub lengths: [u32; 2],
    pub bytes: u32,
    pub param: u16,
    pub data: [i32; 2],
    pub submit_us: u64,
}

#[derive(Clone, Copy)]
pub struct AudioMeasurements {
    pub buffer_bytes: u32,
    pub volume: u32,
    pub volume_ok: bool,
    pub call: &'static str,
    pub ok: bool,
    pub handle: u32,
    pub streaming: bool,
    pub state: u32,
    pub position_ms: u32,
    pub bytes: u32,
    pub status_ok: bool,
    pub events: u32,
    pub last_kind: u16,
    pub last_param: u16,
    pub last_data: [i32; 2],
    pub submitted: [u32; 2],
    pub staging: [u32; 2],
    pub cursors: [u32; 2],
    pub restarts: u32,
    pub active: u32,
    pub lengths: [u32; 2],
    pub submit_us: u64,
    pub max_submit_us: u64,
    pub initial: Option<AudioSnapshot>,
    pub stalled: Option<AudioSnapshot>,
}

impl Default for AudioMeasurements {
    fn default() -> Self {
        Self {
            buffer_bytes: 4096,
            volume: 0,
            volume_ok: true,
            call: "IDLE",
            ok: true,
            handle: 0,
            streaming: false,
            state: 0,
            position_ms: 0,
            bytes: 0,
            status_ok: false,
            events: 0,
            last_kind: 0,
            last_param: 0,
            last_data: [0; 2],
            submitted: [0; 2],
            staging: [0; 2],
            cursors: [0; 2],
            restarts: 0,
            active: 0,
            lengths: [0; 2],
            submit_us: 0,
            max_submit_us: 0,
            initial: None,
            stalled: None,
        }
    }
}

pub struct Canvas<'a> {
    pixels: &'a mut [u16],
    x: usize,
    y: usize,
    color: u16,
    scale: usize,
}
impl Canvas<'_> {
    fn pixel(&mut self, x: usize, y: usize, color: u16) {
        if x < WIDTH && y < HEIGHT {
            self.pixels[y * WIDTH + x] = color;
        }
    }
    fn at(&mut self, x: usize, y: usize, color: u16) {
        self.x = x;
        self.y = y;
        self.color = color;
    }
    fn rect(&mut self, label: &str, r: [i16; 4], y: usize, color: u16) {
        self.at(28, y, color);
        write!(
            self,
            "{label:10} {:4},{:4} - {:4},{:4}  {}X{}",
            r[0],
            r[1],
            r[2],
            r[3],
            i32::from(r[2]) - i32::from(r[0]) + 1,
            i32::from(r[3]) - i32::from(r[1]) + 1
        )
        .unwrap();
    }
}
impl Write for Canvas<'_> {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        let foreground = self.color;
        let background = foreground ^ 0xffff;
        for ch in text.chars() {
            let mut encoded = [0; 4];
            let x = self.x;
            let y = self.y;
            let scale = self.scale;
            // Sample the shared 2x bitmap at its native 6x13 resolution for dense views.
            reflex::text::draw(
                |dx, dy, color| {
                    if dx % 2 == 0 && dy % 2 == 0 && color != background {
                        for sy in 0..scale {
                            for sx in 0..scale {
                                self.pixel(x + dx / 2 * scale + sx, y + dy / 2 * scale + sy, color);
                            }
                        }
                    }
                },
                0,
                0,
                ch.encode_utf8(&mut encoded),
                foreground,
                background,
                1,
            );
            self.x += (reflex::text::CELL_WIDTH / 2) * scale;
        }
        Ok(())
    }
}

/// `edge` is the provisional exclusive right edge, not an OS measurement.
pub fn render(pixels: &mut [u16], m: &Measurements, edge: usize, input: &Input) {
    assert_eq!(pixels.len(), WIDTH * HEIGHT);
    assert!((1..=WIDTH).contains(&edge));
    pixels.fill(0x1082);
    let mut c = Canvas {
        pixels,
        x: 0,
        y: 0,
        color: 0xffff,
        scale: 2,
    };
    if input.mode == Mode::Bar {
        c.pixels.fill(0);
        c.at(28, 24, 0xffff);
        write!(c, "OPERATION BAR").unwrap();
        c.scale = 1;
        let b = &m.bar;
        c.at(500, 30, 0xffe0);
        write!(
            c,
            "L {}   O COLUMNS {}",
            if b.light { "LIGHT" } else { "DARK" },
            b.columns()
        )
        .unwrap();
        for (y, text) in [
            (
                70,
                "C CREATE   D DESTROY   S SHOW (0A)   T RANDOM   SHIFT+T ALL",
            ),
            (
                96,
                "Q  CONTROL 01    W  CONTROL 02    E  CONTROL 04    R  CONTROL 08",
            ),
            (122, "UP/DOWN  SELECT SLOT    LEFT/RIGHT  CHANGE BUTTON"),
        ] {
            c.at(28, y, 0xffe0);
            write!(c, "{text}").unwrap();
        }
        let start = b.selected / 9 * 9;
        let end = (start + 9).min(b.item_count());
        c.at(28, 142, 0xffe0);
        write!(
            c,
            "PAGE UP/DOWN  ROWS {} (1-{})    VIEW {}-{} / {}",
            b.slots,
            BAR_MAX_SLOTS / b.columns(),
            start + 1,
            end,
            b.item_count()
        )
        .unwrap();
        for (row, i) in (start..end).rev().enumerate() {
            c.at(
                28,
                166 + row * 16,
                if i == b.selected { 0xffe0 } else { 0xffff },
            );
            if b.two_columns && i == 0 {
                write!(
                    c,
                    "{} SLOT 1  OPEN / CLOSE",
                    if i == b.selected { "*" } else { " " }
                )
                .unwrap();
                continue;
            }
            let (name, image) = BAR_BUTTONS[b.buttons[i]];
            write!(
                c,
                "{} SLOT {}  {:32}  ",
                if i == b.selected { "*" } else { " " },
                i + 1,
                name
            )
            .unwrap();
            match image {
                Resource(normal, pressed) => write!(c, "{normal:04X}/{pressed:04X}"),
                RustLogo => write!(c, "RGB565 52X52"),
            }
            .unwrap();
        }
        c.at(28, 318, 0xffff);
        write!(
            c,
            "HANDLE {:08X}   ITEMS {}   SLOT {} ITEM {:08X}",
            b.handle,
            b.item_count(),
            b.selected + 1,
            b.items[b.selected]
        )
        .unwrap();
        c.at(28, 340, 0xffff);
        write!(
            c,
            "CALL {}   RESULT {}   LAST FLAGS {:02X}",
            b.call, b.result, b.flags
        )
        .unwrap();
        c.at(28, 362, 0x07e0);
        write!(
            c,
            "EVENTS {}   TYPE 010B   PARAM {:04X}   DATA {:?}",
            b.events, b.param, b.data
        )
        .unwrap();
        c.rect("WIN REAL", m.window_real, 386, 0xffff);
        c.rect("WIN CLIP", m.window_clip, 406, 0xffff);
        c.at(28, 446, 0xffe0);
        write!(
            c,
            "{}   LIST  SETTINGS   ESC  EXIT",
            if input.locked { "LOCKED" } else { "UNLOCKED" }
        )
        .unwrap();
        return;
    }
    if input.mode == Mode::Fpu {
        c.pixels.fill(0);
        c.at(28, 24, 0xffff);
        write!(c, "FPU TEST").unwrap();
        c.scale = 1;
        let f = &m.fpu;
        c.at(28, 70, 0xffe0);
        let result = match f.status {
            fpu::Status::Idle => "READY",
            fpu::Status::NoAccess => "NO ACCESS",
            fpu::Status::Unsupported => "UNSUPPORTED",
            fpu::Status::Pass => "PASS",
            fpu::Status::Fail => "FAIL",
        };
        write!(
            c,
            "{}  ENTER:RUN",
            if input.fpu_requested {
                "QUEUED"
            } else {
                result
            }
        )
        .unwrap();
        c.at(28, 100, 0xffff);
        write!(c, "(1.5 + 2.25) * 2 / 3 = 2.5   F64 LOOPS {}", fpu::ROUNDS).unwrap();
        if f.status != fpu::Status::Idle {
            c.at(28, 132, 0xffff);
            write!(c, "CPACR {:08X}", f.cpacr).unwrap();
        }
        if matches!(
            f.status,
            fpu::Status::Pass | fpu::Status::Fail | fpu::Status::Unsupported
        ) {
            c.at(28, 156, 0xffff);
            write!(
                c,
                "FPEXC BEFORE {:08X}   MVFR0 {:08X}",
                f.raw.fpexc, f.raw.mvfr0
            )
            .unwrap();
            c.at(28, 180, 0xffff);
            write!(
                c,
                "CPSR {:08X}   IRQ {}",
                f.raw.cpsr,
                if f.raw.cpsr & 0x80 == 0 {
                    "UNMASKED"
                } else {
                    "MASKED"
                }
            )
            .unwrap();
            c.at(28, 204, 0xffff);
            write!(
                c,
                "FPSCR BEFORE {:08X}   TEST {:08X}",
                f.raw.fpscr, f.raw.fpscr_after
            )
            .unwrap();
            c.at(28, 240, 0xffff);
            write!(c, "F32 {:08X}          EXPECT 40200000", f.raw.single).unwrap();
            c.at(28, 264, 0xffff);
            write!(c, "F64 {:016X}  EXPECT 4004000000000000", f.raw.double).unwrap();
            c.at(28, 300, 0xffff);
            write!(
                c,
                "D0-D31 BAD MASK {:08X}   EXPECT 00000000",
                f.bad_registers
            )
            .unwrap();
            c.at(28, 336, 0xffff);
            write!(c, "ELAPSED {} US", f.elapsed_us).unwrap();
        }
        status(&mut c, input);
        return;
    }
    if input.mode == Mode::Audio {
        c.scale = 1;
        c.pixels.fill(0);
        let a = &m.audio;
        c.at(28, 12, 0xffff);
        write!(c, "AUDIO 11025HZ/16/STEREO  Q:ONCE W:STREAM P:STOP").unwrap();
        c.at(28, 34, 0xffe0);
        write!(c, "LEFT/RIGHT INPUT {}  INTERNAL 4096", a.buffer_bytes).unwrap();
        c.at(28, 56, 0xffff);
        write!(
            c,
            "PAGEUP/DOWN VOL {}/9 {}  {} {}",
            a.volume,
            if a.volume_ok { "OK" } else { "FAIL" },
            a.call,
            if a.ok { "OK" } else { "FAIL" }
        )
        .unwrap();
        c.at(28, 78, 0xffff);
        write!(
            c,
            "STATE {} STATUS {} MS {} BYTE {}",
            a.state,
            if a.status_ok { "OK" } else { "FAIL" },
            a.position_ms,
            a.bytes
        )
        .unwrap();
        c.at(28, 100, 0xffff);
        write!(
            c,
            "ACTIVE {} CUR {}/{} {}/{}",
            a.active, a.cursors[0], a.lengths[0], a.cursors[1], a.lengths[1]
        )
        .unwrap();
        c.at(28, 122, 0xffff);
        write!(
            c,
            "EVENTS {} PARAM {:04X} DATA {} {}",
            a.events, a.last_param, a.last_data[0], a.last_data[1]
        )
        .unwrap();
        c.at(28, 144, 0xffff);
        write!(
            c,
            "SUBMIT {} {} US {} MAX {}",
            a.submitted[0], a.submitted[1], a.submit_us, a.max_submit_us
        )
        .unwrap();
        c.at(28, 166, 0xffff);
        write!(
            c,
            "STAGING {} {} RESTART {}",
            a.staging[0], a.staging[1], a.restarts
        )
        .unwrap();
        for (label, snapshot, y) in [("START", a.initial, 192), ("STALL", a.stalled, 292)] {
            c.at(28, y, 0xffe0);
            if let Some(s) = snapshot {
                write!(c, "{} ACTIVE {} BYTE {}", label, s.active, s.bytes).unwrap();
                c.at(28, y + 22, 0xffff);
                write!(
                    c,
                    "CUR {}/{} {}/{}",
                    s.cursors[0], s.lengths[0], s.cursors[1], s.lengths[1]
                )
                .unwrap();
                c.at(28, y + 44, 0xffff);
                write!(c, "PARAM {:04X} DATA {} {}", s.param, s.data[0], s.data[1]).unwrap();
                c.at(28, y + 66, 0xffff);
                write!(c, "SUBMIT US {}", s.submit_us).unwrap();
            } else {
                write!(c, "{} --", label).unwrap();
            }
        }
        status(&mut c, input);
        return;
    }
    for x in 0..WIDTH {
        for y in 0..HEIGHT {
            if x % 50 == 0 || y % 50 == 0 {
                c.pixel(x, y, 0x3186);
            }
            if x % 10 == 0 && !(12..HEIGHT - 12).contains(&y) {
                c.pixel(x, y, 0xffff);
            }
        }
    }
    for x in (0..WIDTH).step_by(100) {
        c.at(x + 2, 16, 0xffff);
        write!(c, "{x}").unwrap();
    }
    for y in (50..HEIGHT).step_by(50) {
        c.at(2, y + 2, 0xffff);
        write!(c, "{y}").unwrap();
    }
    for y in 0..HEIGHT {
        c.pixel(WIDTH / 2, y, 0x07ff);
        c.pixel(edge / 2, y, 0xffe0);
        c.pixel(edge - 1, y, 0xf81f);
    }
    for x in 0..WIDTH {
        c.pixel(x, HEIGHT / 2, 0x07ff);
    }
    if input.mode != Mode::Settings {
        if input.mode == Mode::Text {
            c.pixels.fill(0);
            c.at(28, 20, 0xffff);
            write!(c, "INSPECTOR / TEXT TEST").unwrap();
            for (scale, y) in [(1, 62), (2, 112), (3, 196)] {
                c.scale = scale;
                c.at(28, y, 0xffff);
                write!(c, "ABCDEFGHIJKLMNOPQRSTUVWXYZ").unwrap();
                c.at(28, y + 14 * scale, 0xffff);
                write!(c, "abcdefghijklmnopqrstuvwxyz").unwrap();
            }
            c.scale = 2;
            for (i, color) in [0xffff, 0xf800, 0x07e0, 0x001f, 0xffe0, 0x07ff, 0xf81f]
                .iter()
                .enumerate()
            {
                c.at(28 + i * 108, 290, *color);
                write!(c, "ABC 123").unwrap();
            }
            for y in 328..364 {
                for x in 22..780 {
                    c.pixel(x, y, 0xffff);
                }
            }
            c.at(28, 338, 0);
            write!(c, "BLACK ON WHITE  0123456789").unwrap();
        } else if input.mode == Mode::Keys {
            c.scale = 1;
            c.pixels.fill(0x0000);
            c.at(28, 18, 0xffff);
            write!(c, "KEY HISTORY - NEWEST FIRST").unwrap();
            for (i, event) in input.history[..input.count].iter().enumerate() {
                c.at(28, 46 + i * 22, 0xffff);
                write!(
                    c,
                    "{:2} KEY={:04X} TYPE={:2} FLAGS={:08X} SHIFT={}",
                    i + 1,
                    event.key,
                    event.kind,
                    event.flags,
                    u8::from(event.flags as u32 & reflex::peg::KMOD_SHIFT != 0)
                )
                .unwrap();
            }
        } else {
            let point = if input.mode == Mode::Pixels {
                Some(input.cursor)
            } else {
                m.touch.map(|(x, y)| (usize::from(x), usize::from(y)))
            };
            if let Some((x, y)) = point {
                let color = if input.mode == Mode::Pixels {
                    0xffff
                } else if m.touch_down {
                    0x07e0
                } else {
                    0xfd20
                };
                for px in 0..WIDTH {
                    c.pixel(px, y, color);
                }
                for py in 0..HEIGHT {
                    c.pixel(x, py, color);
                }
            }
            // Keep coordinate text away from the cursor, including at the panel edges.
            let label_y = if point.is_some_and(|(_, y)| y < HEIGHT / 2) {
                370
            } else {
                44
            };
            for y in label_y..label_y + 36 {
                for x in 22..780 {
                    c.pixel(x, y, 0);
                }
            }
            c.at(28, label_y + 4, 0xffff);
            match point {
                Some((x, y)) if input.mode == Mode::Pixels => {
                    write!(c, "PIXEL X={x} Y={y}  ARROWS:1 SHIFT:10")
                }
                Some((x, y)) => write!(
                    c,
                    "TOUCH {} X={x} Y={y}",
                    if m.touch_down { "DOWN" } else { "UP" }
                ),
                None => write!(c, "TOUCH --"),
            }
            .unwrap();
        }
        status(&mut c, input);
        return;
    }
    c.pixels.fill(0);
    c.scale = 1;
    c.at(28, 44, 0xffff);
    write!(c, "SETTINGS").unwrap();
    c.rect("SCREEN API", m.screen, 66, 0xffff);
    c.rect("PM REAL", m.pm_real, 84, 0x07e0);
    c.rect("PM CLIENT", m.pm_client, 102, 0x07e0);
    c.rect("PM CLIP", m.pm_clip, 120, 0x07e0);
    c.rect("WIN REAL", m.window_real, 138, 0xffff);
    c.rect("WIN CLIENT", m.window_client, 156, 0xffff);
    c.rect("WIN CLIP", m.window_clip, 174, 0xffff);
    if let Some(rect) = m.invalid {
        c.rect("DRAW RECT", rect, 192, 0xffff);
    } else {
        c.at(28, 192, 0xffff);
        write!(c, "DRAW RECT  --").unwrap();
    }
    c.at(28, 240, 0xffe0);
    write!(
        c,
        "WINDOW CLIP: {}",
        if input.window_clip { "ON" } else { "OFF" }
    )
    .unwrap();
    c.at(28, 270, 0xffff);
    write!(c, "LEFT:OFF RIGHT:ON").unwrap();
    c.at(28, 294, 0xffe0);
    write!(c, "F  FPU TEST     B  OPERATION BAR").unwrap();
    c.at(28, 318, 0xffe0);
    write!(c, "SHIFT P: PANIC TEST").unwrap();
    if input.panic_confirm {
        c.at(28, 350, 0xf800);
        write!(c, "PANIC? Y:YES N:NO").unwrap();
        c.at(28, 378, 0xffff);
        write!(c, "RESTART REQUIRED").unwrap();
    }
    status(&mut c, input);
}

fn status(c: &mut Canvas<'_>, input: &Input) {
    c.scale = 1;
    // Leave the pixel cursor visible at the bottom too.
    let top = if input.mode == Mode::Pixels && input.cursor.1 >= 450 {
        82
    } else {
        450
    };
    for y in top..top + 18 {
        for x in 22..780 {
            c.pixel(x, y, 0);
        }
    }
    let name = match input.mode {
        Mode::Text => "TEXT",
        Mode::Keys => "KEYS",
        Mode::Touch => "TOUCH",
        Mode::Pixels => "PIXELS",
        Mode::Settings => "SETTINGS",
        Mode::Audio => "AUDIO",
        Mode::Fpu => "FPU",
        Mode::Bar => "BAR",
    };
    c.at(28, top + 2, 0xffe0);
    write!(
        c,
        "{name} {} | SHIFT:HOLD 1S | HOME:TEXT JP:KEYS EN:TOUCH MY:PIXELS HISTORY:AUDIO LIST:SETTINGS ESC:EXIT",
        if input.locked { "LOCKED" } else { "UNLOCKED" }
    )
    .unwrap();
}
