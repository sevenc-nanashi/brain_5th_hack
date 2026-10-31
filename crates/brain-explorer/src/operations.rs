use super::*;

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
            0 => entry.is_some_and(|e| !e.directory),
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
            && destination.starts_with(&[clip.source.as_slice(), b"\\"].concat())
        {
            return Err("Cannot move a folder inside itself");
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
            if current.size > i32::MAX as u32 {
                return Err("Copy supports files up to 2 GiB - 1 byte");
            }
            let stage = self.path(Some(b".explorer-copy"));
            let file = [stage.as_slice(), b"\\data"].concat();
            if destination.len() >= 260 || file.len() >= 260 {
                return Err("Path exceeds 259 bytes");
            }
            self.fs.create_dir(&stage)?;
            if let Err(error) = self.fs.create_file(&file) {
                // A failed close can leave an empty file; retain the staging folder for recovery.
                self.dialog = None;
                self.refresh_after(None)?;
                return Err(error);
            }
            self.copying = Some(CopyJob {
                source: clip.source.clone(),
                destination,
                stage,
                file,
                size: current.size,
                offset: 0,
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

    /// One bounded copy chunk per host tick. No filesystem handles survive a tick.
    pub fn poll(&mut self) -> Action {
        if self.copying.is_none() {
            return Action::None;
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
        } else {
            if self.fs.metadata(&job.source)?.size != job.size
                || self.fs.read(&job.source, job.size, &mut [0])? != 0
            {
                return Err("Source changed during copy");
            }
            self.fs.rename(&job.file, &job.destination)?;
            let job = self.copying.take().unwrap();
            let cleanup = self.fs.remove(&job.stage, true);
            let refresh = self.refresh_after(None);
            cleanup.map_err(|_| "Copied; remove the empty .explorer-copy folder")?;
            refresh?;
        }
        Ok(())
    }
    pub fn cancel_copy(&mut self) -> Result<(), &'static str> {
        if let Some(job) = self.copying.take() {
            self.fs
                .remove(&job.file, false)
                .map_err(|_| "Copy stopped; remove .explorer-copy manually")?;
            self.fs
                .remove(&job.stage, true)
                .map_err(|_| "Copy stopped; remove .explorer-copy manually")?;
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
