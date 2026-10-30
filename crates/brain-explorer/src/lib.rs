#![no_std]
extern crate alloc;

use alloc::{format, string::String, vec::Vec};
use core::fmt::Write;
use reflex::{
    Action, Application, Event, TouchPhase,
    display::{HEIGHT, WIDTH},
    peg::key,
};
mod paint;

pub const MAX_ENTRIES: usize = 4096;
const LIST_ROWS: usize = 9;
const TEXT_ROWS: usize = 12;
const TEXT_COLUMNS: usize = 64;
const HEX_BYTES: usize = 256;

pub struct Entry {
    pub name: Vec<u8>,
    pub directory: bool,
    pub size: u32,
}

/// Both the device and host preview read real directories through this boundary.
pub trait Filesystem: 'static {
    fn read_dir(&mut self, path: &[u8]) -> Result<Vec<Entry>, &'static str>;
    /// Fill the buffer, stopping early only at EOF; propagate I/O failures.
    fn read(&mut self, path: &[u8], offset: u32, buffer: &mut [u8]) -> Result<usize, &'static str>;
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Control {
    Row(usize),
    Back,
    Previous,
    Next,
    Open,
    Toggle,
    Refresh,
    Send,
}

struct Viewer {
    name: Vec<u8>,
    size: u32,
    offset: u32,
    next: Option<u32>,
    history: Vec<u32>,
    hex: bool,
    lines: Vec<String>,
}

pub struct Explorer<F> {
    fs: F,
    folders: Vec<Vec<u8>>,
    entries: Vec<Entry>,
    selected: usize,
    top: usize,
    viewer: Option<Viewer>,
    error: Option<&'static str>,
    keys_down: [u32; 32],
    pressed: Option<Control>,
    send_requested: bool,
}

impl<F: Filesystem> Explorer<F> {
    pub fn new(fs: F) -> Self {
        Self {
            fs,
            folders: Vec::new(),
            entries: roots(),
            selected: 0,
            top: 0,
            viewer: None,
            error: None,
            keys_down: [0; 32],
            pressed: None,
            send_requested: false,
        }
    }

    fn send_target(&self) -> Option<(Vec<u8>, u32, bool)> {
        if let Some(viewer) = &self.viewer {
            Some((self.path(Some(&viewer.name)), viewer.size, false))
        } else {
            self.entries
                .get(self.selected)
                .map(|entry| (self.path(Some(&entry.name)), entry.size, entry.directory))
        }
    }

    /// Consume the device UI's send request, preserving the current folder/viewer.
    pub fn take_send_target(&mut self) -> Option<(Vec<u8>, u32, bool)> {
        if !core::mem::take(&mut self.send_requested) {
            return None;
        }
        self.keys_down.fill(0);
        self.pressed = None;
        self.send_target()
    }

    fn path(&self, name: Option<&[u8]>) -> Vec<u8> {
        let mut path = Vec::new();
        for folder in &self.folders {
            path.extend_from_slice(folder);
            path.push(b'\\');
        }
        if let Some(name) = name {
            path.extend_from_slice(name);
        }
        path
    }

