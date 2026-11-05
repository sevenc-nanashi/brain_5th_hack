use brain_inspector::{BAR_BUTTONS, BarImage, BarMeasurements, bar_image};
use core::ffi::c_void;
use reflex::operation_bar::{Config, raw};
use reflex::peg::key;

pub const MESSAGE: u16 = 0x10b;

#[derive(Default)]
pub struct Bar {
    handle: *mut c_void,
    keys: u16,
}

impl Bar {
    pub unsafe fn close(&mut self, m: &mut BarMeasurements) {
        if !self.handle.is_null() {
            m.call = "DESTROY";
            m.result = unsafe { raw::destroy(self.handle) };
            self.handle = core::ptr::null_mut();
            m.handle = 0;
            m.items.fill(0);
            m.flags = 0;
        }
    }

    pub unsafe fn control(&self, flags: u32, m: &mut BarMeasurements) {
        if !self.handle.is_null() {
            m.call = "CONTROL";
            m.flags = flags;
            m.result = unsafe { raw::control(self.handle, flags) };
        }
    }

    unsafe fn create(&mut self, window: *mut c_void, m: &mut BarMeasurements) {
        if !self.handle.is_null() {
            return;
        }
        m.call = "CREATE";
        self.handle = unsafe { raw::create(window, 0, &Config::pcconnect(m.slots as u8), 0) };
        m.handle = self.handle as u32;
        m.result = i32::from(!self.handle.is_null());
        if self.handle.is_null() {
            return;
        }
        unsafe { raw::suspend_updates(self.handle, 1) };
        let normal = bar_image::bitmap(false);
        let pressed = bar_image::bitmap(true);
        for (i, button) in m.buttons.into_iter().take(m.slots).enumerate() {
            let (_, image) = BAR_BUTTONS[button];
            // shortcut: PCConnect's one-state image descriptor; other layouts need ABI analysis.
            let mut item = [0u32; 0x220 / 4];
            item[0] = 0x21;
            item[1] = u32::from(MESSAGE) | ((i as u32 + 1) << 16);
            item[4] = 52;
            item[5] = 52;
            match image {
                BarImage::Resource(normal_id, pressed_id) => {
                    item[0x24 / 4] = normal_id;
                    item[0x44 / 4] = pressed_id;
                }
                BarImage::RustLogo => {
                    // SharpLib 0x10fae8 wraps this bitmap and copies its pixels during add_item.
                    item[0x20 / 4] = core::ptr::from_ref(&normal) as u32;
                    item[0x40 / 4] = core::ptr::from_ref(&pressed) as u32;
                }
            }
            m.items[i] = unsafe { raw::add_item(self.handle, 0, item.as_ptr().cast()) };
            if m.items[i] == 0 {
                unsafe { self.close(m) };
                m.call = "ADD ITEM";
                m.result = 0;
                return;
            }
        }
        unsafe {
            raw::suspend_updates(self.handle, 0);
            self.control(0x0a, m);
        }
    }

    /// Release tracking also runs outside the bar screen.
    pub unsafe fn key(
        &mut self,
        key: u16,
        flags: u32,
        released: bool,
        enabled: bool,
        window: *mut c_void,
        m: &mut BarMeasurements,
    ) -> bool {
        let bit = match key {
            0x43 | 0x63 => 1,  // C
            0x44 | 0x64 => 2,  // D
            0x51 | 0x71 => 4,  // Q: 1
            0x57 | 0x77 => 8,  // W: 2
            0x45 | 0x65 => 16, // E: 4
            0x52 | 0x72 => 32, // R: 8
            0x53 | 0x73 => 64, // S: 0x0a
            key::UP => 128,
            key::DOWN => 256,
            key::LEFT => 512,
            key::RIGHT => 1024,
            key::PAGE_UP => 2048,
            key::PAGE_DOWN => 4096,
            0x54 | 0x74 => 8192, // T: random image
            _ => return false,
        };
        if released {
            self.keys &= !bit;
        } else {
            let fresh = self.keys & bit == 0;
            self.keys |= bit;
            if enabled && fresh {
                unsafe {
                    match bit {
                        1 => self.create(window, m),
                        2 => self.close(m),
                        4 => self.control(1, m),
                        8 => self.control(2, m),
                        16 => self.control(4, m),
                        32 => self.control(8, m),
                        64 => self.control(0x0a, m),
                        8192 => {
                            let now = reflex::runtime::micros();
                            m.shuffle(
                                flags & reflex::peg::KMOD_SHIFT != 0,
                                now as u32 ^ (now >> 32) as u32,
                            );
                            if !self.handle.is_null() {
                                self.close(m);
                                self.create(window, m);
                            }
                        }
                        128 | 256 | 512 | 1024 | 2048 | 4096 => {
                            if m.edit_button(key) && !self.handle.is_null() {
                                self.close(m);
                                self.create(window, m);
                            }
                        }
                        _ => unreachable!(),
                    }
                }
            }
        }
        enabled
    }
}
