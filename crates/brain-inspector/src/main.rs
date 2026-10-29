#![no_std]
#![no_main]
use brain_inspector::{
    Measurements,
    input::{Input, KeyEvent},
    render,
};
use core::{arch::global_asm, ffi::c_void, panic::PanicInfo, ptr};
use reflex::{
    app::{self, AppContext},
    display::{HEIGHT, WIDTH},
    peg::{self, raw},
    runtime,
};
global_asm!(include_str!("startup.S"), options(raw));
const TIMER: u32 = 0x5052;
// Provisional usable width observed on PW-H1.
const USABLE_WIDTH: usize = 800;
static mut FRAME: [u16; WIDTH * HEIGHT] = [0; WIDTH * HEIGHT];
static mut VTABLE: [usize; 80] = [0; 80];
static mut ORIGINAL: *const usize = ptr::null();
static mut WINDOW: *mut c_void = ptr::null_mut();
static mut CONTEXT: AppContext = AppContext {
    reserved: ptr::null(),
    exec_path: ptr::null(),
    presentation: ptr::null_mut(),
    reserved2: ptr::null(),
};
static mut FOCUS: *mut c_void = ptr::null_mut();
static mut BASE_MESSAGE: Option<peg::MessageFn> = None;
static mut MEASUREMENTS: Option<Measurements> = None;
static mut READY: bool = false;
static mut INPUT: Option<Input> = None;