    fn load_dir(&mut self, path: &[u8]) -> Result<Vec<Entry>, &'static str> {
        let mut entries = self.fs.read_dir(path)?;
        entries.retain(|entry| entry.name != b"." && entry.name != b"..");
        entries.sort_unstable_by(|a, b| b.directory.cmp(&a.directory).then(a.name.cmp(&b.name)));
        Ok(entries)
    }

    fn open(&mut self) -> Result<(), &'static str> {
        let Some(entry) = self.entries.get(self.selected) else {
            return Ok(());
        };
        let name = entry.name.clone();
        let mut path = self.path(Some(&name));
        if entry.directory {
            path.push(b'\\');
            let entries = self.load_dir(&path)?;
            self.folders.push(name);
            self.entries = entries;
            self.selected = 0;
            self.top = 0;
        } else {
            let (lines, next) = self.page(&path, 0, false)?;
            self.viewer = Some(Viewer {
                name,
                size: self.entries[self.selected].size,
                offset: 0,
                next,
                history: Vec::new(),
                hex: false,
                lines,
            });
        }
        Ok(())
    }

    fn page(
        &mut self,
        path: &[u8],
        offset: u32,
        hex: bool,
    ) -> Result<(Vec<String>, Option<u32>), &'static str> {
        // A full text page can consume a CRLF after every 64-column row.
        let mut buffer = [0; TEXT_ROWS * (TEXT_COLUMNS + 2) + 1];
        let capacity = if hex { HEX_BYTES + 1 } else { buffer.len() };
        let count = self.fs.read(path, offset, &mut buffer[..capacity])?;
        let data = &buffer[..count];
        let (lines, consumed) = if hex {
            let consumed = count.min(HEX_BYTES);
            let lines = data[..consumed]
                .chunks(16)
                .enumerate()
                .map(|(row, bytes)| {
                    let mut line = format!("{:08X}  ", offset + row as u32 * 16);
                    for index in 0..16 {
                        if let Some(byte) = bytes.get(index) {
                            write!(line, "{byte:02X} ").unwrap();
                        } else {
                            line.push_str("   ");
                        }
                        if index == 7 {
                            line.push(' ');
                        }
                    }
                    line.push(' ');
                    for byte in bytes {
                        line.push(if (32..=126).contains(byte) {
                            *byte as char
                        } else {
                            '.'
                        });
                    }
                    line
                })
                .collect();
            (lines, consumed)
        } else {
            text_page(data)
        };
        let next = if consumed < count {
            Some(
                offset
                    .checked_add(consumed as u32)
                    .ok_or("File offset overflow")?,
            )
        } else {
            None
        };
        Ok((lines, next))
    }

    fn back(&mut self) -> Result<Action, &'static str> {
        if self.viewer.take().is_some() {
            return Ok(Action::Redraw);
        }
        if self.folders.is_empty() {
            return Ok(Action::Exit);
        }
        let previous = self.folders.pop().unwrap();
        let result = if self.folders.is_empty() {
            Ok(roots())
        } else {
            self.load_dir(&self.path(None))
        };
        match result {
            Ok(entries) => {
                self.selected = 0;
                if let Some(index) = entries.iter().position(|entry| entry.name == previous) {
                    self.selected = index;
                }
                self.entries = entries;
                self.top = (self.selected / LIST_ROWS * LIST_ROWS)
                    .min(self.entries.len().saturating_sub(LIST_ROWS));
                Ok(Action::Redraw)
            }
            Err(error) => {
                self.folders.push(previous);
                Err(error)
            }
        }
    }

    fn move_selection(&mut self, delta: isize) {
        self.selected = self
            .selected
            .saturating_add_signed(delta)
            .min(self.entries.len().saturating_sub(1));
        if self.selected < self.top {
            self.top = self.selected;
        }
        if self.selected >= self.top + LIST_ROWS {
            self.top = self.selected + 1 - LIST_ROWS;
        }
        self.top = self.top.min(self.entries.len().saturating_sub(LIST_ROWS));
    }

    fn control(&mut self, control: Control) -> Result<Action, &'static str> {
        if control == Control::Send {
            if cfg!(feature = "device") && self.send_target().is_some() {
                self.send_requested = true;
            }
            return Ok(Action::Redraw);
        }
        if control == Control::Back {
            return self.back();
        }
        if let Some(viewer) = &self.viewer {
            let (offset, hex) = match control {
                Control::Next => {
                    let Some(next) = viewer.next else {
                        return Ok(Action::None);
                    };
                    (next, viewer.hex)
                }
                Control::Previous => {
                    let Some(previous) = viewer.history.last() else {
                        return Ok(Action::None);
                    };
                    (*previous, viewer.hex)
                }
                Control::Toggle => (viewer.offset, !viewer.hex),
                Control::Refresh => (viewer.offset, viewer.hex),
                _ => return Ok(Action::None),
            };
            let path = self.path(Some(&viewer.name));
            let (lines, next) = self.page(&path, offset, hex)?;
            let viewer = self.viewer.as_mut().unwrap();
            if control == Control::Next {
                viewer.history.push(viewer.offset);
            }
            if control == Control::Previous {
                viewer.history.pop();
            }
            viewer.offset = offset;
            viewer.hex = hex;
            viewer.lines = lines;
            viewer.next = next;
        } else {
            match control {
                Control::Row(index) => {
                    self.selected = index;
                    self.open()?;
                }
                Control::Open => self.open()?,
                Control::Previous => self.move_selection(-(LIST_ROWS as isize)),
                Control::Next => self.move_selection(LIST_ROWS as isize),
                Control::Refresh if !self.folders.is_empty() => {
                    self.entries = self.load_dir(&self.path(None))?;
                    self.move_selection(0);
                }
                _ => {}
            }
        }
        Ok(Action::Redraw)
    }

    fn activate(&mut self, control: Control) -> Action {
        self.error = None;
        match self.control(control) {
            Ok(action) => action,
            Err(error) => {
                self.error = Some(error);
                Action::Redraw
            }
        }
    }

    fn hit(&self, x: usize, y: usize) -> Option<Control> {
        if cfg!(feature = "device")
            && (440..608).contains(&x)
            && (4..48).contains(&y)
            && self.send_target().is_some()
        {
            return Some(Control::Send);
        }
        if !(16..784).contains(&x) {
            return None;
        }
        if (426..474).contains(&y) {
            return Some(match (x - 16) / 192 {
                0 => Control::Back,
                1 => Control::Previous,
                2 => Control::Next,
                _ if self.viewer.is_some() => Control::Toggle,
                _ => Control::Open,
            });
        }
        if self.viewer.is_none() && (92..380).contains(&y) {
            let index = self.top + (y - 92) / 32;
            if index < self.entries.len() {
                return Some(Control::Row(index));
            }
        }
        None
    }
}

