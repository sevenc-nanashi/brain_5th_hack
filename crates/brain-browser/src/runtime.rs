use alloc::alloc::{alloc, dealloc, realloc};
use core::{
    alloc::Layout,
    ffi::{CStr, c_char, c_void},
    net::IpAddr,
    ptr,
};

const HEADER: usize = 16;
static mut CLOCK: Option<(u64, u64)> = None;
pub fn set_clock(seconds: u32) {
    unsafe {
        CLOCK = Some((seconds as u64 * 1_000_000, reflex::runtime::micros()));
    }
}
#[unsafe(no_mangle)]
extern "C" fn browser_micros() -> u64 {
    unsafe { reflex::runtime::micros() }
}
#[unsafe(no_mangle)]
extern "C" fn browser_wall_micros() -> u64 {
    let now = browser_micros();
    unsafe {
        match CLOCK {
            Some((epoch, at)) => epoch + now.wrapping_sub(at),
            None => now,
        }
    }
}
#[unsafe(no_mangle)]
extern "C" fn browser_panic() -> ! {
    panic!("NetSurf C runtime failure")
}
#[unsafe(no_mangle)]
extern "C" fn browser_alloc(size: usize) -> *mut c_void {
    let Some(total) = size.max(1).checked_add(HEADER) else {
        return ptr::null_mut();
    };
    let Ok(layout) = Layout::from_size_align(total, HEADER) else {
        return ptr::null_mut();
    };
    let p = unsafe { alloc(layout) };
    if p.is_null() {
        return p.cast();
    }
    unsafe {
        p.cast::<usize>().write(total);
        p.add(HEADER).cast()
    }
}
#[unsafe(no_mangle)]
unsafe extern "C" fn browser_free(p: *mut c_void) {
    if !p.is_null() {
        unsafe {
            let base = p.cast::<u8>().sub(HEADER);
            dealloc(
                base,
                Layout::from_size_align_unchecked(base.cast::<usize>().read(), HEADER),
            );
        }
    }
}
#[unsafe(no_mangle)]
unsafe extern "C" fn browser_realloc(p: *mut c_void, size: usize) -> *mut c_void {
    if p.is_null() {
        return browser_alloc(size);
    }
    if size == 0 {
        unsafe {
            browser_free(p);
        }
        return ptr::null_mut();
    }
    let Some(total) = size.checked_add(HEADER) else {
        return ptr::null_mut();
    };
    if Layout::from_size_align(total, HEADER).is_err() {
        return ptr::null_mut();
    }
    unsafe {
        let base = p.cast::<u8>().sub(HEADER);
        let old = Layout::from_size_align_unchecked(base.cast::<usize>().read(), HEADER);
        let next = realloc(base, old, total);
        if next.is_null() {
            return ptr::null_mut();
        }
        next.cast::<usize>().write(total);
        next.add(HEADER).cast()
    }
}
#[unsafe(no_mangle)]
unsafe extern "C" fn inet_pton(family: i32, source: *const c_char, dest: *mut c_void) -> i32 {
    let Ok(s) = unsafe { CStr::from_ptr(source) }.to_str() else {
        return 0;
    };
    match s.parse::<IpAddr>() {
        Ok(IpAddr::V4(a)) if family == 2 => {
            unsafe {
                ptr::copy_nonoverlapping(a.octets().as_ptr(), dest.cast(), 4);
            }
            1
        }
        Ok(IpAddr::V6(a)) if family == 10 => {
            unsafe {
                ptr::copy_nonoverlapping(a.octets().as_ptr(), dest.cast(), 16);
            }
            1
        }
        _ => 0,
    }
}
