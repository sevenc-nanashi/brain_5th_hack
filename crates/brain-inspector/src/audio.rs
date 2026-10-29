use brain_inspector::AudioMeasurements;
use core::{ffi::c_void, ptr};
use reflex::{audio, peg};

const BLOCK: usize = 32768;
static mut TONE: [[i16; BLOCK / 2]; 2] = [[0; BLOCK / 2]; 2];
static mut HANDLE: audio::Handle = ptr::null_mut();
static mut CONFIGURED: bool = false;
static mut STARTED: bool = false;

fn result(m: &mut AudioMeasurements, call: &'static str, ok: bool) -> bool {
    m.call = call;
    m.ok = ok;
    ok
}

pub unsafe fn stop(m: &mut AudioMeasurements) -> bool {
    unsafe {
        if HANDLE.is_null() {
            return true;
        }
        if STARTED {
            if !result(m, "STOP", audio::stop_wait(HANDLE)) {
                return false;
            }
            poll(m);
            if !result(m, "STOPPED", m.status_ok && m.state == 0) {
                return false;
            }
        }
        if !result(m, "CLOSE", audio::close(HANDLE)) {
            return false;
        }
        HANDLE = ptr::null_mut();
        CONFIGURED = false;
        STARTED = false;
        m.handle = 0;
        true
    }
}

pub unsafe fn play(window: *mut c_void, streaming: bool, m: &mut AudioMeasurements) {
    unsafe {
        if !stop(m) {
            return;
        }
        *m = AudioMeasurements {
            streaming,
            buffer_bytes: m.buffer_bytes,
            ..AudioMeasurements::default()
        };
        // Quiet 344.5 Hz square wave, identical on both channels; no engine involved.
        let samples = ptr::addr_of_mut!(TONE).cast::<i16>();
        for i in 0..BLOCK {
            samples
                .add(i)
                .write(if (i / 2) % 32 < 16 { 2048 } else { -2048 });
        }
        HANDLE = audio::open_playback();
        m.handle = HANDLE as u32;
        if !result(m, "OPEN", !HANDLE.is_null()) {
            return;
        }
        if m.buffer_bytes != BLOCK as u32
            && !result(
                m,
                "STAGING",
                audio::set_staging_bytes(HANDLE, m.buffer_bytes),
            )
        {
            return;
        }
        let config = audio::Config {
            flags: audio::PCM | audio::NOTIFY_WINDOW,
            total_bytes: if streaming {
                0x7fff_fffc
            } else {
                m.buffer_bytes * 2
            },
            sample_rate: 11025,
            bits: 16,
            channels: 2,
            buffers: [samples.cast(), samples.cast::<u8>().add(BLOCK)],
            lengths: [m.buffer_bytes; 2],
            window,
        };
        if !result(m, "CONFIG", audio::configure(HANDLE, &config)) {
            return;
        }
        CONFIGURED = true;
        STARTED = true;
        if !result(m, "START", audio::start(HANDLE)) {
            return;
        }
        if streaming {
            for index in 0..2 {
                if !submit(index, m) {
                    return;
                }
            }
        }
        poll(m);
    }
}

unsafe fn submit(index: u32, m: &mut AudioMeasurements) -> bool {
    unsafe {
        if !result(m, "SUBMIT", audio::submit(HANDLE, index, m.buffer_bytes)) {
            return false;
        }
        m.submitted[index as usize] = m.submitted[index as usize].wrapping_add(1);
        true
    }
}

pub unsafe fn event(msg: &peg::Message, m: &mut AudioMeasurements) {
    unsafe {
        m.events = m.events.wrapping_add(1);
        m.last_kind = msg.kind;
        m.last_param = msg.param;
        m.last_data = msg.data.ext_params;
        if msg.kind == audio::PM_AUDIO
            && msg.param == audio::BUFFER_CONSUMED
            && m.streaming
            && m.ok
            && STARTED
            && !HANDLE.is_null()
        {
            let index = msg.data.ext_params[0];
            if result(m, "INDEX", (0..=1).contains(&index)) {
                submit(index as u32, m);
            }
        }
    }
}

pub unsafe fn poll(m: &mut AudioMeasurements) {
    unsafe {
        m.volume = audio::volume();
        if HANDLE.is_null() || !CONFIGURED {
            return;
        }
        let mut status = audio::Status::default();
        m.status_ok = audio::status(HANDLE, &mut status);
        if m.status_ok {
            m.state = status.state;
            m.position_ms = status.position_ms;
            m.bytes = status.byte_position;
        }
        let words = HANDLE.cast::<u32>();
        m.staging = [
            words.add(0x20 / 4).read_volatile(),
            words.add(0x2c / 4).read_volatile(),
        ];
        m.cursors = [
            words.add(0x4c / 4).read_volatile(),
            words.add(0x58 / 4).read_volatile(),
        ];
    }
}

pub unsafe fn change_volume(up: bool, m: &mut AudioMeasurements) {
    unsafe {
        let level = audio::volume();
        if !result(m, "VOLUME", level <= 9) {
            return;
        }
        let next = if up {
            (level + 1).min(9)
        } else {
            level.saturating_sub(1)
        };
        m.volume_ok = audio::set_volume(next);
        m.volume = audio::volume();
    }
}

pub unsafe fn change_buffer(larger: bool, m: &mut AudioMeasurements) {
    let next = if larger {
        (m.buffer_bytes * 2).min(BLOCK as u32)
    } else {
        (m.buffer_bytes / 2).max(2048)
    };
    if next != m.buffer_bytes && unsafe { stop(m) } {
        *m = AudioMeasurements {
            buffer_bytes: next,
            volume: m.volume,
            volume_ok: m.volume_ok,
            ..AudioMeasurements::default()
        };
    }
}
