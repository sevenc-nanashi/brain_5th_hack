//! Allocation-free panic formatting and rendering, also usable by host previews.
use crate::{
    display::{HEIGHT, WIDTH},
    text,
};
use core::fmt::{self, Write};

pub struct Report {
    bytes: [u8; 192],
    length: usize,
    truncated: bool,
}
impl Default for Report {
    fn default() -> Self {
        Self::new()
    }
}
impl Report {
    pub const fn new() -> Self {
        Self {
            bytes: [0; 192],
            length: 0,
            truncated: false,
        }
    }
    pub fn as_str(&self) -> &str {
        // Only complete UTF-8 characters are copied by write_str.
        unsafe { core::str::from_utf8_unchecked(&self.bytes[..self.length]) }
    }
}
impl Write for Report {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        if self.truncated {
            return Ok(());
        }
        let capacity = self.bytes.len() - 3 - self.length;
        let mut count = capacity.min(value.len());
        while !value.is_char_boundary(count) {
            count -= 1;
        }
        self.bytes[self.length..self.length + count].copy_from_slice(&value.as_bytes()[..count]);
        self.length += count;
        if count != value.len() {
            self.bytes[self.length..self.length + 3].copy_from_slice(b"...");
            self.length += 3;
            self.truncated = true;
        }
        Ok(())
    }
}

/// Paint without allocating, touching PEG, or invoking application code.
pub fn render(report: &Report, mut pixel: impl FnMut(usize, usize, u16)) {
    const BG: u16 = 0x1082;
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            pixel(x, y, BG);
        }
    }
    text::draw(&mut pixel, 24, 24, "PANIC", 0xffff, BG, 2);
    let mut column = 0;
    let mut row = 0;
    for ch in report.as_str().chars() {
        if ch == '\n' || column == 36 {
            row += 1;
            column = 0;
            if ch == '\n' {
                continue;
            }
        }
        if row >= 11 {
            break;
        }
        let mut bytes = [0; 4];
        text::draw(
            &mut pixel,
            24 + column * 12,
            100 + row * 26,
            ch.encode_utf8(&mut bytes),
            0xffff,
            BG,
            1,
        );
        column += 1;
    }
    text::draw(&mut pixel, 24, 430, "HALTED", 0xfd20, BG, 1);
    #[cfg(feature = "panic-qr")]
    {
        use qrcodegen_no_heap::{QrCode, QrCodeEcc, Version};
        const SIZE: usize = Version::new(10).buffer_len();
        let mut temp = [0; SIZE];
        let mut output = [0; SIZE];
        match QrCode::encode_text(
            report.as_str(),
            &mut temp,
            &mut output,
            QrCodeEcc::Medium,
            Version::MIN,
            Version::new(10),
            None,
            false,
        ) {
            Ok(qr) => {
                let modules = qr.size() as usize;
                let scale = 4;
                let side = (modules + 8) * scale;
                let left = 500;
                let top = (HEIGHT - side) / 2;
                for y in 0..side {
                    for x in 0..side {
                        let mx = x as i32 / scale as i32 - 4;
                        let my = y as i32 / scale as i32 - 4;
                        pixel(
                            left + x,
                            top + y,
                            if qr.get_module(mx, my) { 0 } else { 0xffff },
                        );
                    }
                }
            }
            Err(_) => text::draw(&mut pixel, 500, 200, "QR ERROR", 0xfd20, BG, 1),
        }
    }
}

#[cfg(all(target_arch = "arm", target_os = "none"))]
pub fn halt(info: &core::panic::PanicInfo<'_>) -> ! {
    use core::sync::atomic::{AtomicBool, Ordering};
    static ENTERED: AtomicBool = AtomicBool::new(false);
    if !ENTERED.swap(true, Ordering::SeqCst) {
        let mut report = Report::new();
        if let Some(location) = info.location() {
            let _ = writeln!(
                report,
                "{}:{}:{}",
                location.file(),
                location.line(),
                location.column()
            );
        }
        let _ = write!(report, "{}", info.message());
        render(&report, |x, y, color| unsafe {
            crate::display::write_pixel(x, y, color)
        });
        unsafe {
            crate::display::present();
        }
    }
    loop {
        core::hint::spin_loop();
    }
}
