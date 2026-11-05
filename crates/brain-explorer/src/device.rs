use crate::{Entry, Filesystem, MAX_ENTRIES};
use alloc::{ffi::CString, vec::Vec};
use core::ffi::CStr;
use reflex::fs;

pub struct Device;

pub fn path(value: &[u8]) -> Result<CString, &'static str> {
    if value.len() >= 260 {
        return Err("Path exceeds 259 bytes");
    }
    CString::new(value).map_err(|_| "Path contains a NUL byte")
}

impl Filesystem for Device {
    fn read_dir(&mut self, value: &[u8]) -> Result<Vec<Entry>, &'static str> {
        let path = path(value)?;
        let mut directory = unsafe { fs::Directory::open(&path) }.ok_or("Cannot open folder")?;
        let mut entries = Vec::new();
        while let Some(entry) = directory.read() {
            let name = CStr::from_bytes_until_nul(&entry.name)
                .map_err(|_| "Unterminated filename")?
                .to_bytes();
            if name == b"." || name == b".." {
                continue;
            }
            // shortcut: bound directory memory; add paged enumeration if larger folders are needed.
            if entries.len() == MAX_ENTRIES {
                return Err("Folder exceeds 4096 entries");
            }
            entries.push(Entry {
                name: name.to_vec(),
                directory: entry.kind == 2,
                size: entry.size,
            });
        }
        Ok(entries)
    }

    fn read(
        &mut self,
        value: &[u8],
        offset: u32,
        buffer: &mut [u8],
    ) -> Result<usize, &'static str> {
        let path = path(value)?;
        let offset = i32::try_from(offset).map_err(|_| "Offsets above 2 GiB are unsupported")?;
        let mut file = unsafe { fs::File::open(&path) }.ok_or("Cannot open file")?;
        file.seek(offset)
            .map_err(|_| "Cannot seek file. R to retry")?;
        let count = file
            .read(buffer)
            .map_err(|_| "Cannot read file. R to retry")?;
        file.close().map_err(|_| "Cannot close file")?;
        Ok(count)
    }
    fn metadata(&mut self, value: &[u8]) -> Result<Entry, &'static str> {
        let path = path(value)?;
        let metadata = unsafe { fs::metadata(&path) }.map_err(|_| "Cannot read metadata")?;
        Ok(Entry {
            name: Vec::new(),
            directory: metadata.mode & 0xf000 == 0x4000,
            size: metadata.size,
        })
    }
    fn create_dir(&mut self, value: &[u8]) -> Result<(), &'static str> {
        let path = writable(value)?;
        absent(&path)?;
        unsafe { fs::create_dir(&path) }.map_err(|_| "Cannot create folder")
    }
    fn create_file(&mut self, value: &[u8]) -> Result<(), &'static str> {
        let path = writable(value)?;
        absent(&path)?;
        let file = unsafe { fs::File::create(&path) }.ok_or("Cannot create file")?;
        file.close().map_err(|_| "Cannot close new file")
    }
    fn write(&mut self, value: &[u8], offset: u32, data: &[u8]) -> Result<(), &'static str> {
        let path = writable(value)?;
        let offset = i32::try_from(offset).map_err(|_| "Offsets above 2 GiB are unsupported")?;
        let mut file = unsafe { fs::File::update(&path) }.ok_or("Cannot open output file")?;
        file.seek(offset).map_err(|_| "Cannot seek output file")?;
        file.write(data).map_err(|_| "Cannot write output file")?;
        file.flush().map_err(|_| "Cannot flush output file")?;
        file.close().map_err(|_| "Cannot close output file")
    }
    fn rename(&mut self, source: &[u8], destination: &[u8]) -> Result<(), &'static str> {
        let source = writable(source)?;
        let destination = writable(destination)?;
        absent(&destination)?;
        unsafe { fs::rename(&source, &destination) }.map_err(|_| "Cannot move or rename item")
    }
    fn remove(&mut self, value: &[u8], directory: bool) -> Result<(), &'static str> {
        let path = writable(value)?;
        if self.metadata(value)?.directory != directory {
            return Err("Item type changed. Refresh and retry");
        }
        if directory && !self.read_dir(value)?.is_empty() {
            return Err("Folder is not empty");
        }
        unsafe { fs::remove(&path) }.map_err(|_| "Cannot delete item")
    }
}

fn writable(value: &[u8]) -> Result<CString, &'static str> {
    if !value.starts_with(b"SD0:\\") || value.len() <= 5 {
        return Err("Only items on SD0 can be changed");
    }
    path(value)
}
fn absent(value: &CStr) -> Result<(), &'static str> {
    match unsafe { fs::metadata(value) } {
        Ok(_) => Err("Destination already exists"),
        // V690_H1 FAT lookup returns -0x460d at system.bin:0x184b4.
        Err(-0x460d) => Ok(()),
        Err(_) => Err("Cannot check destination"),
    }
}
