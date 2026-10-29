//! Direct RGB565 framebuffer access for PW-H1 (854 × 480).
pub const WIDTH: usize = 854;
pub const HEIGHT: usize = 480;

/// Convert landscape coordinates into the rotated panel's pixel offset.
pub const fn framebuffer_index(x: usize, y: usize) -> usize {
    assert!(x < WIDTH && y < HEIGHT);
    x * HEIGHT + (HEIGHT - 1 - y)
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
const FRAMEBUFFER: usize = 0x60a0_0008;

/// Write one RGB565 pixel; call `present` after drawing.
///
/// # Safety
/// Requires PW-H1's framebuffer mapping and exclusive access to the display.
#[cfg(all(target_arch = "arm", target_os = "none"))]
pub unsafe fn write_pixel(x: usize, y: usize, color: u16) {
    let offset = framebuffer_index(x, y);
    unsafe { core::ptr::write_volatile((FRAMEBUFFER as *mut u16).add(offset), color) };
}

/// Clean the framebuffer cache lines and request a display update.
///
/// # Safety
/// Requires privileged ARM execution, PW-H1's framebuffer/MMIO mappings,
/// and exclusive display access through completion of the update.
#[cfg(all(target_arch = "arm", target_os = "none"))]
pub unsafe fn present() {
    use core::{arch::asm, ptr::write_volatile};
    const DISPLAY_UPDATE: *mut u32 = 0x40aa_0004 as *mut u32;
    unsafe {
        let ctr: u32;
        asm!("mrc p15, 0, {ctr}, c0, c0, 1", ctr = out(reg) ctr, options(nostack, preserves_flags));
        let line_size = 4usize << ((ctr >> 16) & 15);
        let mut address = FRAMEBUFFER & !(line_size - 1);
        while address < FRAMEBUFFER + WIDTH * HEIGHT * 2 {
            asm!("mcr p15, 0, {address}, c7, c10, 1", address = in(reg) address, options(nostack, preserves_flags));
            address += line_size;
        }
        asm!("dsb sy", options(nostack, preserves_flags));
        write_volatile(DISPLAY_UPDATE, 1);
        asm!("dsb sy", options(nostack, preserves_flags));
    }
}
