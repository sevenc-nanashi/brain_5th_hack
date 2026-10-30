use alloc::alloc::{alloc, dealloc, realloc};
use core::{alloc::Layout, ffi::c_void, ptr};

const HEADER: usize = 16;
#[unsafe(no_mangle)]
extern "C" fn tic_panic() -> ! {
    panic!("TIC-80 allocation or C runtime failure")
}
#[unsafe(no_mangle)]
extern "C" fn tic_alloc(size: usize) -> *mut c_void {
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
unsafe extern "C" fn tic_free(p: *mut c_void) {
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
unsafe extern "C" fn tic_realloc(p: *mut c_void, size: usize) -> *mut c_void {
    if p.is_null() {
        return tic_alloc(size);
    }
    if size == 0 {
        unsafe {
            tic_free(p);
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
