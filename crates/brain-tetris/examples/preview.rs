use brain_tetris::Tetris;
use reflex::{
    Application, Event,
    display::{HEIGHT, WIDTH},
    peg::key,
};
use std::{fs, io::Write, path::Path};

fn press(game: &mut Tetris, code: u16) {
    for down in [true, false] {
        game.event(Event::Key {
            code,
            down,
            flags: 0,
        });
    }
}
fn save(game: &Tetris, directory: &Path, name: &str) -> std::io::Result<()> {
    let mut pixels = vec![0; WIDTH * HEIGHT];
    game.draw(&mut pixels);
    let mut file = fs::File::create(directory.join(name))?;
    write!(file, "P6\n{WIDTH} {HEIGHT}\n255\n")?;
    for p in pixels {
        file.write_all(&[
            (((p >> 11) & 31) * 255 / 31) as u8,
            (((p >> 5) & 63) * 255 / 63) as u8,
            ((p & 31) * 255 / 31) as u8,
        ])?;
    }
    Ok(())
}
fn main() -> std::io::Result<()> {
    let directory = std::env::args()
        .nth(1)
        .expect("Usage: preview OUTPUT_DIRECTORY");
    let directory = Path::new(&directory);
    fs::create_dir_all(directory)?;
    let mut game = Tetris::new(42);
    save(&game, directory, "start.ppm")?;
    press(&mut game, b'C' as u16);
    save(&game, directory, "hold.ppm")?;
    for keys in [vec![key::LEFT; 3], vec![key::RIGHT; 3], vec![b'X' as u16]] {
        for code in keys {
            press(&mut game, code);
        }
        press(&mut game, key::UP);
    }
    game.advance(700);
    save(&game, directory, "playing.ppm")?;
    press(&mut game, key::ENTER);
    save(&game, directory, "paused.ppm")?;
    let mut paused = vec![0; WIDTH * HEIGHT];
    game.draw(&mut paused);
    assert_eq!(game.advance(10_000), reflex::Action::None);
    let mut after = vec![0; WIDTH * HEIGHT];
    game.draw(&mut after);
    assert_eq!(paused, after, "paused board must not advance");
    press(&mut game, key::ENTER);
    for _ in 0..30 {
        press(&mut game, key::UP);
    }
    save(&game, directory, "game-over.ppm")?;
    press(&mut game, key::BACKSPACE);
    save(&game, directory, "retry.ppm")?;
    Ok(())
}
