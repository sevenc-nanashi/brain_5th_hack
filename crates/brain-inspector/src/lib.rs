#![no_std]
pub mod input;
use core::fmt::{self, Write};
use input::{Input, Mode};
use reflex::display::{HEIGHT, WIDTH};

#[derive(Clone, Copy, Default)]
pub struct Measurements {
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
    };
    c.at(28, top + 2, 0xffe0);
    write!(
        c,
        "{name} {} | SHIFT:HOLD 1S | HOME:TEXT JP:KEYS EN:TOUCH MY:PIXELS HISTORY:AUDIO LIST:SETTINGS ESC:EXIT",
        if input.locked { "LOCKED" } else { "UNLOCKED" }
    )
    .unwrap();
}
