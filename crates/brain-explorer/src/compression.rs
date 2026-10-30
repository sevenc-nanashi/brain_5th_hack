use miniz_oxide::{
    DataFormat,
    deflate::core::{CompressorOxide, TDEFLFlush, TDEFLStatus, compress},
};

#[cfg(target_arch = "arm")]
core::arch::global_asm!(
    ".arm",
    ".section .text.on_compression_stack,\"ax\"",
    ".balign 4",
    ".global on_compression_stack",
    ".type on_compression_stack,%function",
    "on_compression_stack:",
    "push {{r4, lr}}",
    "mov r4, sp",
    "mov sp, r2",
    "mov r3, r0",
    "mov r0, r1",
    "blx r3",
    "mov sp, r4",
    "pop {{r4, pc}}",
);

#[cfg(target_arch = "arm")]
#[repr(C, align(16))]
struct Stack([u8; 128 * 1024]);
#[cfg(target_arch = "arm")]
static mut STACK: Stack = Stack([0; 128 * 1024]);

struct Block<'a> {
    input: &'a [u8],
    output: &'a mut [u8],
    written: usize,
}

#[inline(never)]
unsafe extern "C" fn encode(context: *mut core::ffi::c_void) {
    let block = unsafe { &mut *context.cast::<Block<'_>>() };
    let mut compressor = CompressorOxide::default();
    compressor.set_format_and_level(DataFormat::Zlib, 1);
    let (status, consumed, written) = compress(
        &mut compressor,
        block.input,
        block.output,
        TDEFLFlush::Finish,
    );
    assert_eq!(status, TDEFLStatus::Done);
    assert_eq!(consumed, block.input.len());
    block.written = written;
}

/// Serialized by Sender's Busy state; like Doom, avoid using the PEG stack for large frames.
///
/// # Safety
/// Must not be called concurrently or recursively: the ARM compression stack is shared.
pub unsafe fn encode_block(input: &[u8], output: &mut [u8]) -> usize {
    let mut block = Block {
        input,
        output,
        written: 0,
    };
    #[cfg(target_arch = "arm")]
    unsafe {
        unsafe extern "C" {
            fn on_compression_stack(
                call: unsafe extern "C" fn(*mut core::ffi::c_void),
                context: *mut core::ffi::c_void,
                top: *mut u8,
            );
        }
        on_compression_stack(
            encode,
            (&mut block as *mut Block<'_>).cast(),
            core::ptr::addr_of_mut!(STACK)
                .cast::<u8>()
                .add(size_of::<Stack>()),
        );
    }
    #[cfg(not(target_arch = "arm"))]
    unsafe {
        encode((&mut block as *mut Block<'_>).cast());
    }
    block.written
}
