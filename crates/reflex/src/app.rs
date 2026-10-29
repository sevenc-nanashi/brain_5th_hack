use core::ffi::c_void;

/// Context passed in r0 by the additional-app launcher.
#[repr(C)]
pub struct AppContext {
    pub reserved: *const c_void,
    pub exec_path: *const u8,
    pub presentation: *mut c_void,
    pub reserved2: *const c_void,
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
