#![no_std]

extern crate alloc;

use core::cell::RefCell;
use microts::{Input, Ui, spec::btn};
use reflex::{Action, Application, Event, display, peg::key};

#[allow(clippy::all)]
#[rustfmt::skip]
mod generated;

pub struct PocketJs {
    app: RefCell<generated::AppApp<generated::AppModel>>,
    buttons: u32,
}

impl Default for PocketJs {
    fn default() -> Self {
        let mut ui = Ui::new();
        assert!(ui.load_styles(include_bytes!("generated/styles.bin")));
        {
            let core = ui.core_mut();
            core.set_viewport(display::WIDTH as f32, display::HEIGHT as f32);
            assert!(core.load_font_atlas(include_bytes!("../assets/font-24.bin")));
            assert!(core.load_font_atlas(include_bytes!("../assets/font-54.bin")));
        }
        let mut app =
            generated::AppApp::new(ui, generated::AppProps {}, generated::AppModel::default());
        app.frame(&Input::buttons(0));
        Self {
            app: RefCell::new(app),
            buttons: 0,
        }
    }
}

impl Application for PocketJs {
    fn event(&mut self, event: Event) -> Action {
        let Event::Key { code, down, .. } = event else {
            return Action::None;
        };
        let button = match code {
            key::ESCAPE if down => return Action::Exit,
            key::ENTER => btn::CROSS,
            0x52 | 0x72 => btn::TRIANGLE,
            _ => return Action::None,
        };
        if down {
            self.buttons |= button;
        } else {
            self.buttons &= !button;
        }
        self.app.get_mut().frame(&Input::buttons(self.buttons));
        Action::Redraw
    }

    fn draw(&self, pixels: &mut [u16]) {
        let mut app = self.app.borrow_mut();
        let words = app.ui_mut().core_mut().draw().words.clone();
        pocket_raster::render_scaled_rgb565(app.ui().core(), &words, pixels, 1);
    }
}
