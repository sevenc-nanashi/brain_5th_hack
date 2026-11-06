use brain_inspector::{BAR_BUTTONS, BarImage, BarMeasurements, bar_image};
use core::ffi::c_void;
use reflex::operation_bar::{Config, image, raw};
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
        let mut config = Config::pcconnect(m.slots as u8);
        if m.light {
            config.background_color = 0x00dc_dcdc;
            config.boundary_color = 0;
        }
        self.handle = unsafe { raw::create(window, 0, &config, 0) };
        m.handle = self.handle as u32;
        m.result = i32::from(!self.handle.is_null());
        if self.handle.is_null() {
            return;
        }
        unsafe { raw::suspend_updates(self.handle, 1) };
        let normal = bar_image::bitmap(false);
        let pressed = bar_image::bitmap(true);
        for (i, button) in m.buttons.into_iter().take(m.item_count()).enumerate() {
            let (_, image) = BAR_BUTTONS[button];
            // PCConnect's image descriptor, plus DicMain's two-state expand button.
            let mut item = [0u32; 0x220 / 4];
            item[0] = 0x21;
            item[1] = u32::from(MESSAGE) | ((i as u32 + 1) << 16);
            item[4] = 52;
            item[5] = 52;
            if m.two_columns && i == 0 {
                // DicMain 0x370a64: bit 0x4000 expands groups; 0x100 suppresses notification.
                item[0] = 0x4121;
                item[1] = 0;
                let (open, open_pressed, close, close_pressed) = if m.light {
                    (
                        image::LIGHT_OPEN_ARROW,
                        image::LIGHT_OPEN_ARROW_PRESSED,
                        image::LIGHT_CLOSE_ARROW,
                        image::LIGHT_CLOSE_ARROW_PRESSED,
                    )
                } else {
                    (
                        image::OPEN_ARROW,
                        image::OPEN_ARROW_PRESSED,
                        image::CLOSE_ARROW,
                        image::CLOSE_ARROW_PRESSED,
                    )
                };
                item[0x24 / 4] = open;
                item[0x44 / 4] = open_pressed;
                item[0xa4 / 4] = close;
                item[0xc4 / 4] = close_pressed;
            } else {
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
            0x54 | 0x74 => 8192,  // T: random image
            0x4c | 0x6c => 16384, // L: light/dark theme
            0x4f | 0x6f => 32768, // O: one/two columns
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
                        16384 => {
                            m.toggle_light();
                            if !self.handle.is_null() {
                                self.close(m);
                                self.create(window, m);
                            }
                        }
                        32768 => {
                            m.toggle_columns();
                            if !self.handle.is_null() {
                                self.close(m);
                                self.create(window, m);
                            }
                        }
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
