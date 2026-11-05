//! Experimental V690_H1 SharpLib operation bar ABI. Names are provisional.
//! See PCConnect offsets 0x15884..0x159bc and SharpLib 0x111390..0x112740.
use core::ffi::c_void;

pub mod image;

/// SharpLib source bitmap (flags precede bpp, unlike the existing PEG wrapper).
/// Keep the RGB565 pixels alive while SharpLib reads this descriptor.
#[repr(C)]
pub struct Bitmap {
    pub flags: u8,
    pub bpp: u8,
    pub width: u16,
    pub height: u16,
    pub reserved: u16,
    pub refcount: u32,
    pub data: *const u16,
}

#[repr(C)]
pub struct Config {
    pub size: u32,
    pub slots_and_flags: u32,
    pub background: *const c_void,
    pub background_color: u32,
    pub leading_width: u16,
    pub unknown_12: u16,
    pub boundary_width: u16,
    pub unknown_16: u8,
    pub unknown_17: u8,
    pub boundary: *const c_void,
    pub boundary_color: u32,
}

impl Config {
    /// Values used by PCConnect, with a caller-selected slot count.
    pub const fn pcconnect(slots: u8) -> Self {
        assert!(slots != 0);
        Self {
            size: 32,
            slots_and_flags: slots as u32,
            background: core::ptr::null(),
            background_color: 0x001f1e22,
            leading_width: 0,
            unknown_12: 0,
            boundary_width: 1,
            unknown_16: 6,
            unknown_17: 6,
            boundary: core::ptr::null(),
            boundary_color: 0x00ffffff,
        }
    }
}

#[cfg(target_pointer_width = "32")]
const _: () = {
    assert!(core::mem::size_of::<Config>() == 32);
    assert!(core::mem::offset_of!(Config, boundary) == 0x18);
    assert!(core::mem::size_of::<Bitmap>() == 16);
    assert!(core::mem::offset_of!(Bitmap, data) == 12);
};

/// All calls require V690_H1 and the PEG event context. Keep the notification
/// target and referenced resources alive until destruction; destroy the bar
/// before its target window. Handles must come from `create` and remain live.
#[cfg(all(feature = "experimental-peg", target_arch = "arm", target_os = "none"))]
pub mod raw {
    use super::*;

    /// # Safety
    /// The module requirements apply. `config` must be valid or null (host defaults).
    /// The meanings of `unknown1` and `unknown3` are unknown; verified callers use 0.
    pub unsafe fn create(
        target: *mut c_void,
        unknown1: u32,
        config: *const Config,
        unknown3: u32,
    ) -> *mut c_void {
        let call: unsafe extern "C" fn(*mut c_void, u32, *const Config, u32) -> *mut c_void =
            unsafe { core::mem::transmute(0x8401_0640usize) };
        unsafe { call(target, unknown1, config, unknown3) }
    }

    /// # Safety
    /// The module requirements apply. Do not use the handle after this call.
    pub unsafe fn destroy(bar: *mut c_void) -> i32 {
        let call: unsafe extern "C" fn(*mut c_void) -> i32 =
            unsafe { core::mem::transmute(0x8401_0644usize) };
        unsafe { call(bar) }
    }

    /// # Safety
    /// The module requirements apply. Flags are raw: 1/2 clear/set one visibility
    /// gate, 4/8 clear/set the other; PCConnect initially uses 0x0a.
    pub unsafe fn control(bar: *mut c_void, flags: u32) -> i32 {
        let call: unsafe extern "C" fn(*mut c_void, u32) -> i32 =
            unsafe { core::mem::transmute(0x8401_0648usize) };
        unsafe { call(bar, flags) }
    }

    /// # Safety
    /// The module requirements apply. Pass 1 to defer updates, 0 to resume.
    pub unsafe fn suspend_updates(bar: *mut c_void, suspend: u32) -> i32 {
        let call: unsafe extern "C" fn(*mut c_void, u32) -> i32 =
            unsafe { core::mem::transmute(0x8401_0658usize) };
        unsafe { call(bar, suspend) }
    }

    /// Returns an item ID, or zero on failure. Parent zero selects the root.
    /// # Safety
    /// The module requirements apply. `item` must point to a fully initialized
    /// SharpLib descriptor large enough for its flags and state count. Its full
    /// layout is not yet verified; PCConnect uses a zeroed 0x220-byte descriptor.
    pub unsafe fn add_item(bar: *mut c_void, parent: u32, item: *const c_void) -> u32 {
        let call: unsafe extern "C" fn(*mut c_void, u32, *const c_void) -> u32 =
            unsafe { core::mem::transmute(0x8401_0664usize) };
        unsafe { call(bar, parent, item) }
    }
}
