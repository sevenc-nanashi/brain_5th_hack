#![no_std]
#![no_main]

use core::{arch::global_asm, panic::PanicInfo};

use brain_hello_world::render;
use reflex::{
    app::{self, AppContext},
    display,
};

// The launcher supplies the stack. This demo has no writable static data.
global_asm!(
    ".section .text.entry,\"ax\"",
    ".arm",
    ".balign 4",
    ".global _start",
    ".type _start,%function",
    "_start:",
    "push {{r4, lr}}",
    "bl hello_main",
    "pop {{r4, pc}}",
);

#[unsafe(no_mangle)]
extern "C" fn hello_main(context: *const AppContext) {
    // SAFETY: The launcher supplies privileged execution and the PW-H1 mappings.
    unsafe {
        render(|x, y, color| display::write_pixel(x, y, color));
        display::present();
    }
    // A finite display pause; its duration depends on the device clock.
    for _ in 0..0x1000_0000u32 {
        core::hint::spin_loop();
    }
    // SAFETY: Match PCConnect's ctx->exec_path argument and SharpLib export.
    // This posts EVENT_APP_FINISH; returning alone does not notify the launcher.
    unsafe {
        app::exit(context);
    }
}

#[panic_handler]
fn panic(_: &PanicInfo<'_>) -> ! {
    loop {
        core::hint::spin_loop();
    }
}
