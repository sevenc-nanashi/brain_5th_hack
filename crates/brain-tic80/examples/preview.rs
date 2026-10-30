use brain_tic80::{HEIGHT, Input, WIDTH, debug, engine, paint};
use std::{ffi::CStr, fs, io::Write, path::Path};

fn check(result: i32) {
    assert_eq!(
        result,
        0,
        "{}",
        unsafe { CStr::from_ptr(engine::brain_tic_error()) }.to_string_lossy()
    );
}
fn cart(code: &str) -> Vec<u8> {
    assert!(code.len() < 65536);
    let mut bytes = vec![17, 0, 0, 0, 5, code.len() as u8, (code.len() >> 8) as u8, 0];
    bytes.extend_from_slice(code.as_bytes());
    bytes
}
fn save(pixels: &[u16], path: &Path) -> std::io::Result<()> {
    let mut image = fs::File::create(path)?;
    write!(
        image,
        "P6\n{} {}\n255\n",
        reflex::display::WIDTH,
        reflex::display::HEIGHT
    )?;
    for &p in pixels {
        image.write_all(&[
            ((p >> 11) * 255 / 31) as u8,
            (((p >> 5) & 63) * 255 / 63) as u8,
            ((p & 31) * 255 / 31) as u8,
        ])?;
    }
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let cartridge = args.next().expect("preview CARTRIDGE OUTPUT_DIRECTORY");
    let output = args.next().expect("preview CARTRIDGE OUTPUT_DIRECTORY");
    let output = Path::new(&output);
    fs::create_dir_all(output)?;
    let data = fs::read(cartridge)?;
    let mut pcm = vec![0i16; 1470];
    let mut audio = Vec::new();
    let mut pixels = vec![0; reflex::display::WIDTH * reflex::display::HEIGHT];
    unsafe {
        check(engine::brain_tic_init(data.as_ptr(), data.len()));
        let mut input = Input::default();
        for frame in 0..120 {
            input.key(b'Z' as u16, frame % 30 == 0);
            input.key(reflex::peg::key::RIGHT, frame < 20);
            engine::brain_tic_input(input.buttons(), input.keyboard(), 0, 0, false);
            check(engine::brain_tic_fill(pcm.as_mut_ptr(), pcm.len()));
            for sample in &pcm {
                audio.extend_from_slice(&sample.to_le_bytes());
            }
        }
        let screen = core::slice::from_raw_parts(engine::brain_tic_screen(), WIDTH * HEIGHT);
        paint(screen, &mut pixels);
        assert!(pixels.iter().any(|&p| p != 0), "blank cartridge output");
        engine::brain_tic_close();
    }
    save(&pixels, &output.join("tic80.ppm"))?;
    let mut performance = debug::Performance::default();
    performance.reset(0, 0);
    for frame in (2..=60).step_by(2) {
        performance.rendered(frame);
        performance.rendered(frame); // UI-only redraws must not increase the game FPS.
    }
    assert!(!performance.sample(999_999, 60));
    assert!(performance.sample(1_000_000, 60));
    assert_eq!((performance.fps, performance.emulation_fps), (30, 60));
    debug::hud(&mut pixels, &performance);
    save(&pixels, &output.join("fps.ppm"))?;
    debug::menu(&mut pixels, &performance, &debug::Audio::default(), true);
    save(&pixels, &output.join("debug.ppm"))?;
    assert!(performance.sample(2_000_000, 60));
    assert_eq!((performance.fps, performance.emulation_fps), (0, 0));
    performance.reset(5_000_000, 0);
    performance.rendered(1);
    assert!(performance.sample(6_000_000, 1));
    assert_eq!((performance.fps, performance.emulation_fps), (1, 1));
    let mut wav = fs::File::create(output.join("tic80.wav"))?;
    wav.write_all(b"RIFF")?;
    wav.write_all(&(36 + audio.len() as u32).to_le_bytes())?;
    wav.write_all(b"WAVEfmt \x10\0\0\0\x01\0\x02\0")?;
    wav.write_all(&44100u32.to_le_bytes())?;
    wav.write_all(&176400u32.to_le_bytes())?;
    wav.write_all(b"\x04\0\x10\0data")?;
    wav.write_all(&(audio.len() as u32).to_le_bytes())?;
    wav.write_all(&audio)?;

    // Exercise the C boundary with malformed input, script errors and repeated teardown.
    unsafe {
        for invalid in [&[][..], &[5, 255, 255, 0][..], &[19, 0, 0, 0][..]] {
            assert_eq!(engine::brain_tic_init(invalid.as_ptr(), invalid.len()), -1);
            engine::brain_tic_close();
        }
        for (code, expected) in [
            ("function TIC() error('expected failure') end", -1),
            ("function TIC() while true do end end", -1),
            ("function TIC() exit() end", 1),
            ("function TIC() cls(4) end", 0),
        ] {
            let data = cart(code);
            check(engine::brain_tic_init(data.as_ptr(), data.len()));
            assert_eq!(
                engine::brain_tic_fill(pcm.as_mut_ptr(), pcm.len()),
                expected
            );
            engine::brain_tic_close();
        }
    }
    println!(
        "{} (120 frames, PCM output, malformed carts, Lua errors, exit and reload checked)",
        output.display()
    );
    Ok(())
}
