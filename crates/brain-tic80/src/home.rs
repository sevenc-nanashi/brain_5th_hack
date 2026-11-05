use alloc::{format, string::String, vec::Vec};
use reflex::{Event, TouchPhase, display::WIDTH, peg::key, text};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub fn game_id(path: &[u8]) -> String {
    let hash = Sha256::digest(path.to_ascii_lowercase());
    let mut name = String::new();
    use core::fmt::Write;
    for byte in &hash[..16] {
        write!(name, "{byte:02x}").unwrap();
    }
    name
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Game {
    pub path: Vec<u8>,
    pub launches: u32,
}

pub enum Choice {
    None,
    Start(Vec<u8>),
    Browse,
    Exit,
}

pub struct Home {
    title: &'static str,
    default: &'static [u8],
    extension: &'static [u8],
    pub games: Vec<Game>,
    selected: usize,
    held: Option<u16>,
    pressed: Option<usize>,
    active: Option<Vec<u8>>,
    keymap: bool,
    pub notice: Option<&'static str>,
}

impl Home {
    pub fn new(title: &'static str, default: &'static [u8], extension: &'static [u8]) -> Self {
        Self {
            title,
            default,
            extension,
            games: Vec::new(),
            selected: 0,
            held: None,
            pressed: None,
            active: None,
            keymap: false,
            notice: None,
        }
    }
    pub fn decode(&mut self, bytes: &[u8]) -> Result<(), &'static str> {
        let games: Vec<Game> = serde_json::from_slice(bytes).map_err(|_| "History is damaged")?;
        if games.len() > 6 || games.iter().any(|game| !self.valid_path(&game.path)) {
            return Err("History is damaged");
        }
        self.games = games;
        self.selected = 0;
        Ok(())
    }
    fn valid_path(&self, path: &[u8]) -> bool {
        path == self.default
            || (path.len() < 260
                && !path.contains(&0)
                && (path.starts_with(b"SD0:\\") || path.starts_with(b"ROM:\\"))
                && path
                    .rsplit(|&b| b == b'.')
                    .next()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case(self.extension)))
    }
    pub fn record(&mut self, path: &[u8]) {
        let launches = self
            .games
            .iter()
            .find(|game| game.path == path)
            .map_or(1, |game| game.launches.saturating_add(1));
        self.games.retain(|game| game.path != path);
        self.games.insert(
            0,
            Game {
                path: path.to_vec(),
                launches,
            },
        );
        self.games.truncate(6);
        self.selected = 0;
    }
    pub fn encode(&self) -> Vec<u8> {
        serde_json::to_vec(&self.games).expect("serialize game history")
    }
    fn count(&self) -> usize {
        self.games.len() + usize::from(!self.games.iter().any(|game| game.path == self.default))
    }
    fn path(&self, index: usize) -> &[u8] {
        if index < self.games.len() {
            &self.games[index].path
        } else {
            self.default
        }
    }
    fn choose(&mut self, control: usize) -> Choice {
        match control {
            10 => Choice::Start(self.path(self.selected).to_vec()),
            11 => Choice::Browse,
            12 => {
                self.keymap = !self.keymap;
                Choice::None
            }
            13 => Choice::Exit,
            _ => Choice::None,
        }
    }
    fn hit(&self, x: u16, y: u16) -> Option<usize> {
        let (x, y) = (usize::from(x), usize::from(y));
        if self.keymap {
            return ((24..776).contains(&x) && (424..472).contains(&y)).then_some(12);
        }
        if (320..776).contains(&x) && (114..394).contains(&y) {
            let index = (y - 114) / 40;
            if index < self.count() {
                return Some(index);
            }
        }
        if (424..472).contains(&y) {
            for i in 0..4 {
                if (24 + i * 192..208 + i * 192).contains(&x) {
                    return Some(10 + i);
                }
            }
        }
        None
    }
    pub fn event(&mut self, event: Event) -> Choice {
        match event {
            Event::Key { code, down, .. } => {
                if !down {
                    if self.held == Some(code) {
                        self.held = None;
                    }
                    return Choice::None;
                }
                if self.held == Some(code) {
                    return Choice::None;
                }
                self.held = Some(code);
                if self.keymap {
                    if matches!(code, key::ESCAPE | key::ENTER | key::HOME | 0x48 | 0x68) {
                        self.keymap = false;
                    }
                    return Choice::None;
                }
                match code {
                    key::UP => self.selected = self.selected.saturating_sub(1),
                    key::DOWN => self.selected = (self.selected + 1).min(self.count() - 1),
                    key::ENTER => return self.choose(10),
                    key::SELECT_FROM_LIST => return Choice::Browse,
                    key::ESCAPE => return Choice::Exit,
                    0x48 | 0x68 => return self.choose(12),
                    _ => {}
                }
            }
            Event::Touch { x, y, phase } => {
                let hit = self.hit(x, y);
                match phase {
                    TouchPhase::Down => self.pressed = hit,
                    TouchPhase::Move => {
                        if hit != self.pressed {
                            self.pressed = None;
                        }
                    }
                    TouchPhase::Up => {
                        if let Some(control) = self.pressed.take().filter(|&p| Some(p) == hit) {
                            if control < 10 {
                                self.selected = control;
                            } else {
                                return self.choose(control);
                            }
                        }
                    }
                }
            }
        }
        Choice::None
    }
    pub fn reset_input(&mut self) {
        self.held = None;
        self.pressed = None;
    }
    pub fn set_active(&mut self, path: Option<&[u8]>) {
        self.active = path.map(|path| path.to_vec());
        if let Some(path) = path {
            self.selected = (0..self.count())
                .find(|&index| self.path(index) == path)
                .unwrap();
        }
        self.reset_input();
    }
    pub fn draw(&self, pixels: &mut [u16]) {
        const BG: u16 = 0x0863;
        const PANEL: u16 = 0x1906;
        const INK: u16 = 0xff9b;
        const MUTED: u16 = 0xad77;
        const GOLD: u16 = 0xfda8;
        pixels.fill(BG);
        rect(pixels, 24, 28, 752, 4, GOLD);
        label(pixels, 24, 44, self.title, INK, BG, 2);
        if self.keymap {
            label(pixels, 584, 57, "KEYMAP", GOLD, BG, 1);
            let rows = [
                ("Arrows", "Move: Up / Down / Left / Right"),
                ("Z / X / A / S", "Gamepad: A / B / X / Y"),
                ("A1 / symbols", "ABC / 123 / symbols"),
                ("Space / Enter / Del", "Space / Enter / Backspace"),
                ("Shift / symbols", "Keyboard input"),
                ("Touch / drag", "Mouse / Left button"),
                ("List", "Browse games"),
                ("History", "Debug menu"),
                ("My dictionary", "FPS on/off"),
                ("Home", "Save and return home"),
            ];
            for (row, (button, action)) in rows.iter().enumerate() {
                label(pixels, 40, 106 + row * 28, button, GOLD, BG, 1);
                label(pixels, 320, 106 + row * 28, action, INK, BG, 1);
            }
            label(
                pixels,
                40,
                390,
                "123: QWERTYUIOP = 1234567890",
                MUTED,
                BG,
                1,
            );
            let bg = if self.pressed == Some(12) {
                GOLD
            } else {
                PANEL
            };
            rect(pixels, 24, 424, 752, 48, bg);
            label(
                pixels,
                40,
                435,
                "ESC / ENTER  Back",
                if bg == GOLD { BG } else { INK },
                bg,
                1,
            );
            return;
        }
        label(pixels, 584, 57, "RECENT FIRST", MUTED, BG, 1);
        rect(pixels, 24, 114, 272, 280, PANEL);
        // Cartridge silhouette uses the same square pixel geometry as the type.
        rect(pixels, 86, 142, 148, 132, INK);
        rect(pixels, 104, 154, 112, 70, BG);
        label(
            pixels,
            116,
            172,
            if self.extension == b"tic" {
                "TIC-80"
            } else {
                "DOOM"
            },
            GOLD,
            BG,
            1,
        );
        rect(pixels, 112, 244, 96, 30, GOLD);
        for x in (120..204).step_by(12) {
            rect(pixels, x, 250, 4, 24, PANEL);
        }
        let selected = self.path(self.selected);
        label(pixels, 40, 296, &filename(selected, 20), INK, PANEL, 1);
        let plays = self
            .games
            .get(self.selected)
            .map_or(0, |game| game.launches);
        label(
            pixels,
            40,
            336,
            &format!("Played {plays}x"),
            MUTED,
            PANEL,
            1,
        );
        label(pixels, 320, 88, "SELECT A GAME", MUTED, BG, 1);
        for index in 0..self.count() {
            let selected = index == self.selected;
            let (fg, bg) = if selected { (BG, INK) } else { (INK, BG) };
            let y = 114 + index * 40;
            rect(pixels, 320, y, 456, 36, bg);
            if selected {
                rect(pixels, 332, y + 12, 8, 12, BG);
            }
            label(
                pixels,
                352,
                y + 5,
                &filename(self.path(index), 34),
                fg,
                bg,
                1,
            );
        }
        if let Some(notice) = self.notice {
            label(pixels, 24, 397, notice, GOLD, BG, 1);
        } else {
            let mut path = format!("{}", selected.escape_ascii());
            if path.len() > 62 {
                path = format!("...{}", &path[path.len() - 59..]);
            }
            label(pixels, 24, 397, &path, MUTED, BG, 1);
        }
        let play = if self.active.as_deref() == Some(selected) {
            "ENTER Resume"
        } else {
            "ENTER  Play"
        };
        for (index, name) in [play, "LIST Browse", "H Keymap", "ESC Exit"]
            .iter()
            .enumerate()
        {
            let bg = if self.pressed == Some(10 + index) {
                GOLD
            } else {
                PANEL
            };
            rect(pixels, 24 + index * 192, 424, 184, 48, bg);
            label(
                pixels,
                40 + index * 192,
                435,
                name,
                if bg == GOLD { BG } else { INK },
                bg,
                1,
            );
        }
    }
}

pub fn filename(path: &[u8], max: usize) -> String {
    let name = path.rsplit(|&b| b == b'\\' || b == b'/').next().unwrap();
    let mut name = format!("{}", name.escape_ascii());
    if name.len() > max {
        name.truncate(max - 3);
        name.push_str("...");
    }
    name
}
fn rect(pixels: &mut [u16], x: usize, y: usize, w: usize, h: usize, color: u16) {
    for row in y..y + h {
        pixels[row * WIDTH + x..row * WIDTH + x + w].fill(color);
    }
}
fn label(pixels: &mut [u16], x: usize, y: usize, value: &str, fg: u16, bg: u16, scale: usize) {
    text::draw(
        |x, y, c| pixels[y * WIDTH + x] = c,
        x,
        y,
        value,
        fg,
        bg,
        scale,
    );
}
