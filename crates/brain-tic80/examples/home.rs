use brain_tic80::home::Home;
use reflex::display::{HEIGHT, WIDTH};
use std::{fs, io::Write, path::Path};

fn save(home: &Home, output: &Path) -> std::io::Result<()> {
    let mut pixels = vec![0; WIDTH * HEIGHT];
    home.draw(&mut pixels);
    let mut file = fs::File::create(output)?;
    write!(file, "P6\n800 480\n255\n")?;
    for row in pixels.as_chunks::<WIDTH>().0 {
        for &p in &row[..800] {
            file.write_all(&[
                ((p >> 11) * 255 / 31) as u8,
                (((p >> 5) & 63) * 255 / 63) as u8,
                ((p & 31) * 255 / 31) as u8,
            ])?;
        }
    }
    Ok(())
}
fn main() -> std::io::Result<()> {
    let output = std::env::args().nth(1).expect("preview OUTPUT_DIRECTORY");
    let output = Path::new(&output);
    fs::create_dir_all(output)?;
    let mut tic = Home::new("TIC-80", br"SD0:\APP\TIC80\cart.tic", b"tic");
    save(&tic, &output.join("tic-first.ppm"))?;
    // Illustrative history; the device reads actual launch records.
    tic.record(br"SD0:\GAMES\tetris.tic");
    tic.record(br"SD0:\APP\TIC80\cart.tic");
    tic.record(br"SD0:\GAMES\quest.tic");
    save(&tic, &output.join("tic-recent.ppm"))?;
    tic.set_active(Some(br"SD0:\GAMES\quest.tic"));
    save(&tic, &output.join("tic-paused.ppm"))?;
    tic.event(reflex::Event::Key {
        code: b'h' as u16,
        down: true,
        flags: 0,
    });
    save(&tic, &output.join("tic-keymap.ppm"))?;
    tic.event(reflex::Event::Key {
        code: b'h' as u16,
        down: false,
        flags: 0,
    });
    tic.event(reflex::Event::Key {
        code: reflex::peg::key::ESCAPE,
        down: true,
        flags: 0,
    });
    tic.event(reflex::Event::Key {
        code: reflex::peg::key::ESCAPE,
        down: false,
        flags: 0,
    });
    tic.set_active(None);

    tic.notice = Some("Cannot write save file");
    save(&tic, &output.join("tic-error.ppm"))?;
    Ok(())
}
