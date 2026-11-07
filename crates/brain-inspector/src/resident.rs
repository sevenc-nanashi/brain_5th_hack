extern crate alloc;
use alloc::ffi::CString;
use core::ffi::{CStr, c_void};
use reflex::app;

pub unsafe fn launch(presentation: *mut c_void, executable: *const u8) -> &'static str {
    unsafe {
        if app::timer_slot_in_use() {
            return "SLOT 3 BUSY";
        }
        let path = CStr::from_ptr(executable.cast()).to_bytes();
        let Some(separator) = path.iter().rposition(|&c| matches!(c, b'\\' | b'/')) else {
            return "INVALID APP PATH";
        };
        let mut resident = path[..=separator].to_vec();
        resident.extend_from_slice(b"Resident.bin");
        let resident = CString::new(resident).unwrap();
        // The native loader reserves its slot before opening the file.
        let Ok(metadata) = reflex::fs::metadata(&resident) else {
            return "RESIDENT.BIN MISSING";
        };
        if !(128..=0x0040_0000).contains(&metadata.size) {
            return "INVALID RESIDENT SIZE";
        }
        if app::launch_timer_slot(presentation, &resident).is_null() {
            "LOAD FAILED"
        } else if !app::timer_slot_in_use() {
            "RESIDENT EXITED"
        } else {
            "STARTED"
        }
    }
}
