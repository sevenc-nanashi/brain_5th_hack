//! Runtime used by #[reflex::main]. Single PEG thread, one owned window.
extern crate alloc;
use crate::{
    Action, Application, Event, TouchPhase,
    allocator::SystemAllocator,
    app::{self, AppContext},
    display::{HEIGHT, WIDTH},
    peg::{self, raw},
    runtime,
};
use alloc::{boxed::Box, vec, vec::Vec};
use core::{ffi::c_void, ptr};

struct State {
    application: Box<dyn Application>,
    frame: Vec<u16>,
    context: *const AppContext,
    window: *mut c_void,
    original: *const usize,
    previous_focus: *mut c_void,
    allocator: &'static SystemAllocator,
}
static mut STATE: *mut State = ptr::null_mut();
static mut VTABLE: [usize; 80] = [0; 80];
static mut BASE_MESSAGE: Option<peg::MessageFn> = None;
static mut BUSY: bool = false;
static mut CLOSING: bool = false;
static mut CLOSE_PENDING: bool = false;

/// # Safety
/// Called once by the ARM entry after data initialization, with the launcher's
/// live context. The app must not register external callbacks that outlive Exit.
/// The caller must also satisfy the safety requirements of `create`.
pub unsafe fn start<A: Application>(
    context: *const AppContext,
    create: unsafe fn() -> A,
    allocator: &'static SystemAllocator,
) {
    unsafe {
        BUSY = true;
        let application = Box::new(create());
        let frame = vec![0; WIDTH * HEIGHT];
        let window = runtime::alloc(0x108);
        if window.is_null() {
            panic!("Cannot allocate PEG window");
        }
        ptr::write_bytes(window.cast::<u8>(), 0, 0x108);
        raw::dec_window_ctor(window, 0x10);
        raw::rect_set(window.cast::<u8>().add(0x0c).cast(), 0, 0, 853, 479);
        raw::adjust_client(window);
        let original = *window.cast::<*const usize>();
        ptr::copy_nonoverlapping(original, ptr::addr_of_mut!(VTABLE).cast(), 80);
        BASE_MESSAGE = Some(core::mem::transmute::<usize, peg::MessageFn>(
            *original.add(peg::VFUNC_MESSAGE),
        ));
        VTABLE[peg::VFUNC_MESSAGE] = message as *const () as usize;
        VTABLE[peg::VFUNC_DRAW] = draw as *const () as usize;
        *window.cast::<*const usize>() = ptr::addr_of!(VTABLE).cast();
        let presentation = (*context).presentation;
        let previous_focus = presentation
            .cast::<u8>()
            .add(0x18c)
            .cast::<*mut c_void>()
            .read();
        STATE = Box::into_raw(Box::new(State {
            application,
            frame,
            context,
            window,
            original,
            previous_focus,
            allocator,
        }));
        let table = *presentation.cast::<*const usize>();
        let add: unsafe extern "C" fn(*mut c_void, *mut c_void, i32) =
            core::mem::transmute(*table.add(3));
        add(presentation, window, 1);
        raw::set_current(presentation, window);
        let action = (*STATE).application.mounted(crate::Window {
            raw: window,
            presentation,
        });
        BUSY = false;
        if CLOSE_PENDING || action == Action::Exit {
            close();
        } else {
            present(window.cast::<u8>().add(0x0c).cast());
        }
    }
}

unsafe fn close() {
    unsafe {
        if CLOSING || STATE.is_null() {
            return;
        }
        CLOSING = true;
        BUSY = true;
        CLOSE_PENDING = false;
        if !(*STATE).application.can_exit() {
            CLOSING = false;
            BUSY = false;
            present((*STATE).window.cast::<u8>().add(0x0c).cast());
            return;
        }
        let state = STATE;
        STATE = ptr::null_mut();
        let context = (*state).context;
        let allocator = (*state).allocator;
        raw::set_current((*context).presentation, (*state).previous_focus);
        *(*state).window.cast::<*const usize>() = (*state).original;
        raw::destroy((*state).window, (*state).window);
        drop(Box::from_raw(state));
        allocator.release_all();
        app::exit(context);
    }
}

unsafe extern "C" fn message(window: *mut c_void, message: *const peg::Message) -> i32 {
    unsafe {
        if CLOSING {
            return 0;
        }
        let msg = &*message;
        if msg.kind == peg::PM_DESTROY {
            if BUSY {
                CLOSE_PENDING = true;
            } else {
                close();
            }
            return 0;
        }
        if BUSY || STATE.is_null() {
            return BASE_MESSAGE.unwrap()(window, message);
        }
        let event = match msg.kind {
            peg::PM_KEY if msg.param == peg::TOUCH_MOVE_PARAM => {
                let point = msg.data.point;
                Some(Event::Touch {
                    x: point.x,
                    y: point.y,
                    phase: TouchPhase::Move,
                })
            }
            peg::PM_KEY | peg::PM_KEY_DOWN | peg::PM_KEY_UP => Some(Event::Key {
                code: msg.param,
                down: msg.kind != peg::PM_KEY_UP,
                flags: msg.data.ext_params[0] as u32,
            }),
            peg::PM_LBUTTONDOWN | peg::PM_LBUTTONUP => {
                let point = msg.data.point;
                Some(Event::Touch {
                    x: point.x,
                    y: point.y,
                    phase: if msg.kind == peg::PM_LBUTTONDOWN {
                        TouchPhase::Down
                    } else {
                        TouchPhase::Up
                    },
                })
            }
            _ => None,
        };
        BUSY = true;
        let raw_action = (*STATE).application.raw_event(msg);
        let action = raw_action.or_else(|| event.map(|event| (*STATE).application.event(event)));
        BUSY = false;
        if CLOSE_PENDING || action == Some(Action::Exit) {
            close();
        } else if matches!(action, Some(Action::Redraw | Action::RedrawAndBubble)) {
            present(window.cast::<u8>().add(0x0c).cast());
            if action == Some(Action::RedrawAndBubble) && !CLOSING {
                return BASE_MESSAGE.unwrap()(window, message);
            }
        } else if raw_action.is_some() {
            return 0;
        } else {
            return BASE_MESSAGE.unwrap()(window, message);
        }
        0
    }
}

unsafe extern "C" fn draw(_: *mut c_void, invalid: *const peg::Rect) {
    unsafe {
        present(invalid);
    }
}
unsafe fn present(invalid: *const peg::Rect) {
    unsafe {
        if BUSY || CLOSING || STATE.is_null() {
            return;
        }
        BUSY = true;
        let state = STATE;
        (*state).application.before_draw(invalid.read());
        // End Rust borrows before entering host drawing, which can reenter us.
        (*state).application.draw(&mut (*state).frame);
        let mut bitmap = peg::Bitmap {
            bpp: 16,
            flags: 0,
            width: WIDTH as u16,
            height: HEIGHT as u16,
            reserved: 0,
            refcount: 1,
            data: (*state).frame.as_mut_ptr().cast(),
        };
        raw::blit((*state).window, invalid, &mut bitmap);
        BUSY = false;
        if CLOSE_PENDING {
            close();
        }
    }
}

pub fn panic(info: &core::panic::PanicInfo<'_>) -> ! {
    #[cfg(feature = "panic-screen")]
    crate::panic_screen::halt(info);
    #[cfg(not(feature = "panic-screen"))]
    {
        let _ = info;
        loop {
            core::hint::spin_loop();
        }
    }
}
