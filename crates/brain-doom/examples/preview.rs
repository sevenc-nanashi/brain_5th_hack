use brain_doom::{engine, paint};
use std::{
    fs::File,
    io::{BufWriter, Write},
};

fn main() -> std::io::Result<()> {
    let path = std::env::args_os()
        .nth(1)
        .expect("Usage: preview OUTPUT.ppm");
    let mut output = BufWriter::new(File::create_new(path)?);
    // One engine instance, called only from this thread.
    unsafe {
        engine::set_time(0);
        assert_eq!(
            engine::init(),
            0,
            "{}",
            String::from_utf8_lossy(engine::last_log())
        );
        for tick in 1..=210u64 {
            engine::set_time(tick * 1_000_000 / 35);
            let press = match tick {
                5 => Some(27),
                20 | 35 | 50 => Some(13),
                100 => Some(0xad),
                140 => Some(b'z' as i32),
                _ => None,
            };
            let release = match tick {
                6 => Some(27),
                21 | 36 | 51 => Some(13),
                130 => Some(0xad),
                180 => Some(b'z' as i32),
                _ => None,
            };
            if let Some(key) = press {
                engine::key(key, true);
            }
            if let Some(key) = release {
                engine::key(key, false);
            }
            assert_eq!(
                engine::step(),
                0,
                "{}",
                String::from_utf8_lossy(engine::last_log())
            );
        }
        let mut pixels = vec![0u16; reflex::display::WIDTH * reflex::display::HEIGHT];
        paint(engine::frame(), |x, y, color| {
            pixels[y * reflex::display::WIDTH + x] = color
        });
        assert!(
            pixels.iter().any(|&p| p != 0),
            "Engine produced a blank frame"
        );
        write!(
            output,
            "P6\n{} {}\n255\n",
            reflex::display::WIDTH,
            reflex::display::HEIGHT
        )?;
        for color in pixels {
            output.write_all(&[
                (((color >> 11) & 31) * 255 / 31) as u8,
                (((color >> 5) & 63) * 255 / 63) as u8,
                ((color & 31) * 255 / 31) as u8,
            ])?;
        }
        // Exercise the engine's real menu exit and the C nonlocal-return boundary.
        engine::key(0xc4, true); // F10: quit confirmation
        engine::set_time(211 * 1_000_000 / 35);
        assert_eq!(engine::step(), 0);
        engine::key(0xc4, false);
        engine::key(b'y' as i32, true);
        engine::set_time(212 * 1_000_000 / 35);
        assert_eq!(
            engine::step(),
            1,
            "Doom did not return through its exit callback"
        );
        engine::shutdown();
    }
    output.flush()
}
