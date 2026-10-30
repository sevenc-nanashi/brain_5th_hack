use brain_pocketjs::PocketJs;
use reflex::{
    Application, Event,
    display::{HEIGHT, WIDTH},
    peg::key,
};
use std::{fs, io::Write, path::Path};

fn main() -> std::io::Result<()> {
    let output = std::env::args()
        .nth(1)
        .expect("Usage: preview OUTPUT_DIRECTORY");
    let output = Path::new(&output);
    fs::create_dir_all(output)?;
    let mut app = PocketJs::default();
    let mut pixels = vec![0; WIDTH * HEIGHT];
    for (name, code) in [
        ("initial", None),
        ("increment", Some(key::ENTER)),
        ("reset", Some(b'R' as u16)),
    ] {
        if let Some(code) = code {
            for down in [true, false] {
                app.event(Event::Key {
                    code,
                    down,
                    flags: 0,
                });
            }
        }
        app.draw(&mut pixels);
        let mut file = fs::File::create(output.join(format!("{name}.ppm")))?;
        write!(file, "P6\n{WIDTH} {HEIGHT}\n255\n")?;
        for pixel in &pixels {
            file.write_all(&[
                (((pixel >> 11) & 31) * 255 / 31) as u8,
                (((pixel >> 5) & 63) * 255 / 63) as u8,
                ((pixel & 31) * 255 / 31) as u8,
            ])?;
        }
    }
    Ok(())
}
