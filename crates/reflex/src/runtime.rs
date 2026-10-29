//! Experimental fifth-generation runtime services used by native apps.
//! Allocation entries also occur in V690_H1 PCConnect's import thunks.
use core::{arch::asm, ffi::c_void};

/// Allocate storage using the system allocator (null on failure).
///
/// # Safety
/// Requires V690_H1's runtime at its normal address. Release with `free`.
pub unsafe fn alloc(size: usize) -> *mut c_void {
    unsafe {
        let call: unsafe extern "C" fn(usize) -> *mut c_void =
            core::mem::transmute(0x6000_63c4usize);
        call(size)
    }
}

/// Release system-allocated storage.
///
/// # Safety
/// `ptr` must be a live allocation from `alloc`, with no remaining users.
pub unsafe fn free(ptr: *mut c_void) {
    unsafe {
        let call: unsafe extern "C" fn(*mut c_void) -> i32 = core::mem::transmute(0x6000_63c8usize);
        call(ptr);
    }
}

/// Read the 1 MHz timestamp counter used by the x1 event_dumper.
///
/// # Safety
/// Requires privileged execution and the fifth-generation TSTMR mapping/clock.
/// Interrupts are masked only across the two reads, then their state is restored.
pub unsafe fn micros() -> u64 {
    unsafe {
        let low: u32;
        let high: u32;
        // A competing timer read in an interrupt would replace the latched high word.
        asm!(
            "mrs {saved}, cpsr",
            "cpsid if",
            "ldr {low}, [{address}]",
            "ldr {high}, [{address}, #4]",
            "msr cpsr_c, {saved}",
            address = in(reg) 0x410a_3c00usize,
            saved = out(reg) _, low = out(reg) low, high = out(reg) high,
            options(nostack, preserves_flags),
        );
        (u64::from(high) << 32) | u64::from(low)
    }
}

/// Mask interrupts only while changing allocator bookkeeping.
#[cfg(feature = "alloc")]
pub(crate) unsafe fn critical_section<R>(operation: impl FnOnce() -> R) -> R {
    unsafe {
        let saved: u32;
        asm!("mrs {saved}, cpsr", "cpsid if", saved = out(reg) saved, options(nostack, preserves_flags));
        let result = operation();
        asm!("msr cpsr_c, {saved}", saved = in(reg) saved, options(nostack, preserves_flags));
        result
    }
}