unsafe fn rectangle(object: *mut c_void, offset: usize) -> [i16; 4] {
    unsafe { object.cast::<u8>().add(offset).cast::<[i16; 4]>().read() }
}
unsafe fn sample() {
    unsafe {
        let mut m = MEASUREMENTS.unwrap();
        raw::screen_rect(CONTEXT.presentation, m.screen.as_mut_ptr());
        m.pm_real = rectangle(CONTEXT.presentation, 0x0c);
        m.pm_client = rectangle(CONTEXT.presentation, 0x14);
        m.pm_clip = rectangle(CONTEXT.presentation, 0x1c);
        m.window_real = rectangle(WINDOW, 0x0c);
        m.window_client = rectangle(WINDOW, 0x14);
        m.window_clip = rectangle(WINDOW, 0x1c);
        MEASUREMENTS = Some(m);
    }
}
#[unsafe(no_mangle)]
unsafe extern "C" fn inspector_main(context: *const AppContext) {
    unsafe {
        CONTEXT = context.read();
        let window = runtime::alloc(0x108);
        if window.is_null() {
            app::exit(ptr::addr_of!(CONTEXT));
            return;
        }
        ptr::write_bytes(window.cast::<u8>(), 0, 0x108);
        raw::dec_window_ctor(window, 0x10);
        // Request the full panel so host clipping remains observable.
        raw::rect_set(window.cast::<u8>().add(0x0c).cast(), 0, 0, 853, 479);
        raw::adjust_client(window);
        WINDOW = window;
        MEASUREMENTS = Some(Measurements::default());
        INPUT = Some(Input::default());
        ORIGINAL = *window.cast::<*const usize>();
        ptr::copy_nonoverlapping(ORIGINAL, ptr::addr_of_mut!(VTABLE).cast(), 80);
        BASE_MESSAGE = Some(core::mem::transmute::<usize, peg::MessageFn>(
            *ORIGINAL.add(peg::VFUNC_MESSAGE),
        ));
        VTABLE[peg::VFUNC_MESSAGE] = message as *const () as usize;
        VTABLE[peg::VFUNC_DRAW] = draw as *const () as usize;
        *window.cast::<*const usize>() = ptr::addr_of!(VTABLE).cast();
        let pm = CONTEXT.presentation;
        FOCUS = pm.cast::<u8>().add(0x18c).cast::<*mut c_void>().read();
        let vtable = *pm.cast::<*const usize>();
        let add: unsafe extern "C" fn(*mut c_void, *mut c_void, i32) =
            core::mem::transmute(*vtable.add(3));
        add(pm, window, 1);
        raw::set_current(pm, window);
        READY = true;
        sample();
        present(window, window.cast::<u8>().add(0x0c).cast());
        raw::set_timer(window, TIMER, 10, 10);
    }
}
unsafe fn close() {
    unsafe {
        READY = false;
        raw::kill_timer(WINDOW, TIMER);
        raw::set_current(CONTEXT.presentation, FOCUS);
        *WINDOW.cast::<*const usize>() = ORIGINAL;
        raw::destroy(WINDOW, WINDOW);
        app::exit(ptr::addr_of!(CONTEXT));
    }
}
unsafe extern "C" fn message(window: *mut c_void, message: *const peg::Message) -> i32 {
    unsafe {
        let msg = &*message;
        if !READY {
            return BASE_MESSAGE.unwrap()(window, message);
        }
        match msg.kind {
            peg::PM_DESTROY => {
                close();
                return 0;
            }
            peg::PM_KEY if msg.param == peg::TOUCH_MOVE_PARAM => {
                let mut m = MEASUREMENTS.unwrap();
                let p = msg.data.point;
                m.touch = Some((p.x, p.y));
                m.touch_down = true;
                let mut input = INPUT.unwrap();
                input.touch(p.x, p.y);
                INPUT = Some(input);
                MEASUREMENTS = Some(m);
                present(window, window.cast::<u8>().add(0x0c).cast());
                // shortcut: suppress bubbling like PCConnect; verify the system gesture on device.
                if !input.window_clip {
                    return 0;
                }
            }
            peg::PM_KEY | peg::PM_KEY_DOWN | peg::PM_KEY_UP => {
                let mut m = MEASUREMENTS.unwrap();
                m.kind = msg.kind;
                m.key = msg.param;
                m.flags = msg.data.ext_params[0];
                if msg.kind != peg::PM_KEY_UP {
                    m.pressed_key = Some(msg.param);
                    m.pressed_flags = msg.data.ext_params[0];
                }
                MEASUREMENTS = Some(m);
                let mut input = INPUT.unwrap();
                let consumed = input.key(
                    KeyEvent {
                        key: msg.param,
                        kind: msg.kind,
                        flags: m.flags,
                    },
                    runtime::micros(),
                );
                INPUT = Some(input);
                if msg.kind != peg::PM_KEY_UP && msg.param == peg::key::ESCAPE && !input.locked {
                    close();
                    return 0;
                }
                present(window, window.cast::<u8>().add(0x0c).cast());
                if consumed
                    || matches!(
                        msg.param,
                        peg::key::LEFT
                            | peg::key::RIGHT
                            | peg::key::UP
                            | peg::key::DOWN
                            | peg::key::ENTER
                    )
                {
                    return 0;
                }
            }
            peg::PM_LBUTTONDOWN | peg::PM_LBUTTONUP => {
                let mut m = MEASUREMENTS.unwrap();
                let p = msg.data.point;
                m.touch = Some((p.x, p.y));
                m.touch_down = msg.kind == peg::PM_LBUTTONDOWN;
                let mut input = INPUT.unwrap();
                input.touch(p.x, p.y);
                INPUT = Some(input);
                MEASUREMENTS = Some(m);
                present(window, window.cast::<u8>().add(0x0c).cast());
                if !input.window_clip {
                    return 0;
                }
            }
            peg::PM_TIMER if msg.param == TIMER as u16 => {
                sample();
                present(window, window.cast::<u8>().add(0x0c).cast());
                return 0;
            }
            _ => {}
        }
        BASE_MESSAGE.unwrap()(window, message)
    }
}
unsafe extern "C" fn draw(window: *mut c_void, invalid: *const peg::Rect) {
    unsafe {
        let mut m = MEASUREMENTS.unwrap();
        m.invalid = Some(invalid.cast::<[i16; 4]>().read());
        MEASUREMENTS = Some(m);
        present(window, invalid);
    }
}
unsafe fn present(window: *mut c_void, invalid: *const peg::Rect) {
    unsafe {
        sample();
        let m = MEASUREMENTS.unwrap();
        render(
            &mut *ptr::addr_of_mut!(FRAME),
            &m,
            USABLE_WIDTH,
            &INPUT.unwrap(),
        );
        let mut bitmap = peg::Bitmap {
            bpp: 16,
            flags: 0,
            width: WIDTH as u16,
            height: HEIGHT as u16,
            reserved: 0,
            refcount: 1,
            data: ptr::addr_of_mut!(FRAME).cast(),
        };
        raw::blit(window, invalid, &mut bitmap);
    }
}
#[panic_handler]
fn panic(_: &PanicInfo<'_>) -> ! {
    loop {
        core::hint::spin_loop();
    }
}
