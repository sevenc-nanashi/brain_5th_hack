use brain_doom::engine;
use core::{ffi::c_void, ptr};
use reflex::{audio, runtime};

const MIX_SAMPLES: usize = 512 * 2;
const SAMPLES: usize = 4096 / size_of::<i16>();
const BYTES: u32 = (SAMPLES * size_of::<i16>()) as u32;
const _: () = assert!(SAMPLES.is_multiple_of(MIX_SAMPLES));
static mut BUFFERS: [[i16; SAMPLES]; 2] = [[0; SAMPLES]; 2];
static mut HANDLE: audio::Handle = ptr::null_mut();
static mut STARTED: bool = false;
static mut MAX_FILL_US: u64 = 0;
static mut SUBMITS: u32 = 0;

unsafe fn fill(index: usize) -> bool {
    unsafe {
        let start = runtime::micros();
        for offset in (0..SAMPLES).step_by(MIX_SAMPLES) {
            if super::run(engine::mix) != 0 {
                return false;
            }
            ptr::copy_nonoverlapping(
                engine::sound().as_ptr(),
                ptr::addr_of_mut!(BUFFERS)
                    .cast::<i16>()
                    .add(index * SAMPLES + offset),
                MIX_SAMPLES,
            );
        }
        MAX_FILL_US = MAX_FILL_US.max(runtime::micros().wrapping_sub(start));
        true
    }
}

pub unsafe fn init(window: *mut c_void) -> bool {
    unsafe {
        HANDLE = audio::open_playback();
        if HANDLE.is_null() {
            return false;
        }
        if !audio::set_staging_bytes(HANDLE, BYTES) {
            return false;
        }
        if !fill(0) || !fill(1) {
            return false;
        }
        let data = ptr::addr_of!(BUFFERS).cast::<u8>();
        let config = audio::Config {
            flags: audio::PCM | audio::NOTIFY_WINDOW,
            // shortcut: finite ~13.5 hour stream; add a drained restart for longer sessions.
            total_bytes: 0x7fff_fffc,
            sample_rate: 11025,
            bits: 16,
            channels: 2,
            buffers: [data, data.add(BYTES as usize)],
            lengths: [BYTES; 2],
            window,
        };
        if !audio::configure(HANDLE, &config) {
            return false;
        }
        // A failed start may still have queued work; always stop before unloading.
        STARTED = true;
        if !audio::start(HANDLE) {
            return false;
        }
        // Start synchronously consumes both blocks, without sending refill events.
        refill(0) && refill(1)
    }
}

pub unsafe fn refill(index: i32) -> bool {
    if !(0..=1).contains(&index) {
        return false;
    }
    unsafe {
        if HANDLE.is_null() || !fill(index as usize) || !audio::submit(HANDLE, index as u32, BYTES)
        {
            return false;
        }
        SUBMITS = SUBMITS.saturating_add(1);
        true
    }
}

pub unsafe fn max_fill_ms() -> u32 {
    unsafe { MAX_FILL_US.div_ceil(1000).min(u32::MAX as u64) as u32 }
}

pub unsafe fn diagnostics() -> (u32, Option<u32>, u32) {
    unsafe {
        let mut status = audio::Status::default();
        let state = if !HANDLE.is_null() && audio::status(HANDLE, &mut status) {
            Some(status.state)
        } else {
            None
        };
        (max_fill_ms(), state, SUBMITS)
    }
}

pub unsafe fn shutdown() -> bool {
    unsafe {
        if HANDLE.is_null() {
            return true;
        }
        if STARTED {
            let mut status = audio::Status::default();
            if !audio::stop_wait(HANDLE) || !audio::status(HANDLE, &mut status) || status.state != 0
            {
                return false;
            }
        }
        if !audio::close(HANDLE) {
            return false;
        }
        HANDLE = ptr::null_mut();
        STARTED = false;
        true
    }
}
