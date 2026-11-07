#![no_std]
#![no_main]
extern crate alloc;
use alloc::{ffi::CString, format, string::String, vec::Vec};
use brain_llm::{
    Model, Tokenizer,
    elan::{self, Elan},
};
use core::ffi::{CStr, c_void};
use reflex::{
    Action, Application, Event, Window,
    display::WIDTH,
    fs,
    peg::{self, key, raw},
    runtime, text,
};
use sha2::{Digest, Sha256};
const TIMER: u32 = 0x4c4d;
const LABELS: [&str; 2] = ["TinyStories 260K", "ElanMT tiny  English / Japanese"];
const FONT: &[u8] = include_bytes!("../assets/japanese.bin");
const MODEL_HASH: &[u8; 32] = include_bytes!(env!("ELAN_MODEL_HASH"));
const TOKENIZER_HASH: &[u8; 32] = include_bytes!(env!("ELAN_TOKENIZER_HASH"));
struct Llm {
    window: *mut c_void,
    base: Vec<u8>,
    menu: bool,
    selected: usize,
    file: Option<fs::File>,
    data: Vec<u8>,
    buffer: Vec<u8>,
    hash: Sha256,
    read: usize,
    size: usize,
    stage: u8,
    model: Option<Model>,
    tokenizer: Option<Tokenizer>,
    elan: Option<Elan>,
    elan_data: Vec<u8>,
    prompt: String,
    output: String,
    status: &'static str,
    tokens: Vec<usize>,
    pos: usize,
    current: usize,
    generated: usize,
    running: bool,
    started: u64,
    first: u64,
    elapsed: u64,
    scroll: usize,
    last_draw: u64,
    enter_down: bool,
    esc_down: bool,
}
#[reflex::main]
fn main() -> Llm {
    Llm {
        window: core::ptr::null_mut(),
        base: Vec::new(),
        menu: true,
        selected: 1,
        file: None,
        data: Vec::new(),
        buffer: Vec::new(),
        hash: Sha256::new(),
        read: 0,
        size: 0,
        stage: 3,
        model: None,
        tokenizer: None,
        elan: None,
        elan_data: Vec::new(),
        prompt: String::new(),
        output: String::new(),
        status: "",
        tokens: Vec::new(),
        pos: 0,
        current: 1,
        generated: 0,
        running: false,
        started: 0,
        first: 0,
        elapsed: 0,
        scroll: 0,
        last_draw: 0,
        enter_down: false,
        esc_down: false,
    }
}
impl Llm {
    fn path(&self, name: &[u8]) -> CString {
        let mut p = self.base.clone();
        p.extend_from_slice(name);
        CString::new(p).unwrap()
    }
    fn release(&mut self) {
        self.running = false;
        self.file = None;
        self.model = None;
        self.tokenizer = None;
        self.elan = None;
        self.elan_data = Vec::new();
        self.data = Vec::new();
        self.buffer = Vec::new();
        self.tokens = Vec::new();
        self.hash = Sha256::new();
        self.read = 0;
        self.size = 0;
        self.stage = 3;
    }
    fn select(&mut self) -> Result<(), &'static str> {
        self.release();
        self.menu = false;
        self.output.clear();
        self.scroll = 0;
        self.generated = 0;
        self.first = 0;
        self.elapsed = 0;
        self.prompt = String::from(if self.selected == 0 {
            "Once upon a time"
        } else {
            "Hello."
        });
        self.buffer
            .try_reserve_exact(65536)
            .map_err(|_| "OUT OF MEMORY")?;
        self.buffer.resize(65536, 0);
        self.stage = 0;
        self.status = "LOADING";
        Ok(())
    }
    fn fail(&mut self, e: &'static str) {
        self.release();
        self.status = e;
    }
    fn load(&mut self) -> Result<(), &'static str> {
        let (name, expected) = match (self.selected == 0, self.stage) {
            (true, 0) => (b"stories260K.bin".as_slice(), 1056540),
            (true, _) => (b"tok512.bin".as_slice(), 6227),
            (false, 0) => (b"elan.bin".as_slice(), 16536516),
            (false, _) => (b"elan.tok".as_slice(), 649650),
        };
        if self.file.is_none() {
            let path = self.path(name);
            self.size = unsafe { fs::metadata(&path) }
                .map_err(|_| "MODEL FILE MISSING")?
                .size as usize;
            if self.size != expected {
                return Err("INVALID FILE SIZE");
            }
            self.read = 0;
            self.hash = Sha256::new();
            self.data
                .try_reserve_exact(self.size)
                .map_err(|_| "OUT OF MEMORY")?;
            self.file = Some(unsafe { fs::File::open(&path) }.ok_or("OPEN FAILED")?);
        }
        let n = self
            .file
            .as_mut()
            .unwrap()
            .read(&mut self.buffer)
            .map_err(|_| "READ FAILED")?;
        if n != 0 {
            self.read += n;
            if self.read > self.size {
                return Err("FILE CHANGED");
            }
            if self.selected != 0 {
                self.hash.update(&self.buffer[..n]);
            }
            self.data.extend_from_slice(&self.buffer[..n]);
            return Ok(());
        }
        if self.read != self.size {
            return Err("TRUNCATED FILE");
        }
        self.file
            .take()
            .unwrap()
            .close()
            .map_err(|_| "CLOSE FAILED")?;
        if self.selected == 0 {
            if self.stage == 0 {
                self.model = Some(Model::load(&self.data)?);
            } else {
                self.tokenizer = Some(Tokenizer::load(&self.data)?);
            }
            self.data = Vec::new();
        } else {
            let expected = if self.stage == 0 {
                MODEL_HASH
            } else {
                TOKENIZER_HASH
            };
            if self.hash.clone().finalize().as_slice() != expected {
                return Err("CHECKSUM MISMATCH");
            }
            if self.stage == 0 {
                self.elan_data = core::mem::take(&mut self.data);
            } else {
                self.elan = Some(Elan::load(
                    core::mem::take(&mut self.elan_data),
                    core::mem::take(&mut self.data),
                )?);
            }
        }
        self.stage += 1;
        if self.stage == 2 {
            self.buffer = Vec::new();
            self.status = "READY";
        }
        Ok(())
    }
    fn start(&mut self) -> Result<(), &'static str> {
        self.started = unsafe { runtime::micros() };
        if let Some(m) = &mut self.elan {
            m.start(&self.prompt)?;
        } else {
            self.tokens = self.tokenizer.as_ref().unwrap().encode(&self.prompt);
            self.model.as_mut().unwrap().reset();
            self.current = self.tokens[0];
        }
        self.output.clear();
        self.pos = 0;
        self.generated = 0;
        self.first = 0;
        self.elapsed = 0;
        self.scroll = 0;
        self.running = true;
        self.status = "GENERATING";
        Ok(())
    }
    fn step(&mut self) -> Result<(), &'static str> {
        let predicted = if let Some(m) = &mut self.elan {
            let Some(t) = m.tick()? else {
                self.elapsed = unsafe { runtime::micros() } - self.started;
                return Ok(());
            };
            t
        } else {
            self.model.as_mut().unwrap().forward(self.current, self.pos)
        };
        self.pos += 1;
        if self.selected == 0 && self.pos < self.tokens.len() {
            self.current = self.tokens[self.pos];
        } else {
            let end = if self.selected == 0 {
                predicted == 1
            } else {
                predicted == 0
            };
            if end {
                self.running = false;
                self.status = "DONE";
            } else {
                if let Some(m) = &self.elan {
                    self.output
                        .push_str(&m.tokenizer.decode(predicted, self.output.is_empty()));
                } else {
                    self.output.push_str(
                        &self
                            .tokenizer
                            .as_ref()
                            .unwrap()
                            .decode(self.current, predicted),
                    );
                }
                self.current = predicted;
                self.generated += 1;
                if self.generated == 1 {
                    self.first = unsafe { runtime::micros() } - self.started;
                }
                self.scroll = lines(&self.output).saturating_sub(9);
            }
        }
        self.elapsed = unsafe { runtime::micros() } - self.started;
        if self.pos >= if self.selected == 0 { 512 } else { elan::LIMIT } || self.generated >= 128 {
            self.running = false;
            self.status = "LIMIT";
        }
        Ok(())
    }
}
impl Application for Llm {
    fn mounted(&mut self, w: Window) -> Action {
        self.window = w.raw;
        let path = unsafe { CStr::from_ptr(w.exec_path.cast()) }.to_bytes();
        if let Some(i) = path.iter().rposition(|&b| b == b'\\' || b == b'/') {
            self.base.extend_from_slice(&path[..=i]);
            unsafe {
                raw::set_timer(w.raw, TIMER, 1, 1);
            }
        } else {
            self.menu = false;
            self.status = "INVALID APP PATH";
        }
        Action::Redraw
    }
    fn raw_event(&mut self, m: &peg::Message) -> Option<Action> {
        if m.kind != peg::PM_TIMER || m.param != TIMER as u16 {
            return None;
        }
        if self.menu {
            return Some(Action::None);
        }
        if self.stage < 2 {
            if let Err(e) = self.load() {
                self.fail(e);
            }
        } else if self.running {
            let started = unsafe { runtime::micros() };
            loop {
                if let Err(e) = self.step() {
                    self.fail(e);
                    break;
                }
                if !self.running
                    || self.selected == 0
                    || unsafe { runtime::micros() } - started >= 8000
                {
                    break;
                }
            }
        } else {
            return Some(Action::None);
        }
        let now = unsafe { runtime::micros() };
        if self.stage >= 2 && !self.running || now - self.last_draw >= 100000 {
            self.last_draw = now;
            Some(Action::Redraw)
        } else {
            Some(Action::None)
        }
    }
    fn event(&mut self, e: Event) -> Action {
        let Event::Key { code, down, flags } = e else {
            return Action::None;
        };
        if !down {
            if code == key::ENTER {
                self.enter_down = false;
            }
            if code == key::ESCAPE {
                self.esc_down = false;
            }
            return Action::None;
        }
        if code == key::ESCAPE {
            if self.esc_down {
                return Action::None;
            }
            self.esc_down = true;
            if self.menu {
                return Action::Exit;
            }
            if self.running {
                self.running = false;
                self.status = "STOPPED";
            } else {
                self.release();
                self.menu = true;
            }
            return Action::Redraw;
        }
        if self.menu {
            if code == key::UP {
                self.selected = self.selected.saturating_sub(1);
            }
            if code == key::DOWN {
                self.selected = (self.selected + 1).min(LABELS.len() - 1);
            }
            if code == key::ENTER && !self.enter_down {
                self.enter_down = true;
                if let Err(e) = self.select() {
                    self.fail(e);
                }
            }
            return Action::Redraw;
        }
        if code == key::UP {
            self.scroll = self.scroll.saturating_sub(1);
            return Action::Redraw;
        }
        if code == key::DOWN {
            self.scroll = (self.scroll + 1).min(lines(&self.output).saturating_sub(9));
            return Action::Redraw;
        }
        if code == key::ENTER {
            if self.enter_down {
                return Action::None;
            }
            self.enter_down = true;
            if self.stage == 2
                && !self.running
                && let Err(e) = self.start()
            {
                self.status = e;
            }
            return Action::Redraw;
        }
        if self.running || self.stage != 2 {
            return Action::None;
        }
        if code == key::BACKSPACE {
            if flags & peg::KMOD_SHIFT != 0 {
                self.prompt.clear();
            } else {
                self.prompt.pop();
            }
            return Action::Redraw;
        }
        if (32..=126).contains(&code) && self.prompt.len() < 128 {
            let c = code as u8 as char;
            self.prompt.push(if flags & peg::KMOD_SHIFT != 0 {
                c.to_ascii_uppercase()
            } else {
                c.to_ascii_lowercase()
            });
            return Action::Redraw;
        }
        Action::None
    }
    fn can_exit(&mut self) -> bool {
        unsafe {
            raw::kill_timer(self.window, TIMER);
        }
        self.release();
        true
    }
    fn draw(&self, p: &mut [u16]) {
        p.fill(0x0841);
        if self.menu {
            put(p, 24, 16, "MODEL", 0x07ff);
            for (i, label) in LABELS.iter().enumerate() {
                let color = if i == self.selected { 0xffe0 } else { 0xffff };
                if i == self.selected {
                    put(p, 24, 90 + i * 58, "*", color);
                }
                put(p, 60, 90 + i * 58, label, color);
            }
            put(
                p,
                24,
                446,
                "UP/DOWN  Select    ENTER  Load    ESC  Exit",
                0xffe0,
            );
            return;
        }
        put(p, 24, 16, LABELS[self.selected], 0x07ff);
        let status = if self.stage < 2 {
            format!(
                "{}  {} / {} KB",
                self.status,
                self.read / 1024,
                self.size / 1024
            )
        } else if self.running && self.elan.is_some() {
            let (phase, i, n) = self.elan.as_ref().unwrap().progress();
            format!("{}  {}/{}  {}s", phase, i, n, self.elapsed / 1000000)
        } else {
            String::from(self.status)
        };
        put(p, 24, 48, &status, 0xffe0);
        for (i, part) in self.prompt.as_bytes().chunks(62).enumerate() {
            put(
                p,
                24,
                82 + i * 26,
                core::str::from_utf8(part).unwrap(),
                0xffff,
            );
        }
        let (mut line, mut x) = (0, 0);
        for c in self.output.chars() {
            if c == '\n' {
                line += 1;
                x = 0;
                continue;
            }
            let width = if c.is_ascii() { 12 } else { 16 };
            if x + width > 744 {
                line += 1;
                x = 0;
            }
            if line >= self.scroll && line < self.scroll + 9 {
                glyph(p, 24 + x, 170 + (line - self.scroll) * 26, c);
            }
            x += width;
        }
        let bytes = if let Some(m) = &self.elan {
            m.allocated_bytes()
        } else {
            self.model.as_ref().map_or(0, Model::allocated_bytes)
        };
        let rate = if self.elapsed > self.first {
            self.generated.saturating_sub(1) as u64 * 10000000 / (self.elapsed - self.first)
        } else {
            0
        };
        put(
            p,
            24,
            412,
            &format!(
                "TOK {}  {}.{}/S  FIRST {}MS  WORK {}KB",
                self.generated,
                rate / 10,
                rate % 10,
                self.first / 1000,
                bytes / 1024
            ),
            0x07ff,
        );
        put(
            p,
            24,
            446,
            "ENTER  Generate    ESC  Stop/Models    UP/DOWN  Scroll",
            0xffe0,
        );
    }
}
fn put(p: &mut [u16], x: usize, y: usize, s: &str, color: u16) {
    text::draw(|x, y, c| p[y * WIDTH + x] = c, x, y, s, color, 0x0841, 1);
}
fn lines(s: &str) -> usize {
    let (mut lines, mut x) = (1, 0);
    for c in s.chars() {
        if c == '\n' {
            lines += 1;
            x = 0;
        } else {
            let w = if c.is_ascii() { 12 } else { 16 };
            if x + w > 744 {
                lines += 1;
                x = 0;
            }
            x += w;
        }
    }
    lines
}
fn glyph(p: &mut [u16], x: usize, y: usize, c: char) {
    if c.is_ascii() {
        let mut b = [0; 4];
        put(p, x, y, c.encode_utf8(&mut b), 0xffff);
        return;
    }
    let code = c as usize;
    let index = match code {
        0x3000..=0x30ff => Some(code - 0x3000),
        0x3400..=0x9fff => Some(256 + code - 0x3400),
        0xff00..=0xffef => Some(256 + 0x6c00 + code - 0xff00),
        _ => None,
    };
    if let Some(i) = index {
        for dy in 0..16 {
            let row = u16::from_be_bytes([FONT[i * 32 + dy * 2], FONT[i * 32 + dy * 2 + 1]]);
            for dx in 0..16 {
                if row & (0x8000 >> dx) != 0 {
                    p[(y + 5 + dy) * WIDTH + x + dx] = 0xffff;
                }
            }
        }
    } else {
        put(p, x, y, "?", 0xffff);
    }
}
