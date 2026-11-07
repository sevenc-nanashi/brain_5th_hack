use core::ffi::c_void;

/// Context passed in r0 by the additional-app launcher.
#[repr(C)]
pub struct AppContext {
    pub reserved: *const c_void,
    pub exec_path: *const u8,
    pub presentation: *mut c_void,
    pub reserved2: *const c_void,
}

/// CDTimer's independent load slot on V690_H1.
pub const TIMER_SLOT: u32 = 3;

/// # Safety
/// V690_H1 only; call from the PEG thread. This reads SharpLib's slot registry.
#[cfg(all(target_arch = "arm", target_os = "none"))]
pub unsafe fn timer_slot_in_use() -> bool {
    unsafe { (0x8604_30e0usize as *const u32).read_volatile() != 0 }
}

/// Load a separately linked image into CDTimer's slot. Returns null if occupied
/// or loading fails. SharpLib copies the path and argument before returning.
///
/// # Safety
/// V690_H1, PEG thread only. The image must use entry 0x83400000 and independent
/// data at 0x86c00000. Its callbacks must never refer to the launching app.
#[cfg(all(target_arch = "arm", target_os = "none"))]
pub unsafe fn launch_timer_slot(
    presentation: *mut c_void,
    path: &core::ffi::CStr,
) -> *const AppContext {
    unsafe {
        if timer_slot_in_use() {
            return core::ptr::null();
        }
        let launch: unsafe extern "C" fn(
            *mut c_void,
            *const u8,
            *const u8,
            u32,
        ) -> *const AppContext = core::mem::transmute(0x8401_0138usize);
        launch(
            presentation,
            path.as_ptr().cast(),
            c"".as_ptr().cast(),
            TIMER_SLOT,
        )
    }
}

/// Notify the launcher that this app has finished. This function can return.
///
/// # Safety
/// Requires V690_H1's SharpLib at its normal address and the live, original
/// launcher context. Release app-owned windows, timers and resources first;
/// do not access released app resources or notify exit again afterward.
#[cfg(all(target_arch = "arm", target_os = "none"))]
pub unsafe fn exit(context: *const AppContext) {
    unsafe {
        let exit_app: unsafe extern "C" fn(*const u8) = core::mem::transmute(0x8401_013cusize);
        exit_app((*context).exec_path);
    }
}

#[cfg(target_pointer_width = "32")]
const _: () = {
    assert!(core::mem::size_of::<AppContext>() == 16);
    assert!(core::mem::offset_of!(AppContext, exec_path) == 4);
    assert!(core::mem::offset_of!(AppContext, presentation) == 8);
};