impl<F: Filesystem> Application for Explorer<F> {
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
                if self.viewer.is_none() && matches!(code, key::UP | key::DOWN) {
                    self.move_selection(if code == key::UP { -1 } else { 1 });
                    return Action::Redraw;
                }
                let control = match code {
                    key::ESCAPE | key::BACKSPACE | key::LEFT => Control::Back,
                    key::ENTER | key::RIGHT => Control::Open,
                    key::PAGE_UP | key::UP => Control::Previous,
                    key::PAGE_DOWN | key::DOWN => Control::Next,
                    0x54 | 0x74 => Control::Toggle,
                    0x52 | 0x72 => Control::Refresh,
                    0x53 | 0x73 => Control::Send,
                    _ => return Action::None,
                };
                self.activate(control)
            }
            Event::Touch { x, y, phase } => {
                let hit = self.hit(x as usize, y as usize);
                match phase {
                    TouchPhase::Down => {
                        self.pressed = hit;
                        Action::Redraw
                    }
                    TouchPhase::Move => {
                        if self.pressed != hit {
                            self.pressed = None;
                        }
                        Action::Redraw
                    }
                    TouchPhase::Up => {
                        let pressed = self.pressed.take();
                        if let Some(control) = pressed
                            && pressed == hit
                        {
                            self.activate(control)
                        } else {
                            Action::Redraw
                        }
                    }
                }
            }
        }
    }
    fn draw(&self, pixels: &mut [u16]) {
        assert_eq!(pixels.len(), WIDTH * HEIGHT);
        paint::draw(self, pixels);
    }
}

fn roots() -> Vec<Entry> {
    [b"SD0:".as_slice(), b"ROM:"]
        .into_iter()
        .map(|name| Entry {
            name: name.to_vec(),
            directory: true,
            size: 0,
        })
        .collect()
}

/// Escape bytes without changing the pathname sent to the filesystem.
fn escaped(bytes: &[u8]) -> String {
    let mut text = String::new();
    for byte in bytes {
        if (32..=126).contains(byte) {
            text.push(*byte as char);
        } else {
            write!(text, "\\x{byte:02X}").unwrap();
        }
    }
    text
}

fn text_page(data: &[u8]) -> (Vec<String>, usize) {
    let mut lines = Vec::new();
    let mut line = String::new();
    let mut consumed = 0;
    for (index, byte) in data.iter().enumerate() {
        if lines.len() == TEXT_ROWS {
            break;
        }
        if *byte == b'\n' {
            lines.push(core::mem::take(&mut line));
        } else if *byte == b'\r' && data.get(index + 1) == Some(&b'\n') {
            // CRLF occupies one line break; lone CR remains visible as an escape.
        } else {
            let value = if *byte == b'\t' {
                " ".repeat(4 - line.len() % 4)
            } else {
                escaped(&[*byte])
            };
            if line.len() + value.len() > TEXT_COLUMNS {
                lines.push(core::mem::take(&mut line));
                if lines.len() == TEXT_ROWS {
                    break;
                }
            }
            line.push_str(&value);
        }
        consumed = index + 1;
    }
    if !line.is_empty() {
        lines.push(line);
    }
    (lines, consumed)
}
