#![no_std]
#![no_main]
extern crate alloc;

use alloc::{ffi::CString, vec::Vec};
use brain_explorer::{Entry, Explorer, Filesystem, MAX_ENTRIES};
use core::ffi::CStr;
use reflex::fs;

struct Device;

fn path(value: &[u8]) -> Result<CString, &'static str> {
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
}

#[reflex::main]
fn main() -> Explorer<Device> {
    Explorer::new(Device)
}
