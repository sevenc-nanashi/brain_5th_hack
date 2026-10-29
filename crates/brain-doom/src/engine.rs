//! Single-instance C engine adapter. All calls must be serialized.
use core::{
    ffi::{CStr, c_char, c_void},
    ptr,
};

static WAD: &[u8] = include_bytes!(env!("DOOM_WAD"));

#[repr(C, align(8))]
struct Allocation {
    previous: *mut Allocation,
    next: *mut Allocation,
}
static mut ALLOCATIONS: *mut Allocation = ptr::null_mut();
static mut CLOCK_US: u64 = 1_000_000;
static mut FILE: WadFile = WadFile {
    open: false,
    position: 0,
};
static mut LOG: [u8; 256] = [0; 256];
struct WadFile {
    open: bool,
    position: usize,
}

unsafe extern "C" {
    fn brain_doom_init() -> i32;
    fn brain_doom_step() -> i32;
    fn brain_doom_frame() -> *const u8;
    fn brain_doom_key(code: i32, pressed: i32);
}

#[cfg(not(target_os = "none"))]
unsafe extern "C" {
    fn malloc(size: usize) -> *mut c_void;
    fn free(ptr: *mut c_void);
}
#[cfg(target_os = "none")]
use reflex::runtime::{alloc as malloc, free};

/// # Safety
/// Call once per process/app load, on the single engine thread. Set time first.
pub unsafe extern "C" fn init() -> i32 {
    unsafe { brain_doom_init() }
}
/// # Safety
/// Requires successful initialization and exclusive engine access. Zero means running.
pub unsafe extern "C" fn step() -> i32 {
    unsafe { brain_doom_step() }
}
/// # Safety
/// Serialize with all engine calls. Use elapsed microseconds, not CPU iterations.
pub unsafe fn set_time(elapsed_us: u64) {
    unsafe {
        CLOCK_US = elapsed_us + 1_000_000;
    }
}
/// # Safety
/// Requires initialization and exclusive access until the returned slice is dropped.
pub unsafe fn frame<'a>() -> &'a [u8] {
    unsafe {
        core::slice::from_raw_parts(
            brain_doom_frame(),
            super::DOOM_WIDTH * super::DOOM_HEIGHT * 3,
        )
    }
}
/// # Safety
/// Requires initialization and exclusive access. `code` must be a PureDOOM key.
pub unsafe fn key(code: i32, pressed: bool) {
    unsafe {
        brain_doom_key(code, i32::from(pressed));
    }
}
/// # Safety
/// Call only after the engine has stopped, with no outstanding frame references.
/// The engine cannot be restarted without reinitializing the app image.
pub unsafe fn shutdown() {
    unsafe {
        while !ALLOCATIONS.is_null() {
            brain_free(ALLOCATIONS.add(1).cast());
        }
        FILE.open = false;
    }
}
/// # Safety
/// Serialize with engine execution; no log callback may run while borrowing this.
pub unsafe fn last_log<'a>() -> &'a [u8] {
    unsafe { CStr::from_ptr(ptr::addr_of!(LOG).cast()).to_bytes() }
}

#[unsafe(no_mangle)]
unsafe extern "C" fn brain_print(message: *const c_char) {
    unsafe {
        let bytes = CStr::from_ptr(message).to_bytes();
        if bytes.iter().any(|&c| c != b'\n' && c != b'\r') {
            let len = bytes.len().min(255);
            ptr::write_bytes(ptr::addr_of_mut!(LOG).cast::<u8>(), 0, 256);
            ptr::copy_nonoverlapping(bytes.as_ptr(), ptr::addr_of_mut!(LOG).cast(), len);
        }
    }
}
#[unsafe(no_mangle)]
unsafe extern "C" fn brain_alloc(size: i32) -> *mut c_void {
    if size <= 0 {
        return ptr::null_mut();
    }
    unsafe {
        let allocation = malloc(size as usize + size_of::<Allocation>()).cast::<Allocation>();
        if allocation.is_null() {
            return ptr::null_mut();
        }
        allocation.write(Allocation {
            previous: ptr::null_mut(),
            next: ALLOCATIONS,
        });
        if !ALLOCATIONS.is_null() {
            (*ALLOCATIONS).previous = allocation;
        }
        ALLOCATIONS = allocation;
        allocation.add(1).cast()
    }
}
#[unsafe(no_mangle)]
unsafe extern "C" fn brain_free(pointer: *mut c_void) {
    if pointer.is_null() {
        return;
    }
    unsafe {
        let allocation = pointer.cast::<Allocation>().sub(1);
        let Allocation { previous, next } = allocation.read();
        if previous.is_null() {
            ALLOCATIONS = next;
        } else {
            (*previous).next = next;
        }
        if !next.is_null() {
            (*next).previous = previous;
        }
        free(allocation.cast());
    }
}
#[unsafe(no_mangle)]
unsafe extern "C" fn brain_open(name: *const c_char, mode: *const c_char) -> *mut c_void {
    unsafe {
        let name = CStr::from_ptr(name).to_bytes();
        let mode = CStr::from_ptr(mode).to_bytes();
        if (name == b"./doom1.wad" || name == b"doom1.wad") && mode == b"rb" && !FILE.open {
            FILE = WadFile {
                open: true,
                position: 0,
            };
            ptr::addr_of_mut!(FILE).cast()
        } else {
            ptr::null_mut()
        }
    }
}
#[unsafe(no_mangle)]
unsafe extern "C" fn brain_close(handle: *mut c_void) {
    unsafe {
        (*handle.cast::<WadFile>()).open = false;
    }
}
#[unsafe(no_mangle)]
unsafe extern "C" fn brain_read(handle: *mut c_void, output: *mut c_void, count: i32) -> i32 {
    if count < 0 {
        return -1;
    }
    unsafe {
        let file = &mut *handle.cast::<WadFile>();
        let size = (count as usize).min(WAD.len() - file.position);
        ptr::copy_nonoverlapping(WAD.as_ptr().add(file.position), output.cast(), size);
        file.position += size;
        size as i32
    }
}
#[unsafe(no_mangle)]
unsafe extern "C" fn brain_seek(handle: *mut c_void, offset: i32, origin: i32) -> i32 {
    unsafe {
        let file = &mut *handle.cast::<WadFile>();
        let base = match origin {
            0 => 0,
            1 => file.position,
            2 => WAD.len(),
            _ => return -1,
        };
        let position = base as i64 + i64::from(offset);
        if position < 0 || position > WAD.len() as i64 {
            return -1;
        }
        file.position = position as usize;
        0
    }
}
#[unsafe(no_mangle)]
unsafe extern "C" fn brain_tell(handle: *mut c_void) -> i32 {
    unsafe { (*handle.cast::<WadFile>()).position as i32 }
}
#[unsafe(no_mangle)]
unsafe extern "C" fn brain_eof(handle: *mut c_void) -> i32 {
    unsafe { i32::from((*handle.cast::<WadFile>()).position == WAD.len()) }
}
#[unsafe(no_mangle)]
unsafe extern "C" fn brain_time(seconds: *mut i32, micros: *mut i32) {
    unsafe {
        *seconds = (CLOCK_US / 1_000_000) as i32;
        *micros = (CLOCK_US % 1_000_000) as i32;
    }
}
