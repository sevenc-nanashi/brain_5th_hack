extern crate alloc;
use alloc::format;
use reflex::{display::WIDTH, text};

#[derive(Default)]
pub struct Performance {
    since: u64,
    first_frame: u64,
    last_drawn: u64,
    draws: u32,
    pub fps: u32,
    pub emulation_fps: u32,
}
impl Performance {
    pub fn reset(&mut self, now: u64, frame: u64) {
        *self = Self {
            since: now,
            first_frame: frame,
            last_drawn: frame,
            ..Self::default()
        };
    }
    pub fn rendered(&mut self, frame: u64) {
        if frame != self.last_drawn {
            self.draws += 1;
            self.last_drawn = frame;
        }
    }
    pub fn has_new_frame(&self, frame: u64) -> bool {
        frame != self.last_drawn
    }
    pub fn sample(&mut self, now: u64, frame: u64) -> bool {
        let elapsed = now.wrapping_sub(self.since);
        if elapsed < 1_000_000 {
            return false;
        }
        self.fps = ((u64::from(self.draws) * 1_000_000 + elapsed / 2) / elapsed) as u32;
        self.emulation_fps =
            ((frame.saturating_sub(self.first_frame) * 1_000_000 + elapsed / 2) / elapsed) as u32;
        self.since = now;
        self.first_frame = frame;
        self.draws = 0;
        true
    }
}

#[derive(Default)]
pub struct Audio {
    pub fill_us: u64,
    pub max_fill_us: u64,
    pub submits: u32,
    pub restarts: u32,
    pub state: Option<u32>,
}

pub fn hud(pixels: &mut [u16], performance: &Performance) {
    let label = format!(
        "FPS {}   EMU {}",
        performance.fps, performance.emulation_fps
    );
    text::draw(
        |x, y, c| {
            if x % 2 == 0 && y % 2 == 0 {
                pixels[(4 + y / 2) * WIDTH + 16 + x / 2] = c;
            }
        },
        0,
        0,
        &label,
        0xffff,
        0,
        1,
    );
}

pub fn menu(pixels: &mut [u16], performance: &Performance, audio: &Audio, show_fps: bool) {
    for y in 60..420 {
        pixels[y * WIDTH + 112..y * WIDTH + 688].fill(0x18e3);
    }
    let state = match audio.state {
        Some(value) => format!("{value}"),
        None => "OFF".into(),
    };
    let lines = [
        "TIC-80 DEBUG".into(),
        format!("Display FPS       {}", performance.fps),
        format!("Emulation FPS     {}", performance.emulation_fps),
        format!("Audio fill        {} us", audio.fill_us),
        format!("Audio fill max    {} us", audio.max_fill_us),
        format!("Audio submits     {}", audio.submits),
        format!("Audio restarts    {}", audio.restarts),
        format!("Audio state       {state}"),
        format!(
            "My dictionary     FPS {}",
            if show_fps { "ON" } else { "OFF" }
        ),
        "R                 Reset counters".into(),
        "History / Esc     Close".into(),
    ];
    for (row, line) in lines.iter().enumerate() {
        text::draw(
            |x, y, c| pixels[y * WIDTH + x] = c,
            136,
            78 + row * 29,
            line,
            0xffff,
            0x18e3,
            1,
        );
    }
}
