use super::*;
use alloc::collections::VecDeque;

pub(super) const MENU: [&str; 6] = ["Copy", "Cut", "Paste", "Rename", "Delete", "New folder"];
pub(super) const SYMBOL_KEYS: &[u8] = b"QWERTYUIOPASDFGHJKLZXCVBNM";
pub(super) const SYMBOLS: &[u8] = b"_-.()[]{}!#$%&'+,;=@^`~";
pub(super) enum InputMode {
    Letters,
    Numbers,
    Symbols,
}
pub(super) enum Dialog {
    Menu(usize),
    Name {
        rename: bool,
        name: String,
        mode: InputMode,
    },
    Delete,
}
#[derive(Clone)]
pub(super) struct Clipboard {
    pub source: Vec<u8>,
    pub entry: Entry,
    pub cut: bool,
}
pub(super) struct CopyJob {
    pub source: Vec<u8>,
    pub destination: Vec<u8>,
    pub stage: Vec<u8>,
    pub file: Vec<u8>,
    pub size: u32,
    pub offset: u32,
    pub total_bytes: u64,
    pub copied_bytes: u64,
    pub total_items: u64,
    pub copied_items: u64,
    pub eta_seconds: Option<u64>,
    started_at: Option<u64>,
    active: bool,
    scan: Vec<CopyItem>,
    pending: VecDeque<CopyItem>,
}
impl CopyJob {
    pub fn scanning(&self) -> bool {
        !self.scan.is_empty()
    }

    pub fn progress(&self, scale: u64) -> u64 {
        if self.scanning() {
            0
        } else if let Some(progress) = (self.copied_bytes * scale).checked_div(self.total_bytes) {
            progress
        } else {
            self.copied_items * scale / self.total_items
        }
    }
}
struct CopyItem {
    source: Vec<u8>,
    target: Vec<u8>,
    entry: Entry,
}

fn child(parent: &[u8], name: &[u8]) -> Result<Vec<u8>, &'static str> {
    if name.is_empty()
        || name == b"."
        || name == b".."
        || name.iter().any(|byte| matches!(byte, 0 | b'/' | b'\\'))
    {
        return Err("Invalid filename in folder");
    }
    let path = [parent, b"\\", name].concat();
    if path.len() >= 260 {
        return Err("Path exceeds 259 bytes");
    }
    Ok(path)
}

