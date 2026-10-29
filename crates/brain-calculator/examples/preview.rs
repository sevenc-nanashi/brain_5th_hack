use brain_calculator::Calculator;
use reflex::{
    Application, Event, TouchPhase,
    display::{HEIGHT, WIDTH},
    panic_screen::{Report, render},
};
use std::{fmt::Write, fs, io::Write as _, path::Path};

fn key(app: &mut Calculator, code: u16) {
    app.event(Event::Key {
        code,
        down: true,
        flags: 0,
    });
    app.event(Event::Key {
        code,
        down: false,
        flags: 0,
    });
}
fn input(app: &mut Calculator, value: &str) {
    for byte in value.bytes() {
        key(app, u16::from(byte));
    }
}
fn tap(app: &mut Calculator, x: u16, y: u16) {
    for phase in [TouchPhase::Down, TouchPhase::Up] {
        app.event(Event::Touch { x, y, phase });
    }
}
fn save(directory: &Path, name: &str, pixels: &[u16]) -> std::io::Result<()> {
    let mut file = fs::File::create(directory.join(name))?;
    write!(file, "P6\n{WIDTH} {HEIGHT}\n255\n")?;
    for pixel in pixels {
        let rgb = [
            (((pixel >> 11) & 31) * 255 / 31) as u8,
            (((pixel >> 5) & 63) * 255 / 63) as u8,
            ((pixel & 31) * 255 / 31) as u8,
        ];
        file.write_all(&rgb)?;
    }
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let directory = std::env::args()
        .nth(1)
        .expect("Usage: preview OUTPUT_DIRECTORY");
    let directory = Path::new(&directory);
    fs::create_dir_all(directory)?;
    let mut pixels = vec![0; WIDTH * HEIGHT];
    let mut app = Calculator::new();
    input(&mut app, "sin(30)+2pi=");
    app.draw(&mut pixels);
    save(directory, "calculator.ppm", &pixels)?;
    key(&mut app, reflex::peg::key::LEFT);
    app.draw(&mut pixels);
    save(directory, "equals-focus.ppm", &pixels)?;
    for expression in [
        "sqrt(2)=",
        "2^3^2=",
        "log(100)=",
        "asin(1)=",
        "6/2(1+2)=",
        "exp(1)=",
    ] {
        tap(&mut app, 730, 250);
        input(&mut app, expression);
    }
    key(&mut app, reflex::peg::key::HISTORY);
    app.draw(&mut pixels);
    save(directory, "history.ppm", &pixels)?;
    tap(&mut app, 550, 30);
    app.draw(&mut pixels);
    save(directory, "history-next.ppm", &pixels)?;
    key(&mut app, reflex::peg::key::ESCAPE);
    // AC, via the same touch hit testing used on device.
    tap(&mut app, 730, 250);
    input(&mut app, "sqrt(-1)=");
    app.draw(&mut pixels);
    save(directory, "error.ppm", &pixels)?;
    let mut report = Report::new();
    write!(
        report,
        "brain-calculator/src/main.rs:42:5\npreview panic: sample diagnostic"
    )?;
    render(&report, |x, y, c| pixels[y * WIDTH + x] = c);
    save(directory, "panic.ppm", &pixels)?;
    Ok(())
}
