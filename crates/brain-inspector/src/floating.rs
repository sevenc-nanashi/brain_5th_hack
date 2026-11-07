//! CDTimer window experiment. All callbacks run on the PEG thread.
extern crate alloc;
use alloc::{format, vec, vec::Vec};
use core::{ffi::c_void, ptr};
use reflex::{
    display::{HEIGHT, WIDTH},
    peg::{self, raw},
    runtime,
};

const TIMER: u32 = 0x5053;
struct State {
    window: *mut c_void,
    owner: *mut c_void,
    presentation: *mut c_void,
    original: *const usize,
    table: [usize; 80],
    frame: Vec<u16>,
    ticks: u32,
    hidden: bool,
    busy: bool,
    close_pending: bool,
}
static mut STATE: State = State {
    window: ptr::null_mut(),
    owner: ptr::null_mut(),
    presentation: ptr::null_mut(),
    original: ptr::null(),
    table: [0; 80],
    frame: Vec::new(),
    ticks: 0,
    hidden: false,
    busy: false,
    close_pending: false,
};

pub unsafe fn active() -> bool {
    unsafe { !(*ptr::addr_of!(STATE)).window.is_null() }
}
pub unsafe fn hidden() -> bool {
    unsafe { (*ptr::addr_of!(STATE)).hidden }
}
pub unsafe fn busy() -> bool {
    unsafe { (*ptr::addr_of!(STATE)).busy }
}

pub unsafe fn create(owner: *mut c_void, presentation: *mut c_void, style: u32, status: u32) {
    unsafe {
        if busy() {
            return;
        }
        close();
        let s = ptr::addr_of_mut!(STATE);
        let window = runtime::alloc(0x190);
        assert!(!window.is_null(), "Cannot allocate floating window");
        ptr::write_bytes(window.cast::<u8>(), 0, 0x190);
        raw::floating_window_ctor(window, style);
        raw::rect_set(window.cast::<u8>().add(0x0c).cast(), 460, 64, 747, 215);
        raw::adjust_client(window);
        raw::remove_status(window, 0x4058);
        raw::add_status(window, status);
        (*s).window = window;
        (*s).owner = owner;
        (*s).presentation = presentation;
        (*s).ticks = 0;
        (*s).close_pending = false;
        (*s).frame = vec![0; WIDTH * HEIGHT];
        (*s).original = *window.cast::<*const usize>();
        ptr::copy_nonoverlapping((*s).original, ptr::addr_of_mut!((*s).table).cast(), 80);
        (*s).table[peg::VFUNC_DRAW] = draw as *const () as usize;
        (*s).table[peg::VFUNC_MESSAGE] = message as *const () as usize;
        *window.cast::<*const usize>() = ptr::addr_of!((*s).table).cast();
        let table = *presentation.cast::<*const usize>();
        let add: unsafe extern "C" fn(*mut c_void, *mut c_void, i32) =
            core::mem::transmute(*table.add(3));
        add(presentation, window, 1);
        raw::set_timer(window, TIMER, 10, 10);
    }
}

pub unsafe fn toggle_owner() {
    unsafe {
        if !active() {
            return;
        }
        let s = ptr::addr_of_mut!(STATE);
        (*s).hidden = !(*s).hidden;
        // CDTimer clears keyboard focusability; the hidden-panel test needs keys.
        if (*s).hidden {
            raw::add_status((*s).window, 0x40);
        }

        raw::set_visible((*s).presentation, (*s).owner, !(*s).hidden);
        raw::move_focus(
            (*s).presentation,
            if (*s).hidden { (*s).window } else { (*s).owner },
        );
        if !(*s).hidden {
            raw::remove_status((*s).window, 0x40);
        }
    }
}

pub unsafe fn close() {
    unsafe {
        let s = ptr::addr_of_mut!(STATE);
        if (*s).window.is_null() {
            return;
        }
        if (*s).busy {
            (*s).close_pending = true;
            return;
        }
        if (*s).hidden {
            toggle_owner();
        }
        let window = (*s).window;
        (*s).window = ptr::null_mut();
        raw::kill_timer(window, TIMER);
        raw::move_focus((*s).presentation, (*s).owner);
        *window.cast::<*const usize>() = (*s).original;
        raw::destroy(window, window);
        (*s).frame = Vec::new();
    }
}

unsafe extern "C" fn message(window: *mut c_void, message: *const peg::Message) -> i32 {
    unsafe {
        let s = ptr::addr_of_mut!(STATE);
        let msg = &*message;
        if msg.kind == peg::PM_DESTROY {
            close();
            return 0;
        }
        if (*s).busy {
            return 0;
        }
        if msg.kind == peg::PM_TIMER && msg.param == TIMER as u16 {
            (*s).ticks = (*s).ticks.wrapping_add(1);
            draw(window, window.cast::<u8>().add(0x0c).cast());
            return 0;
        }
        if matches!(msg.kind, peg::PM_KEY | peg::PM_KEY_DOWN | peg::PM_KEY_UP)
            && msg.param != peg::TOUCH_MOVE_PARAM
        {
            // Stay in the inspector's launch slot until resident loading is understood.
            let owner = (*s).owner;
            let table = *owner.cast::<*const usize>();
            let call: peg::MessageFn = core::mem::transmute(*table.add(peg::VFUNC_MESSAGE));
            let forwarded = peg::Message {
                source: msg.source,
                target: owner,
                kind: msg.kind,
                param: msg.param,
                data: msg.data,
                next: ptr::null_mut(),
            };
            return call(owner, &forwarded);
        }
        let base: peg::MessageFn = core::mem::transmute(*(*s).original.add(peg::VFUNC_MESSAGE));
        base(window, message)
    }
}

unsafe extern "C" fn draw(window: *mut c_void, invalid: *const peg::Rect) {
    unsafe {
        let s = ptr::addr_of_mut!(STATE);
        if (*s).busy || (*s).window.is_null() {
            return;
        }
        (*s).busy = true;
        let rect = window.cast::<u8>().add(0x0c).cast::<peg::Rect>().read();
        (*s).frame.fill(0x0230);
        let label = format!(
            "FLOATING WINDOW\nTICKS {}\nH: PANEL  D: CLOSE\nESC: EXIT",
            (*s).ticks
        );
        reflex::text::draw(
            |x, y, color| (&mut (*s).frame)[y * WIDTH + x] = color,
            usize::from(rect.left) + 12,
            usize::from(rect.top) + 12,
            &label,
            0xffff,
            0x0230,
            1,
        );
        let mut bitmap = peg::Bitmap {
            bpp: 16,
            flags: 0,
            width: WIDTH as u16,
            height: HEIGHT as u16,
            reserved: 0,
            refcount: 1,
            data: (*s).frame.as_mut_ptr().cast(),
        };
        raw::blit(window, invalid, &mut bitmap);
        (*s).busy = false;
        if (*s).close_pending {
            close();
        }
    }
}
