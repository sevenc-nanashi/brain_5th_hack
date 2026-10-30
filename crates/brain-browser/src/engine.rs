use core::{
    ffi::{c_char, c_void},
    mem::MaybeUninit,
    ptr,
};

unsafe extern "C" {
    pub fn brain_init(url: *const c_char) -> i32;
    pub fn brain_tick(milliseconds: u64) -> i32;
    pub fn brain_draw(pixels: *mut u16) -> i32;
    pub fn brain_click(x: i32, y: i32);
    pub fn brain_scroll(x: i32, y: i32, delta: i32);
    pub fn brain_back();
    pub fn brain_stop();
    pub fn brain_status() -> *const c_char;
    pub fn brain_request(url: *mut *const c_char) -> u32;
    pub fn brain_response(id: u32, kind: i32, status: i32, data: *const u8, length: usize) -> i32;
}

core::arch::global_asm!(
    ".arm",
    ".section .text.on_browser_stack,\"ax\"",
    ".balign 4",
    ".global on_browser_stack",
    ".type on_browser_stack,%function",
    "on_browser_stack:",
    "push {{r4, lr}}",
    "mov r4, sp",
    "mov sp, r2",
    "mov r3, r0",
    "mov r0, r1",
    "blx r3",
    "mov sp, r4",
    "pop {{r4, pc}}",
);
#[repr(C, align(16))]
struct Stack([u8; 512 * 1024]);
static mut STACK: Stack = Stack([0; 512 * 1024]);

// The Application serialises all calls, including drawing and USB callbacks.
pub unsafe fn run<R, F: FnOnce() -> R>(f: F) -> R {
    struct Call<F, R> {
        f: Option<F>,
        result: MaybeUninit<R>,
    }
    unsafe extern "C" fn invoke<F: FnOnce() -> R, R>(p: *mut c_void) {
        let call = unsafe { &mut *p.cast::<Call<F, R>>() };
        call.result.write(call.f.take().unwrap()());
    }
    unsafe extern "C" {
        fn on_browser_stack(
            call: unsafe extern "C" fn(*mut c_void),
            context: *mut c_void,
            top: *mut u8,
        );
    }
    let mut call = Call {
        f: Some(f),
        result: MaybeUninit::uninit(),
    };
    unsafe {
        on_browser_stack(
            invoke::<F, R>,
            (&mut call as *mut Call<F, R>).cast(),
            ptr::addr_of_mut!(STACK)
                .cast::<u8>()
                .add(size_of::<Stack>()),
        );
        call.result.assume_init()
    }
}
