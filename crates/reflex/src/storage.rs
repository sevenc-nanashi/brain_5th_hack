extern crate alloc;
#[cfg(not(target_os = "none"))]
extern crate std;

use alloc::{format, string::String, vec::Vec};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const MAX_FILE: usize = 2 * 1024 * 1024;
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    version: u8,
    generation: u64,
    digest: [u8; 32],
    data: Vec<u8>,
}

/// Write the inactive snapshot first; a failed write never truncates the active save.
pub struct Store {
    prefix: String,
    generation: u64,
    active: usize,
    pub recovered: bool,
}
impl Store {
    pub fn open(prefix: String) -> Result<(Self, Option<Vec<u8>>), &'static str> {
        #[cfg(target_os = "none")]
        {
            use crate::fs;
            use alloc::ffi::CString;
            let (parent, _) = prefix.rsplit_once('\\').ok_or("Invalid save path")?;
            let parent = CString::new(parent).map_err(|_| "Invalid save folder")?;
            if unsafe { fs::Directory::open(&parent) }.is_none() {
                unsafe { fs::create_dir(&parent) }.map_err(|_| "Cannot create save folder")?;
            }
        }
        let mut latest: Option<(usize, Record)> = None;
        let mut error = None;
        for slot in 0..2 {
            match Self::read_slot(&format!("{prefix}.{slot}.json")) {
                Ok(Some(record)) => {
                    if latest
                        .as_ref()
                        .is_none_or(|(_, old)| record.generation > old.generation)
                    {
                        latest = Some((slot, record));
                    }
                }
                Ok(None) => {}
                Err(cause) => error = Some(cause),
            }
        }
        if latest.is_none()
            && let Some(error) = error
        {
            return Err(error);
        }
        let (active, generation, data) = match latest {
            Some((slot, record)) => (slot, record.generation, Some(record.data)),
            None => (1, 0, None),
        };
        Ok((
            Self {
                prefix,
                generation,
                active,
                recovered: error.is_some(),
            },
            data,
        ))
    }
    fn read_slot(path: &str) -> Result<Option<Record>, &'static str> {
        let Some(bytes) = read(path, MAX_FILE)? else {
            return Ok(None);
        };
        let record: Record = serde_json::from_slice(&bytes).map_err(|_| "Invalid save file")?;
        if record.version != 1 || record.digest != <[u8; 32]>::from(Sha256::digest(&record.data)) {
            return Err("Invalid save checksum");
        }
        Ok(Some(record))
    }
    pub fn save(&mut self, data: &[u8]) -> Result<(), &'static str> {
        let generation = self
            .generation
            .checked_add(1)
            .ok_or("Save counter exhausted")?;
        let record = Record {
            version: 1,
            generation,
            digest: Sha256::digest(data).into(),
            data: data.to_vec(),
        };
        let bytes = serde_json::to_vec(&record).map_err(|_| "Cannot encode saved data")?;
        if bytes.len() > MAX_FILE {
            return Err("Saved data is too large");
        }
        let slot = 1 - self.active;
        write(
            &format!("{}.{slot}.json", self.prefix),
            &bytes,
            self.generation != 0,
        )?;
        self.generation = generation;
        self.active = slot;
        self.recovered = false;
        Ok(())
    }
}

#[cfg(target_os = "none")]
pub fn read(path: &str, limit: usize) -> Result<Option<Vec<u8>>, &'static str> {
    use crate::fs;
    use alloc::ffi::CString;
    use core::ffi::CStr;
    // Use directory entries for both presence and size, without assuming stat's missing-file code.
    let (parent, name) = path.rsplit_once('\\').ok_or("Invalid save path")?;
    let parent = CString::new(parent).map_err(|_| "Invalid save folder")?;
    let mut directory = unsafe { fs::Directory::open(&parent) }.ok_or("Cannot open save folder")?;
    let mut size = None;
    while let Some(entry) = directory.read() {
        let entry_name =
            CStr::from_bytes_until_nul(&entry.name).map_err(|_| "Invalid directory entry")?;
        if entry_name.to_bytes().eq_ignore_ascii_case(name.as_bytes()) {
            if entry.kind == 2 {
                return Err("Save path is a folder");
            }
            size = Some(entry.size as usize);
            break;
        }
    }
    drop(directory);
    let Some(size) = size else {
        return Ok(None);
    };
    if size > limit {
        return Err("File is too large");
    }
    let path = CString::new(path).map_err(|_| "Invalid file path")?;
    let mut file = unsafe { fs::File::open(&path) }.ok_or("Cannot open save file")?;
    let mut bytes = alloc::vec![0; size];
    let mut offset = 0;
    while offset < size {
        let count = file
            .read(&mut bytes[offset..])
            .map_err(|_| "Cannot read file")?;
        if count == 0 {
            return Err("File was truncated");
        }
        offset += count;
    }
    file.close().map_err(|_| "Cannot close file")?;
    Ok(Some(bytes))
}
#[cfg(target_os = "none")]
fn write(path: &str, data: &[u8], replace: bool) -> Result<(), &'static str> {
    use crate::fs;
    use alloc::ffi::CString;
    let path = CString::new(path).map_err(|_| "Invalid file path")?;
    if !replace && let Some(file) = unsafe { fs::File::open(&path) } {
        file.close().map_err(|_| "Cannot close existing save")?;
        return Err("Save already exists. Reopen app");
    }
    let mut file = unsafe { fs::File::create(&path) }.ok_or("Cannot create save file")?;
    file.write(data).map_err(|_| "Cannot write save file")?;
    file.flush().map_err(|_| "Cannot flush save file")?;
    file.close().map_err(|_| "Cannot close save file")
}
#[cfg(not(target_os = "none"))]
pub fn read(path: &str, limit: usize) -> Result<Option<Vec<u8>>, &'static str> {
    use std::{fs, io::Read};
    let mut file = match fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err("Cannot open file"),
    };
    let mut bytes = Vec::new();
    (&mut file)
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Cannot read file")?;
    if bytes.len() > limit {
        return Err("File is too large");
    }
    Ok(Some(bytes))
}
#[cfg(not(target_os = "none"))]
fn write(path: &str, data: &[u8], replace: bool) -> Result<(), &'static str> {
    use std::{fs, io::Write};
    let mut options = fs::OpenOptions::new();
    options.write(true);
    if replace {
        options.create(true).truncate(true);
    } else {
        options.create_new(true);
    }
    let mut file = options.open(path).map_err(|_| "Cannot create save file")?;
    file.write_all(data).map_err(|_| "Cannot write save file")?;
    file.sync_all().map_err(|_| "Cannot flush save file")
}
