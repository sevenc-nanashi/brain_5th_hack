use brain_explorer::{Entry, Explorer, Filesystem, MAX_ENTRIES};
use reflex::{
    Application, Event,
    display::{HEIGHT, WIDTH},
    peg::key,
};
use std::{
    fs,
    io::{Read, Seek, SeekFrom, Write},
    os::unix::ffi::{OsStrExt, OsStringExt},
    path::{Path, PathBuf},
};

struct Host(PathBuf);
impl Host {
    fn path(&self, path: &[u8]) -> Result<PathBuf, &'static str> {
        let relative = path
            .strip_prefix(b"SD0:\\")
            .ok_or("Preview uses SD0 only")?;
        let mut result = self.0.clone();
        for component in relative.split(|byte| *byte == b'\\') {
            result.push(std::ffi::OsString::from_vec(component.to_vec()));
        }
        Ok(result)
    }
}
impl Filesystem for Host {
    fn read_dir(&mut self, path: &[u8]) -> Result<Vec<Entry>, &'static str> {
        let mut entries = Vec::new();
        for entry in fs::read_dir(self.path(path)?).map_err(|_| "Cannot open folder")? {
            let entry = entry.map_err(|_| "Cannot read folder entry")?;
            let metadata = entry.metadata().map_err(|_| "Cannot read metadata")?;
            if entries.len() == MAX_ENTRIES {
                return Err("Folder exceeds 4096 entries");
            }
            entries.push(Entry {
                name: entry.file_name().as_bytes().to_vec(),
                directory: metadata.is_dir(),
                size: u32::try_from(metadata.len()).map_err(|_| "File size exceeds u32::MAX")?,
            });
        }
        Ok(entries)
    }
    fn read(&mut self, path: &[u8], offset: u32, buffer: &mut [u8]) -> Result<usize, &'static str> {
        let mut file = fs::File::open(self.path(path)?).map_err(|_| "Cannot open file")?;
        file.seek(SeekFrom::Start(u64::from(offset)))
            .map_err(|_| "Cannot seek file")?;
        std::io::copy(&mut file.take(buffer.len() as u64), &mut &mut buffer[..])
            .map(|count| count as usize)
            .map_err(|_| "Cannot read file")
    }
}

fn key(app: &mut impl Application, code: u16) {
    for down in [true, false] {
        app.event(Event::Key {
            code,
            down,
            flags: 0,
        });
    }
}
fn save(app: &impl Application, output: &Path, name: &str) -> std::io::Result<()> {
    let mut pixels = vec![0; WIDTH * HEIGHT];
    app.draw(&mut pixels);
    let mut file = fs::File::create(output.join(name))?;
    write!(file, "P6\n{WIDTH} {HEIGHT}\n255\n")?;
    for pixel in pixels {
        file.write_all(&[
            (((pixel >> 11) & 31) * 255 / 31) as u8,
            (((pixel >> 5) & 63) * 255 / 63) as u8,
            ((pixel & 31) * 255 / 31) as u8,
        ])?;
    }
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let root = PathBuf::from(
        args.next()
            .expect("Usage: preview INPUT_DIRECTORY OUTPUT_DIRECTORY"),
    );
    let output = PathBuf::from(args.next().expect("Missing output directory"));
    let mut host = Host(root);
    let entries = host.read_dir(b"SD0:\\")?;
    let directories = entries.iter().filter(|entry| entry.directory).count();
    fs::create_dir_all(&output)?;
    let mut app = Explorer::new(host);
    save(&app, &output, "storage.ppm")?;
    key(&mut app, key::ENTER);
    save(&app, &output, "directory.ppm")?;
    if entries.iter().any(|entry| !entry.directory) {
        for _ in 0..directories {
            key(&mut app, key::DOWN);
        }
        key(&mut app, key::ENTER);
        save(&app, &output, "text.ppm")?;
        key(&mut app, key::PAGE_DOWN);
        save(&app, &output, "text-next.ppm")?;
        key(&mut app, key::PAGE_UP);
        key(&mut app, b'T' as u16);
        save(&app, &output, "hex.ppm")?;
    }
    Ok(())
}
