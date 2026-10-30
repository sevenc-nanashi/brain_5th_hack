use super::path;
use alloc::vec::Vec;
use core::ffi::CStr;
use reflex::fs;

struct Record {
    path: Vec<u8>,
    relative: Vec<u8>,
    directory: bool,
    size: u32,
}

struct Folder {
    directory: fs::Directory,
    path: Vec<u8>,
    relative: Vec<u8>,
}

/// Depth-first enumeration keeps only the ancestor directories open.
struct Walker {
    root: Option<Record>,
    folders: Vec<Folder>,
}
impl Walker {
    fn new(name: &[u8], size: u32, directory: bool) -> Self {
        Self {
            root: Some(Record {
                path: name.to_vec(),
                relative: Vec::new(),
                directory,
                size: if directory { 0 } else { size },
            }),
            folders: Vec::new(),
        }
    }

    fn next(&mut self) -> Result<Option<Record>, &'static str> {
        let record = if let Some(root) = self.root.take() {
            root
        } else {
            loop {
                let Some(folder) = self.folders.last_mut() else {
                    return Ok(None);
                };
                let Some(entry) = folder.directory.read() else {
                    self.folders.pop();
                    continue;
                };
                let name = CStr::from_bytes_until_nul(&entry.name)
                    .map_err(|_| "Unterminated filename")?
                    .to_bytes();
                if name == b"." || name == b".." {
                    continue;
                }
                if name.is_empty() || name.iter().any(|byte| matches!(byte, 0 | b'/' | b'\\')) {
                    return Err("Invalid filename in folder");
                }
                let mut full = folder.path.clone();
                full.extend_from_slice(name);
                path(&full)?;
                let mut relative = folder.relative.clone();
                if !relative.is_empty() {
                    relative.push(b'/');
                }
                relative.extend_from_slice(name);
                break Record {
                    path: full,
                    relative,
                    directory: entry.kind == 2,
                    size: if entry.kind == 2 { 0 } else { entry.size },
                };
            }
        };
        if record.directory {
            let mut full = record.path.clone();
            if !full.ends_with(b"\\") {
                full.push(b'\\');
            }
            let directory =
                unsafe { fs::Directory::open(&path(&full)?) }.ok_or("Cannot open folder")?;
            self.folders.push(Folder {
                directory,
                path: full,
                relative: record.relative.clone(),
            });
        }
        Ok(Some(record))
    }
}

/// Scan once for progress totals, then enumerate again while streaming contents.
pub struct Source {
    walker: Walker,
    entries: u32,
    archive: bool,
    prefix: Vec<u8>,
    prefix_offset: usize,
    file: Option<fs::File>,
    remaining: u32,
    pub size: u64,
}
impl Source {
    pub fn new(name: &[u8], size: u32, directory: bool) -> Result<Self, &'static str> {
        path(name)?;
        let mut entries = 1u32;
        let mut total = u64::from(size);
        let mut prefix = Vec::new();
        if directory {
            entries = 0;
            total = 12;
            let mut scan = Walker::new(name, size, true);
            while let Some(record) = scan.next()? {
                entries = entries
                    .checked_add(1)
                    .ok_or("Folder entry count exceeds u32")?;
                total += 9 + record.relative.len() as u64 + u64::from(record.size);
            }
            prefix.extend_from_slice(b"BRNTREE1");
            prefix.extend_from_slice(&entries.to_le_bytes());
        }
        Ok(Self {
            walker: Walker::new(name, size, directory),
            entries,
            archive: directory,
            prefix,
            prefix_offset: 0,
            file: None,
            remaining: 0,
            size: total,
        })
    }

    pub fn read(&mut self, buffer: &mut [u8]) -> Result<usize, &'static str> {
        let mut written = 0;
        while written < buffer.len() {
            if self.prefix_offset < self.prefix.len() {
                let count = (self.prefix.len() - self.prefix_offset).min(buffer.len() - written);
                buffer[written..written + count]
                    .copy_from_slice(&self.prefix[self.prefix_offset..self.prefix_offset + count]);
                self.prefix_offset += count;
                written += count;
                continue;
            }
            if let Some(file) = &mut self.file {
                if self.remaining != 0 {
                    let count = (self.remaining as usize).min(buffer.len() - written);
                    let count = file
                        .read(&mut buffer[written..written + count])
                        .map_err(|_| "Cannot read file")?;
                    if count == 0 {
                        return Err("File size changed. Cancel and retry");
                    }
                    self.remaining -= count as u32;
                    written += count;
                }
                if self.remaining == 0 {
                    let mut extra = [0];
                    if file.read(&mut extra).map_err(|_| "Cannot read file")? != 0 {
                        return Err("File size changed. Cancel and retry");
                    }
                    self.file
                        .take()
                        .unwrap()
                        .close()
                        .map_err(|_| "Cannot close file")?;
                }
                continue;
            }
            let Some(record) = self.walker.next()? else {
                if self.entries != 0 {
                    return Err("Folder changed. Cancel and retry");
                }
                break;
            };
            self.entries = self
                .entries
                .checked_sub(1)
                .ok_or("Folder changed. Cancel and retry")?;
            self.prefix.clear();
            self.prefix_offset = 0;
            if self.archive {
                self.prefix.push(if record.directory { 1 } else { 2 });
                self.prefix
                    .extend_from_slice(&(record.relative.len() as u32).to_le_bytes());
                self.prefix.extend_from_slice(&record.size.to_le_bytes());
                self.prefix.extend_from_slice(&record.relative);
            }
            if !record.directory {
                self.file = Some(
                    unsafe { fs::File::open(&path(&record.path)?) }.ok_or("Cannot open file")?,
                );
                self.remaining = record.size;
            }
        }
        Ok(written)
    }
}