impl<F: Filesystem> Explorer<F> {
    pub(super) fn operations_enabled(&self) -> bool {
        self.extension.is_none() && self.viewer.is_none() && !self.folders.is_empty()
    }
    pub(super) fn writable_folder(&self) -> bool {
        self.folders.first().is_some_and(|root| root == b"SD0:")
    }
    pub(super) fn menu_enabled(&self, index: usize) -> bool {
        let entry = self.entries.get(self.selected);
        match index {
            0 => entry.is_some(),
            1 | 3 | 4 => self.writable_folder() && entry.is_some(),
            2 => self.writable_folder() && self.clipboard.is_some(),
            5 => self.writable_folder(),
            _ => false,
        }
    }
    fn selected_entry(&self) -> Result<&Entry, &'static str> {
        self.entries
            .get(self.selected)
            .ok_or("Select an item first")
    }
    fn refresh_after(&mut self, name: Option<&[u8]>) -> Result<(), &'static str> {
        self.entries = self.load_dir(&self.path(None))?;
        if let Some(name) = name
            && let Some(index) = self.entries.iter().position(|e| e.name == name)
        {
            self.selected = index;
        }
        self.move_selection(0);
        Ok(())
    }
    fn choose_operation(&mut self, index: usize) -> Result<Action, &'static str> {
        if !self.menu_enabled(index) {
            return Ok(Action::None);
        }
        match index {
            0 | 1 => {
                let entry = self.selected_entry()?.clone();
                self.clipboard = Some(Clipboard {
                    source: self.path(Some(&entry.name)),
                    entry,
                    cut: index == 1,
                });
                self.dialog = None;
            }
            2 => self.paste()?,
            3 | 5 => {
                self.dialog = Some(Dialog::Name {
                    rename: index == 3,
                    name: String::new(),
                    mode: InputMode::Letters,
                })
            }
            4 => self.dialog = Some(Dialog::Delete),
            _ => unreachable!(),
        }
        Ok(Action::Redraw)
    }
    fn paste(&mut self) -> Result<(), &'static str> {
        let clip = self.clipboard.as_ref().unwrap().clone();
        let destination = self.path(Some(&clip.entry.name));
        if destination.eq_ignore_ascii_case(&clip.source) {
            return Err("Source and destination are the same");
        }
        if clip.entry.directory
            && destination
                .get(..clip.source.len())
                .is_some_and(|prefix| prefix.eq_ignore_ascii_case(&clip.source))
            && destination.get(clip.source.len()) == Some(&b'\\')
        {
            return Err("Cannot paste a folder inside itself");
        }
        let current = self.fs.metadata(&clip.source)?;
        if current.directory != clip.entry.directory
            || (!current.directory && current.size != clip.entry.size)
        {
            return Err("Source changed. Select it again");
        }
        if self
            .fs
            .read_dir(&self.path(None))?
            .iter()
            .any(|e| e.name.eq_ignore_ascii_case(&clip.entry.name))
        {
            return Err("Destination already exists");
        }
        if clip.cut {
            self.fs.rename(&clip.source, &destination)?;
            let name = self.clipboard.take().unwrap().entry.name;
            self.dialog = None;
            self.refresh_after(Some(&name))?;
        } else {
            if !current.directory && current.size > i32::MAX as u32 {
                return Err("Copy supports files up to 2 GiB - 1 byte");
            }
            let stage = self.path(Some(b".explorer-copy"));
            let file = [stage.as_slice(), b"\\data"].concat();
            if destination.len() >= 260 || file.len() >= 260 {
                return Err("Path exceeds 259 bytes");
            }
            self.fs.create_dir(&stage)?;
            self.copying = Some(CopyJob {
                source: clip.source.clone(),
                destination,
                stage,
                file: file.clone(),
                size: 0,
                offset: 0,
                total_bytes: 0,
                copied_bytes: 0,
                total_items: 0,
                copied_items: 0,
                eta_seconds: None,
                started_at: None,
                active: false,
                pending: VecDeque::new(),
                scan: alloc::vec![CopyItem {
                    source: clip.source,
                    target: file,
                    entry: current
                }],
            });
            self.dialog = None;
        }
        Ok(())
    }
    fn confirm_operation(&mut self) -> Result<Action, &'static str> {
        match self.dialog.as_ref() {
            Some(Dialog::Name { rename, name, .. }) => {
                if name.is_empty() || name == "." || name == ".." || name.ends_with(['.', ' ']) {
                    return Err("Enter a name without a trailing dot or space");
                }
                let name = name.as_bytes().to_vec();
                let destination = self.path(Some(&name));
                if destination.len() >= 260 {
                    return Err("Path exceeds 259 bytes");
                }
                if *rename {
                    let source = self.path(Some(&self.selected_entry()?.name));
                    self.fs.rename(&source, &destination)?;
                } else {
                    self.fs.create_dir(&destination)?;
                }
                self.dialog = None;
                self.refresh_after(Some(&name))?;
            }
            Some(Dialog::Delete) => {
                let entry = self.selected_entry()?;
                self.fs
                    .remove(&self.path(Some(&entry.name)), entry.directory)?;
                self.dialog = None;
                self.refresh_after(None)?;
            }
            _ => return Ok(Action::None),
        }
        Ok(Action::Redraw)
    }
    pub fn copying(&self) -> bool {
        self.copying.is_some()
    }

    /// One entry or bounded file chunk per tick. No filesystem handles survive a tick.
    pub fn poll(&mut self, now_micros: u64) -> Action {
        let Some(job) = self.copying.as_mut() else {
            return Action::None;
        };
        if !job.scanning() {
            let started_at = *job.started_at.get_or_insert(now_micros);
            let elapsed = now_micros.wrapping_sub(started_at);
            let (done, total) = if job.total_bytes == 0 {
                (job.copied_items, job.total_items)
            } else {
                (job.copied_bytes, job.total_bytes)
            };
            if done > 0 && elapsed > 0 {
                job.eta_seconds = Some(
                    (u128::from(elapsed) * u128::from(total - done))
                        .div_ceil(u128::from(done) * 1_000_000) as u64,
                );
            }
        }
        match self.copy_chunk() {
            Ok(()) => Action::Redraw,
            Err(error) => {
                let cleanup = self.cancel_copy();
                self.error = Some(match cleanup {
                    Ok(()) => error,
                    Err(cleanup) => cleanup,
                });
                Action::Redraw
            }
        }
    }
    fn copy_chunk(&mut self) -> Result<(), &'static str> {
        let job = self.copying.as_mut().unwrap();
        if let Some(item) = job.scan.pop() {
            if job.destination.len() + item.target.len() - job.stage.len() - b"\\data".len() >= 260
            {
                return Err("Path exceeds 259 bytes");
            }
            if item.entry.directory {
                for entry in self.fs.read_dir(&item.source)?.into_iter().rev() {
                    job.scan.push(CopyItem {
                        source: child(&item.source, &entry.name)?,
                        target: child(&item.target, &entry.name)?,
                        entry,
                    });
                }
            } else {
                if item.entry.size > i32::MAX as u32 {
                    return Err("Copy supports files up to 2 GiB - 1 byte");
                }
                job.total_bytes += u64::from(item.entry.size);
            }
            job.total_items += 1;
            job.pending.push_back(item);
            return Ok(());
        }
        if !job.active {
            if let Some(item) = job.pending.pop_front() {
                let current = self.fs.metadata(&item.source)?;
                if current.directory != item.entry.directory
                    || (!current.directory && current.size != item.entry.size)
                {
                    return Err("Source changed during copy");
                }
                job.source = item.source;
                job.file = item.target;
                job.size = if current.directory { 0 } else { current.size };
                job.offset = 0;
                if current.directory {
                    self.fs.create_dir(&job.file)?;
                    job.copied_items += 1;
                } else {
                    self.fs.create_file(&job.file)?;
                    job.active = true;
                }
            } else {
                self.fs
                    .rename(&child(&job.stage, b"data")?, &job.destination)?;
                let job = self.copying.take().unwrap();
                let cleanup = self.fs.remove(&job.stage, true);
                let refresh = self.refresh_after(None);
                cleanup.map_err(|_| "Copied; remove the empty .explorer-copy folder")?;
                refresh?;
            }
            return Ok(());
        }
        if job.offset < job.size {
            let mut data = alloc::vec![0u8; 16 * 1024];
            let count = data.len().min((job.size - job.offset) as usize);
            if self.fs.read(&job.source, job.offset, &mut data[..count])? != count {
                return Err("Source changed during copy");
            }
            self.fs.write(&job.file, job.offset, &data[..count])?;
            let mut verify = alloc::vec![0u8; 16 * 1024];
            if self.fs.read(&job.file, job.offset, &mut verify[..count])? != count
                || data[..count] != verify[..count]
            {
                return Err("Copy verification failed");
            }
            job.offset += count as u32;
            job.copied_bytes += count as u64;
        } else {
            let current = self.fs.metadata(&job.source)?;
            if current.directory
                || current.size != job.size
                || self.fs.read(&job.source, job.size, &mut [0])? != 0
            {
                return Err("Source changed during copy");
            }
            job.active = false;
            job.copied_items += 1;
        }
        Ok(())
    }
    pub fn cancel_copy(&mut self) -> Result<(), &'static str> {
        if let Some(job) = self.copying.take() {
            self.remove_staging(job.stage)
                .map_err(|_| "Copy stopped; remove .explorer-copy manually")?;
        }
        Ok(())
    }
    fn remove_staging(&mut self, stage: Vec<u8>) -> Result<(), &'static str> {
        // Only traverse the staging directory created exclusively by this job.
        let mut pending = alloc::vec![(stage, false)];
        while let Some((path, visited)) = pending.pop() {
            let directory = self.fs.metadata(&path)?.directory;
            if directory && !visited {
                let entries = self.fs.read_dir(&path)?;
                pending.push((path.clone(), true));
                for entry in entries {
                    pending.push((child(&path, &entry.name)?, false));
                }
            } else {
                self.fs.remove(&path, directory)?;
            }
        }
        Ok(())
    }
    pub(super) fn operation_control(&mut self, control: Control) -> Result<Action, &'static str> {
        match control {
            Control::Cancel | Control::Back => {
                self.dialog = None;
                self.cancel_copy()?;
            }
            Control::Menu if self.operations_enabled() => self.dialog = Some(Dialog::Menu(0)),
            Control::MenuItem(index) if matches!(self.dialog, Some(Dialog::Menu(_))) => {
                return self.choose_operation(index);
            }
            Control::Confirm => return self.confirm_operation(),
            _ => return Ok(Action::None),
        }
        self.pressed = None;
        Ok(Action::Redraw)
    }
    pub(super) fn operation_key(&mut self, code: u16, flags: u32) -> Action {
        if code == key::ESCAPE {
            return self.activate(Control::Cancel);
        }
        match &mut self.dialog {
            Some(Dialog::Menu(index)) => match code {
                key::UP => *index = (*index + 5) % 6,
                key::DOWN => *index = (*index + 1) % 6,
                key::ENTER => {
                    let index = *index;
                    return self.activate(Control::MenuItem(index));
                }
                _ => return Action::None,
            },
            Some(Dialog::Name { name, mode, .. }) => {
                if code == key::ENTER {
                    return self.activate(Control::Confirm);
                }
                if code == key::INPUT_MODE {
                    *mode = match mode {
                        InputMode::Letters => InputMode::Numbers,
                        InputMode::Numbers => InputMode::Symbols,
                        InputMode::Symbols => InputMode::Letters,
                    };
                } else if code == key::BACKSPACE || code == 0x7f {
                    name.pop();
                } else if (32..=126).contains(&code) {
                    let mut byte = code as u8;
                    if byte.is_ascii_alphabetic() {
                        byte = match mode {
                            InputMode::Letters => {
                                if flags & reflex::peg::KMOD_SHIFT != 0 {
                                    byte.to_ascii_uppercase()
                                } else {
                                    byte.to_ascii_lowercase()
                                }
                            }
                            InputMode::Numbers | InputMode::Symbols => {
                                let (keys, values) = match mode {
                                    InputMode::Numbers => (&b"QWERTYUIOP"[..], &b"1234567890"[..]),
                                    InputMode::Symbols => (SYMBOL_KEYS, SYMBOLS),
                                    InputMode::Letters => unreachable!(),
                                };
                                let Some((_, value)) = keys
                                    .iter()
                                    .zip(values)
                                    .find(|(key, _)| **key == byte.to_ascii_uppercase())
                                else {
                                    return Action::None;
                                };
                                *value
                            }
                        };
                    }
                    if (byte.is_ascii_alphanumeric() || byte == b' ' || SYMBOLS.contains(&byte))
                        && name.len() < 255
                    {
                        name.push(byte as char);
                    }
                }
            }
            Some(Dialog::Delete) if matches!(code, 0x59 | 0x79) => {
                return self.activate(Control::Confirm);
            }
            Some(Dialog::Delete) if matches!(code, 0x4e | 0x6e) => {
                return self.activate(Control::Cancel);
            }
            _ => return Action::None,
        }
        self.error = None;
        Action::Redraw
    }
    pub(super) fn operation_hit(&self, x: usize, y: usize) -> Option<Control> {
        if (16..384).contains(&x) && (426..474).contains(&y) {
            return Some(Control::Cancel);
        }
        match &self.dialog {
            Some(Dialog::Menu(_)) if (16..784).contains(&x) && (104..368).contains(&y) => {
                let index = (y - 104) / 44;
                self.menu_enabled(index).then_some(Control::MenuItem(index))
            }
            Some(Dialog::Name { .. } | Dialog::Delete)
                if (416..784).contains(&x) && (426..474).contains(&y) =>
            {
                Some(Control::Confirm)
            }
            _ => None,
        }
    }
}
