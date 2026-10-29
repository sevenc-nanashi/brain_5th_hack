//! Experimental V690_H1 PCM playback. See docs/audio.md for firmware evidence.
use core::ffi::c_void;

mod watchdog;
pub use watchdog::StreamWatchdog;

/// Input buffer cursors used by the V690_H1 PCM service.
/// # Safety
/// Requires an open, configured V690_H1 handle. These are firmware-private fields.
pub unsafe fn cursors(handle: Handle) -> [u32; 2] {
    unsafe {
        let words = handle.cast::<u32>();
        [
            words.add(0x4c / 4).read_volatile(),
            words.add(0x58 / 4).read_volatile(),
        ]
    }
}

pub type Handle = *mut c_void;
pub const PM_AUDIO: u16 = 0x107;
/// `MessageData::ext_params[0]` identifies the consumed buffer (0 or 1).
pub const BUFFER_CONSUMED: u16 = 5;
pub const PCM: u32 = 0x100;
pub const NOTIFY_WINDOW: u32 = 0x0100_0000;

/// Read the host's saved playback volume (0–9).
///
/// # Safety
/// Requires V690_H1 and its SharpLib exports.
pub unsafe fn volume() -> u32 {
    let call: unsafe extern "C" fn() -> u32 = unsafe { core::mem::transmute(0x8401_0054usize) };
    unsafe { call() }
}

/// Update the host volume setting, including its system change notification.
///
/// # Safety
/// Requires V690_H1. This changes the host setting, not just this playback session.
pub unsafe fn set_volume(level: u32) -> bool {
    if level > 9 {
        return false;
    }
    let call: unsafe extern "C" fn(u32) -> u32 = unsafe { core::mem::transmute(0x8401_00bcusize) };
    unsafe {
        // The export returns the previous volume, not a success code.
        call(level);
        volume() == level
    }
}

#[repr(C)]
pub struct Config {
    pub flags: u32,
    pub total_bytes: u32,
    pub sample_rate: u32,
    pub bits: u16,
    pub channels: u16,
    pub buffers: [*const u8; 2],
    pub lengths: [u32; 2],
    pub window: *mut c_void,
}

#[repr(C)]
#[derive(Default)]
pub struct Status {
    pub state: u32,
    pub position_ms: u32,
    pub duration_ms: u32,
    pub byte_position: u32,
}

const _: () = {
    assert!(size_of::<Config>() == 36);
    assert!(core::mem::offset_of!(Config, window) == 32);
    assert!(size_of::<Status>() == 16);
};

/// # Safety
/// Requires V690_H1. Own the system audio service exclusively until close.
pub unsafe fn open_playback() -> Handle {
    let call: unsafe extern "C" fn(u32) -> Handle =
        unsafe { core::mem::transmute(0x8401_01d4usize) };
    unsafe { call(0x10) }
}

/// # Safety
/// Handle must be open and stopped. Buffers and window must remain live until
/// playback has stopped and close succeeds. Config itself is copied synchronously.
pub unsafe fn configure(handle: Handle, config: &Config) -> bool {
    let call: unsafe extern "C" fn(Handle, *const Config) -> i32 =
        unsafe { core::mem::transmute(0x8401_01dcusize) };
    unsafe { call(handle, config) == 0 }
}

/// Reduce the kernel's two PCM staging buffers before the first start.
///
/// # Safety
/// Requires a newly opened V690_H1 playback handle, exclusively owned and not
/// started. This accesses firmware-private fields, not a stable public ABI.
pub unsafe fn set_staging_bytes(handle: Handle, bytes: u32) -> bool {
    if handle.is_null() || bytes == 0 || bytes > 0x8000 || !bytes.is_multiple_of(4) {
        return false;
    }
    unsafe {
        let words = handle.cast::<u32>();
        if words.add(0x60 / 4).read_volatile() != 0
            || words.add(0x20 / 4).read_volatile() != 0x8000
            || words.add(0x2c / 4).read_volatile() != 0x8000
        {
            return false;
        }
        // Keep the allocated addresses; only shorten the lengths used by the mixer.
        words.add(0x20 / 4).write_volatile(bytes);
        words.add(0x2c / 4).write_volatile(bytes);
    }
    true
}

/// # Safety
/// Handle must be configured; buffers must contain the initial PCM data.
pub unsafe fn start(handle: Handle) -> bool {
    let call: unsafe extern "C" fn(Handle) -> i32 =
        unsafe { core::mem::transmute(0x8401_01e0usize) };
    unsafe { call(handle) == 0 }
}

/// # Safety
/// Refill only a consumed buffer, without racing the firmware reader. `bytes`
/// must fit its configured allocation. Publish data before calling this function.
pub unsafe fn submit(handle: Handle, index: u32, bytes: u32) -> bool {
    if index > 1 {
        return false;
    }
    let call: unsafe extern "C" fn(Handle, u32, u32) -> i32 =
        unsafe { core::mem::transmute(0x8401_01f8usize) };
    unsafe { call(handle, index, bytes) == 0 }
}

/// # Safety
/// Handle must be open. Success does not guarantee completion; inspect status.
pub unsafe fn stop_wait(handle: Handle) -> bool {
    let call: unsafe extern "C" fn(Handle) -> i32 =
        unsafe { core::mem::transmute(0x8401_01ecusize) };
    unsafe { call(handle) == 0 }
}

/// # Safety
/// Handle must remain open for the duration of the call.
pub unsafe fn status(handle: Handle, out: &mut Status) -> bool {
    let call: unsafe extern "C" fn(Handle, *mut Status) -> i32 =
        unsafe { core::mem::transmute(0x8401_01f4usize) };
    unsafe { call(handle, out) == 0 }
}

/// # Safety
/// Stop playback and verify state == 0 first. Invalidate the handle on success.
pub unsafe fn close(handle: Handle) -> bool {
    let call: unsafe extern "C" fn(Handle) -> i32 =
        unsafe { core::mem::transmute(0x8401_01d8usize) };
    unsafe { call(handle) != 0 }
}
