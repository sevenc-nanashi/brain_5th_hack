#![no_std]
#![no_main]
use core::{ffi::c_void, ptr};
use reflex::{
    app::{self, AppContext},
    display::{HEIGHT, WIDTH},
    peg::{self, raw},
    runtime,
};

core::arch::global_asm!(include_str!("startup.S"), options(raw));
const TIMER: u32 = 0x5253;
#[global_allocator]
static ALLOCATOR: reflex::allocator::SystemAllocator = reflex::allocator::SystemAllocator::new();
static mut CONTEXT: *const AppContext = ptr::null();
static mut WINDOW: *mut c_void = ptr::null_mut();
static mut ORIGINAL: *const usize = ptr::null();
static mut TABLE: [usize; 80] = [0; 80];
static mut FRAME: [u16; WIDTH * HEIGHT] = [0; WIDTH * HEIGHT];
static mut TICKS: u32 = 0;
static mut BUSY: bool = false;
static mut CLOSE_PENDING: bool = false;

#[unsafe(no_mangle)]
unsafe extern "C" fn resident_main(context: *const AppContext) {
    unsafe {
        CONTEXT = context;
        BUSY = true;
        let window = runtime::alloc(0x190);
        if window.is_null() {
            app::exit(context);
            return;
        }
        ptr::write_bytes(window.cast::<u8>(), 0, 0x190);
        raw::floating_window_ctor(window, 0x0800_0001);
        // CDTimer 0xa5708 / 0xa7d3c: host messages address this window by ID.
        window.cast::<u8>().add(0x54).cast::<u16>().write(0x2a1);
        raw::rect_set(window.cast::<u8>().add(0x0c).cast(), 460, 64, 747, 191);
        raw::adjust_client(window);
        raw::remove_status(window, 0x48);
        raw::add_status(window, 0x4010);
        WINDOW = window;
        ORIGINAL = *window.cast::<*const usize>();
        ptr::copy_nonoverlapping(ORIGINAL, ptr::addr_of_mut!(TABLE).cast(), 80);
        TABLE[peg::VFUNC_DRAW] = draw as *const () as usize;
        TABLE[peg::VFUNC_MESSAGE] = message as *const () as usize;
        *window.cast::<*const usize>() = ptr::addr_of!(TABLE).cast();
        let presentation = (*context).presentation;
        let table = *presentation.cast::<*const usize>();
        let add: unsafe extern "C" fn(*mut c_void, *mut c_void, i32) =
            core::mem::transmute(*table.add(3));
        add(presentation, window, 1);
        raw::set_timer(window, TIMER, 10, 10);
        BUSY = false;
        if CLOSE_PENDING {
            close();
        }
    }
}

unsafe fn close() {
    unsafe {
        if WINDOW.is_null() {
            return;
        }
        if BUSY {
            CLOSE_PENDING = true;
            return;
        }
        BUSY = true;
        let window = WINDOW;
        WINDOW = ptr::null_mut();
        raw::kill_timer(window, TIMER);
        *window.cast::<*const usize>() = ORIGINAL;
        raw::destroy(window, window);
        ALLOCATOR.release_all();
        // Leave the host's return-action field intact when another app is running.
        app::exit(CONTEXT);
    }
}

unsafe extern "C" fn message(window: *mut c_void, msg: *const peg::Message) -> i32 {
    unsafe {
        let msg_ref = &*msg;
        if matches!(msg_ref.kind, peg::PM_DESTROY | 0x8024)
            || msg_ref.kind == peg::PM_LBUTTONUP
            || (msg_ref.kind == 0x10e && msg_ref.param == 3)
        {
            close();
            return 0;
        }
        if BUSY || WINDOW.is_null() {
            return 0;
        }
        if msg_ref.kind == peg::PM_TIMER && msg_ref.param == TIMER as u16 {
            TICKS = TICKS.wrapping_add(1);
            draw(window, window.cast::<u8>().add(0x0c).cast());
            return 0;
        }
        if msg_ref.kind == 0x10e && matches!(msg_ref.param, 1 | 2) {
            raw::set_visible((*CONTEXT).presentation, window, msg_ref.param == 2);
            return 0;
        }
        let base: peg::MessageFn = core::mem::transmute(*ORIGINAL.add(peg::VFUNC_MESSAGE));
        base(window, msg)
    }
}

unsafe extern "C" fn draw(window: *mut c_void, invalid: *const peg::Rect) {
    unsafe {
        if BUSY || WINDOW.is_null() {
            return;
        }
        // The host's hide operation adds this status bit; keep counting silently.
        if window.cast::<u8>().add(0x4c).cast::<u32>().read() & 0x10000 != 0 {
            return;
        }
        BUSY = true;
        let frame = ptr::addr_of_mut!(FRAME).cast::<u16>();
        core::slice::from_raw_parts_mut(frame, WIDTH * HEIGHT).fill(0x0320);
        let rect = window.cast::<u8>().add(0x0c).cast::<peg::Rect>().read();
        let mut digits = *b"0000000000";
        let mut value = TICKS;
        for digit in digits.iter_mut().rev() {
            *digit = b'0' + (value % 10) as u8;
            value /= 10;
        }
        for (line, text) in [
            "RESIDENT / SLOT 3",
            core::str::from_utf8(&digits).unwrap(),
            "TAP TO CLOSE",
        ]
        .iter()
        .enumerate()
        {
            reflex::text::draw(
                |x, y, color| frame.add(y * WIDTH + x).write(color),
                usize::from(rect.left) + 12,
                usize::from(rect.top) + 12 + line * 30,
                text,
                0xffff,
                0x0320,
                1,
            );
        }
        let mut bitmap = peg::Bitmap {
            bpp: 16,
            flags: 0,
            width: WIDTH as u16,
            height: HEIGHT as u16,
            reserved: 0,
            refcount: 1,
            data: frame.cast(),
        };
        raw::blit(window, invalid, &mut bitmap);
        BUSY = false;
        if CLOSE_PENDING {
            close();
        }
    }
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo<'_>) -> ! {
    reflex::panic_screen::halt(info)
}
