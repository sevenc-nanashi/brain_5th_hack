//! Experimental PEG ABI from Brain Hackers' event_dumper.
//!
//! Numbers/layouts are not yet verified against V690_H1. Unknown message types
//! remain valid; `PM_KEY_UP` is tentative even in the upstream source.
use core::ffi::c_void;

pub const PM_DESTROY: u16 = 6;
pub const PM_KEY: u16 = 9;
pub const PM_LBUTTONDOWN: u16 = 10;
pub const PM_LBUTTONUP: u16 = 11;
pub const PM_TIMER: u16 = 32;
pub const PM_KEY_DOWN: u16 = 33;
pub const PM_KEY_UP: u16 = 34;
/// PW-H1 touch drag: PM_KEY with this param; data.point is X/Y (16 bits each).
pub const TOUCH_MOVE_PARAM: u16 = 0x8000;
/// PW-H1 observation: key flags change from 0x800 to 0x840 with Shift active.
pub const KMOD_SHIFT: u32 = 0x40;
pub const KFLAG_PRESS: u32 = 0x800;
pub const KFLAG_HELD: u32 = 0xc00;

pub mod key {
    pub const ENTER: u16 = 0x0d;
    pub const ESCAPE: u16 = 0x1b;
    pub const BACKSPACE: u16 = 0x08;
    pub const SPACE: u16 = 0x20;
    /// 二重上。Shift 有効時は音量プラス。
    pub const PAGE_UP: u16 = 0x25c;
    /// 二重下。Shift 有効時は音量マイナス。
    pub const PAGE_DOWN: u16 = 0x25d;
    // PW-H1 device observation: up/left differ from event_dumper's labels.
    pub const LEFT: u16 = 0x260;
    pub const DOWN: u16 = 0x25f;
    pub const UP: u16 = 0x25e;
    pub const RIGHT: u16 = 0x261;
    pub const SHIFT: u16 = 0x265;
    /// ホーム。
    pub const HOME: u16 = 0x28a;
    /// 国語/漢字。
    pub const JAPANESE_DICTIONARY: u16 = 0x28b;
    /// 英和/和英。
    pub const ENGLISH_DICTIONARY: u16 = 0x28c;
    /// My辞書。
    pub const MY_DICTIONARY: u16 = 0x28d;
    /// 履歴。
    pub const HISTORY: u16 = 0x28e;
    /// 一覧から選ぶ。
    pub const SELECT_FROM_LIST: u16 = 0x28f;
    /// 音声。
    pub const AUDIO: u16 = 0x294;
    /// あA1/記号。
    pub const INPUT_MODE: u16 = 0x295;
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Point {
    pub x: u16,
    pub y: u16,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Rect {
    pub left: u16,
    pub top: u16,
    pub right: u16,
    pub bottom: u16,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub union MessageData {
    pub rect: Rect,
    pub point: Point,
    pub ext_params: [i32; 2],
    pub data: *mut c_void,
}

/// Keys use `param` for the code and `data.ext_params[0]` for flags.
/// Touch uses `data.point`; timers use `param` for the timer ID.
#[repr(C)]
pub struct Message {
    pub source: *mut c_void,
    pub target: *mut c_void,
    pub kind: u16,
    pub param: u16,
    pub data: MessageData,
    pub next: *mut Message,
}

#[repr(C, packed)]
pub struct Bitmap {
    pub bpp: u8,
    pub flags: u8,
    pub width: u16,
    pub height: u16,
    pub reserved: u16,
    pub refcount: u32,
    pub data: *mut u8,
}

pub type MessageFn = unsafe extern "C" fn(*mut c_void, *const Message) -> i32;
pub type BitmapFn = unsafe extern "C" fn(*mut c_void, i32, *mut Bitmap);
pub type EndDrawFn = unsafe extern "C" fn(*mut c_void);
pub const VFUNC_DRAW: usize = 12;
pub const VFUNC_MESSAGE: usize = 19;
pub const SCR_VF_BITMAP: usize = 4;
pub const SCR_VF_ENDDRAW: usize = 16;

#[cfg(target_pointer_width = "32")]
const _: () = {
    use core::mem::{offset_of, size_of};
    assert!(size_of::<Point>() == 4);
    assert!(size_of::<Rect>() == 8);
    assert!(size_of::<MessageData>() == 8);
    assert!(size_of::<Message>() == 24);
    assert!(offset_of!(Message, kind) == 8);
    assert!(offset_of!(Message, param) == 10);
    assert!(offset_of!(Message, data) == 12);
    assert!(offset_of!(Message, next) == 20);
    assert!(size_of::<Bitmap>() == 16);
    assert!(offset_of!(Bitmap, data) == 12);
};

/// Raw calls, enabled only by `experimental-peg` on bare-metal ARM.
///
/// # Safety
/// Before calling, verify each address and ABI against the running firmware.
/// Pass live PEG objects and correctly sized, aligned buffers. Calls must run
/// in the PEG event context; objects, callbacks and buffers must outlive their
/// registration/use. Destroy windows and stop timers before releasing storage.
/// Timer count/reset values are PEG ticks, not milliseconds.
// shortcut: window/message ABI still needs device validation before adding safe wrappers.
#[cfg(all(feature = "experimental-peg", target_arch = "arm", target_os = "none"))]
pub mod raw {
    use super::*;
    /// # Safety
    /// The address, object lifetime and event-context requirements in [`self`] apply.
    pub unsafe fn dec_window_ctor(window: *mut c_void, style: u16) -> *mut c_void {
        unsafe {
            let call: unsafe extern "C" fn(*mut c_void, u16) -> *mut c_void =
                core::mem::transmute(0x6000_486cusize);
            call(window, style)
        }
    }
    /// # Safety
    /// The address, object lifetime and event-context requirements in [`self`] apply.
    pub unsafe fn rect_set(rect: *mut i16, x1: i16, y1: i16, x2: i16, y2: i16) {
        unsafe {
            let call: unsafe extern "C" fn(*mut i16, i16, i16, i16, i16) =
                core::mem::transmute(0x6000_4130usize);
            call(rect, x1, y1, x2, y2)
        }
    }
    /// # Safety
    /// The address, object lifetime and event-context requirements in [`self`] apply.
    pub unsafe fn screen_rect(presentation: *mut c_void, out_rect: *mut i16) {
        unsafe {
            let call: unsafe extern "C" fn(*mut c_void, *mut i16) =
                core::mem::transmute(0x6000_5358usize);
            call(presentation, out_rect)
        }
    }
    /// # Safety
    /// The address, object lifetime and event-context requirements in [`self`] apply.
    pub unsafe fn adjust_client(window: *mut c_void) {
        unsafe {
            let call: unsafe extern "C" fn(*mut c_void) = core::mem::transmute(0x6000_4850usize);
            call(window)
        }
    }
    /// # Safety
    /// The address, object lifetime and event-context requirements in [`self`] apply.
    pub unsafe fn win_begin_draw(window: *mut c_void, rect: *const Rect) {
        unsafe {
            let call: unsafe extern "C" fn(*mut c_void, *const Rect) =
                core::mem::transmute(0x6000_4590usize);
            call(window, rect)
        }
    }
    /// # Safety
    /// The address, object lifetime and event-context requirements in [`self`] apply.
    pub unsafe fn win_base_draw(window: *mut c_void, rect: *const Rect) {
        unsafe {
            let call: unsafe extern "C" fn(*mut c_void, *const Rect) =
                core::mem::transmute(0x6000_602cusize);
            call(window, rect)
        }
    }
    /// # Safety
    /// The address, object lifetime and event-context requirements in [`self`] apply.
    pub unsafe fn get_screen() -> *mut c_void {
        unsafe {
            let call: unsafe extern "C" fn() -> *mut c_void =
                core::mem::transmute(0x6000_4144usize);
            call()
        }
    }
    /// Blit through PEG's backing store and flush its drawing transaction.
    ///
    /// # Safety
    /// Requires a live window, its invalid rectangle, and a bitmap whose pixel
    /// storage remains valid during the call. Call from the PEG event thread.
    pub unsafe fn blit(window: *mut c_void, invalid: *const Rect, bitmap: *mut Bitmap) {
        unsafe {
            win_begin_draw(window, invalid);
            let screen = get_screen();
            let vtable = *screen.cast::<*const usize>();
            let draw: BitmapFn = core::mem::transmute(*vtable.add(SCR_VF_BITMAP));
            let end: EndDrawFn = core::mem::transmute(*vtable.add(SCR_VF_ENDDRAW));
            draw(screen, 0, bitmap);
            end(screen);
        }
    }
    /// # Safety
    /// The address, object lifetime and event-context requirements in [`self`] apply.
    pub unsafe fn set_current(presentation: *mut c_void, target: *mut c_void) {
        unsafe {
            // PCConnect/AppMain.bin 0x1564c: inline current-window setter.
            presentation
                .cast::<u8>()
                .add(0x18c)
                .cast::<*mut c_void>()
                .write(target);
        }
    }
    /// # Safety
    /// The address, object lifetime and event-context requirements in [`self`] apply.
    pub unsafe fn set_timer(who: *mut c_void, id: u32, count: u32, reset: u32) {
        unsafe {
            let timers: unsafe extern "C" fn() -> *mut c_void =
                core::mem::transmute(0x6000_413cusize);
            let call: unsafe extern "C" fn(*mut c_void, *mut c_void, u32, u32, u32) =
                core::mem::transmute(0x6000_5d04usize);
            call(timers(), who, id, count, reset);
        }
    }
    /// # Safety
    /// The address, object lifetime and event-context requirements in [`self`] apply.
    pub unsafe fn kill_timer(who: *mut c_void, id: u32) {
        unsafe {
            let timers: unsafe extern "C" fn() -> *mut c_void =
                core::mem::transmute(0x6000_413cusize);
            let call: unsafe extern "C" fn(*mut c_void, *mut c_void, u32, u32) =
                core::mem::transmute(0x6000_5d08usize);
            call(timers(), who, id, 0);
        }
    }
    /// # Safety
    /// The address, object lifetime and event-context requirements in [`self`] apply.
    pub unsafe fn destroy(source: *mut c_void, target: *mut c_void) {
        unsafe {
            let call: unsafe extern "C" fn(*mut c_void, *mut c_void) =
                core::mem::transmute(0x6000_5c9cusize);
            call(source, target)
        }
    }
}
