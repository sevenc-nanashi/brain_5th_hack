use core::{ffi::c_void, mem::MaybeUninit, ptr};

core::arch::global_asm!(
    ".arm",
    ".section .text.on_tic_stack,\"ax\"",
    ".balign 4",
    ".global on_tic_stack",
    ".type on_tic_stack,%function",
    "on_tic_stack:",
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
struct Stack([u8; 256 * 1024]);
static mut STACK: Stack = Stack([0; 256 * 1024]);

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
        fn on_tic_stack(
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
        on_tic_stack(
            invoke::<F, R>,
            (&mut call as *mut Call<F, R>).cast(),
            ptr::addr_of_mut!(STACK)
                .cast::<u8>()
                .add(size_of::<Stack>()),
        );
        call.result.assume_init()
    }
}
