use brain_tic80::engine;
use core::{ffi::c_void, ptr};
use reflex::{audio, runtime};

// Start primes two staging blocks; leave one input block available during the next fill.
const STAGING_BYTES: u32 = 4096;
const SAMPLES: usize = 4096;
const BYTES: u32 = (SAMPLES * size_of::<i16>()) as u32;
static mut BUFFERS: [[i16; SAMPLES]; 2] = [[0; SAMPLES]; 2];
static mut HANDLE: audio::Handle = ptr::null_mut();
static mut STARTED: bool = false;
static mut WATCHDOG: audio::StreamWatchdog = audio::StreamWatchdog::new();
static mut LAST_POLL_US: u64 = 0;
static mut FILL_US: u64 = 0;
static mut MAX_FILL_US: u64 = 0;
static mut SUBMITS: u32 = 0;
static mut RESTARTS: u32 = 0;

unsafe fn fill(index: usize) -> bool {
    unsafe {
        let output = ptr::addr_of_mut!(BUFFERS)
            .cast::<i16>()
            .add(index * SAMPLES);
        super::stack::run(|| engine::brain_tic_fill(output, SAMPLES)) == 0
    }
}

pub unsafe fn init(window: *mut c_void) -> bool {
    unsafe {
        assert!(HANDLE.is_null());
        WATCHDOG = audio::StreamWatchdog::new();
        LAST_POLL_US = runtime::micros();
        HANDLE = audio::open_playback();
        if HANDLE.is_null() || !audio::set_staging_bytes(HANDLE, STAGING_BYTES) {
            return false;
        }
        if !fill(0) || !fill(1) {
            return false;
        }
        let data = ptr::addr_of!(BUFFERS).cast::<u8>();
        let config = audio::Config {
            flags: audio::PCM | audio::NOTIFY_WINDOW,
            // shortcut: finite ~3.4 hour stream; add a drained restart for longer sessions.
            total_bytes: 0x7fff_fffc,
            sample_rate: 44100,
            bits: 16,
            channels: 2,
            buffers: [data, data.add(BYTES as usize)],
            lengths: [BYTES; 2],
            window,
        };
        if !audio::configure(HANDLE, &config) {
            return false;
        }
        STARTED = true;
        if !audio::start(HANDLE) {
            return false;
        }
        refill(0) && refill(1)
    }
}

pub unsafe fn refill(index: i32) -> bool {
    if !(0..=1).contains(&index) {
        return false;
    }
    unsafe {
        if HANDLE.is_null() {
            return false;
        }
        // A stopped cartridge may leave queued notifications; never overwrite unconsumed PCM.
        if audio::cursors(HANDLE)[index as usize] != BYTES {
            return true;
        }
        let start = runtime::micros();
        let success = fill(index as usize) && audio::submit(HANDLE, index as u32, BYTES);
        FILL_US = runtime::micros().wrapping_sub(start);
        MAX_FILL_US = MAX_FILL_US.max(FILL_US);
        if !success {
            return false;
        }
        SUBMITS = SUBMITS.saturating_add(1);
        true
    }
}

pub unsafe fn maintain(window: *mut c_void) -> bool {
    unsafe {
        // Reflex can drop notifications while busy; consumed cursors are authoritative.
        if !refill(0) || !refill(1) {
            return false;
        }
        let now = runtime::micros();
        if HANDLE.is_null() || now.wrapping_sub(LAST_POLL_US) < 250_000 {
            return true;
        }
        LAST_POLL_US = now;
        let mut status = audio::Status::default();
        if !audio::status(HANDLE, &mut status) {
            return false;
        }
        let stalled = (&mut *ptr::addr_of_mut!(WATCHDOG)).stalled(
            audio::cursors(HANDLE),
            status.byte_position,
            BYTES,
            now,
            1_000_000,
        );
        if stalled {
            RESTARTS = RESTARTS.saturating_add(1);
            return shutdown() && init(window);
        }
        true
    }
}

pub unsafe fn reset_counters() {
    unsafe {
        FILL_US = 0;
        MAX_FILL_US = 0;
        SUBMITS = 0;
        RESTARTS = 0;
    }
}

pub unsafe fn diagnostics() -> brain_tic80::debug::Audio {
    unsafe {
        let mut status = audio::Status::default();
        let state = if !HANDLE.is_null() && audio::status(HANDLE, &mut status) {
            Some(status.state)
        } else {
            None
        };
        brain_tic80::debug::Audio {
            fill_us: FILL_US,
            max_fill_us: MAX_FILL_US,
            submits: SUBMITS,
            restarts: RESTARTS,
            state,
        }
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
