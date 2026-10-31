use alloc::{boxed::Box, format, vec::Vec};
use core::{
    ffi::{CStr, c_char, c_void},
    ptr,
};
use reflex::storage::Store;

static mut HANDLES: Vec<*mut File> = Vec::new();
static mut STATUS: Option<Result<(), &'static str>> = None;

enum Contents {
    Wad,
    Saved {
        store: Store,
        data: Vec<u8>,
        writable: bool,
    },
}
struct File {
    position: usize,
    contents: Contents,
    failed: bool,
}

/// # Safety
/// Serialize with the engine and all file callbacks.
pub unsafe fn take_status() -> Option<Result<(), &'static str>> {
    unsafe { (&mut *ptr::addr_of_mut!(STATUS)).take() }
}
/// # Safety
/// The engine must have stopped using every file and WAD reference.
pub unsafe fn shutdown() {
    unsafe {
        while let Some(handle) = (&*ptr::addr_of!(HANDLES)).last().copied() {
            brain_close(handle.cast());
        }
    }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn brain_open(name: *const c_char, mode: *const c_char) -> *mut c_void {
    unsafe {
        let name = CStr::from_ptr(name).to_bytes();
        let name = name.strip_prefix(b"./").unwrap_or(name);
        let mode = CStr::from_ptr(mode).to_bytes();
        let contents = if name == b"doom1.wad" && mode == b"rb" {
            Contents::Wad
        } else {
            let allowed = name == b".doomrc"
                || name == b"default.cfg"
                || (name.len() == 12
                    && name.starts_with(b"doomsav")
                    && (b'0'..=b'5').contains(&name[7])
                    && name.ends_with(b".dsg"));
            if !allowed || !matches!(mode, b"r" | b"rb" | b"w" | b"wb") {
                return ptr::null_mut();
            }
            let name = core::str::from_utf8(name).unwrap();
            let (store, data) = match Store::open(format!(
                r"{}:\Doom\{name}",
                if name.ends_with(".dsg") { "USR" } else { "SYS" }
            )) {
                Ok(value) => value,
                Err(error) => {
                    STATUS = Some(Err(error));
                    return ptr::null_mut();
                }
            };
            if store.recovered {
                STATUS = Some(Err("Recovered previous save"));
            }
            let writable = mode[0] == b'w';
            let data = if writable {
                Vec::new()
            } else {
                match data {
                    Some(data) => data,
                    None => return ptr::null_mut(),
                }
            };
            Contents::Saved {
                store,
                data,
                writable,
            }
        };
        let handle = Box::into_raw(Box::new(File {
            position: 0,
            contents,
            failed: false,
        }));
        (&mut *ptr::addr_of_mut!(HANDLES)).push(handle);
        handle.cast()
    }
}
#[unsafe(no_mangle)]
unsafe extern "C" fn brain_close(handle: *mut c_void) {
    unsafe {
        let mut file = Box::from_raw(handle.cast::<File>());
        if !file.failed
            && let Contents::Saved {
                store,
                data,
                writable: true,
            } = &mut file.contents
        {
            STATUS = Some(store.save(data));
        }
        (&mut *ptr::addr_of_mut!(HANDLES)).retain(|&p| p != handle.cast());
    }
}
#[unsafe(no_mangle)]
unsafe extern "C" fn brain_read(handle: *mut c_void, output: *mut c_void, count: i32) -> i32 {
    if count < 0 {
        return -1;
    }
    unsafe {
        let file = &mut *handle.cast::<File>();
        let bytes = match &file.contents {
            Contents::Wad => super::engine::WAD,
            Contents::Saved { data, .. } => data,
        };
        let size = (count as usize).min(bytes.len() - file.position);
        ptr::copy_nonoverlapping(bytes.as_ptr().add(file.position), output.cast(), size);
        file.position += size;
        size as i32
    }
}
#[unsafe(no_mangle)]
unsafe extern "C" fn brain_write(handle: *mut c_void, input: *const c_void, count: i32) -> i32 {
    if count < 0 {
        return -1;
    }
    unsafe {
        let file = &mut *handle.cast::<File>();
        let Contents::Saved {
            data,
            writable: true,
            ..
        } = &mut file.contents
        else {
            return -1;
        };
        let end = file.position + count as usize;
        if end > 512 * 1024 {
            file.failed = true;
            STATUS = Some(Err("Save file is too large"));
            return -1;
        }
        if end > data.len() {
            data.resize(end, 0);
        }
        data[file.position..end]
            .copy_from_slice(core::slice::from_raw_parts(input.cast(), count as usize));
        file.position = end;
        count
    }
}
#[unsafe(no_mangle)]
unsafe extern "C" fn brain_seek(handle: *mut c_void, offset: i32, origin: i32) -> i32 {
    unsafe {
        let file = &mut *handle.cast::<File>();
        let size = match &file.contents {
            Contents::Wad => super::engine::WAD.len(),
            Contents::Saved { data, .. } => data.len(),
        };
        let base = match origin {
            0 => 0,
            1 => file.position,
            2 => size,
            _ => return -1,
        };
        let position = base as i64 + i64::from(offset);
        if position < 0 || position > size as i64 {
            return -1;
        }
        file.position = position as usize;
        0
    }
}
#[unsafe(no_mangle)]
unsafe extern "C" fn brain_tell(handle: *mut c_void) -> i32 {
    unsafe { (*handle.cast::<File>()).position as i32 }
}
#[unsafe(no_mangle)]
unsafe extern "C" fn brain_eof(handle: *mut c_void) -> i32 {
    unsafe {
        let file = &*handle.cast::<File>();
        let size = match &file.contents {
            Contents::Wad => super::engine::WAD.len(),
            Contents::Saved { data, .. } => data.len(),
        };
        i32::from(file.position == size)
    }
}
