use std::{
    collections::HashSet,
    fs::{self, File},
    io::{self, Write},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail, ensure};
use clap::{Parser, Subcommand};
use serde::Serialize;

#[derive(Parser)]
#[command(version, about = "Inspect and extract Sharp Brain SDU update packages")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Validate the package and print its manifest as JSON.
    Info { input: PathBuf },
    /// Validate and extract into a new directory.
    Extract { input: PathBuf, output: PathBuf },
}

#[derive(Serialize)]
struct Manifest {
    model: String,
    size: usize,
    segments: Vec<Segment>,
    packages: Vec<Package>,
}

#[derive(Serialize)]
struct Segment {
    name: &'static str,
    offset: usize,
    size: usize,
}

#[derive(Serialize)]
struct Package {
    model: String,
    from_version: String,
    to_version: String,
    offset: usize,
    size: usize,
    entries: Vec<Entry>,
}

#[derive(Serialize)]
struct Entry {
    path: String,
    action: u32,
    header_offset: usize,
    data_offset: usize,
    size: usize,
    checksum: u32,
    timestamp: u32,
    secondary_size: u32,
    secondary_checksum: u32,
    attributes: u32,
}

fn bytes(data: &[u8], offset: usize, size: usize) -> Result<&[u8]> {
    let end = offset.checked_add(size).context("Offset overflow")?;
    data.get(offset..end)
        .with_context(|| format!("Truncated data at {offset:#x}, size {size:#x}"))
}

fn u32_at(data: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(bytes(data, offset, 4)?.try_into()?))
}

fn string_at(data: &[u8], offset: usize, size: usize) -> Result<String> {
    let field = bytes(data, offset, size)?;
    let end = field
        .iter()
        .position(|&b| b == 0)
        .context("Unterminated string")?;
    Ok(std::str::from_utf8(&field[..end])?.to_owned())
}

fn sum(data: &[u8]) -> u32 {
    data.iter()
        .fold(0_u32, |s, &b| s.wrapping_add(u32::from(b)))
}

fn check_header(header: &[u8], checksum_offset: usize) -> Result<()> {
    let checksum = u32_at(header, checksum_offset)?;
    let low = checksum as u16;
    let high = (checksum >> 16) as u16;
    ensure!(low == !high, "Invalid header checksum complement");
    ensure!(sum(header) as u16 == low, "Header checksum mismatch");
    Ok(())
}

fn relative_path(path: &str) -> Result<PathBuf> {
    let tail = path.strip_prefix("ROM:\\").context("Expected a ROM path")?;
    let mut result = PathBuf::from("ROM");
    for part in tail.split('\\') {
        ensure!(
            !part.is_empty()
                && part != "."
                && part != ".."
                && !part.contains(['/', ':'])
                && !part.chars().any(char::is_control),
            "Unsafe ROM path: {path}"
        );
        result.push(part);
    }
    Ok(result)
}

fn parse(data: &[u8]) -> Result<Manifest> {
    let header = bytes(data, 0, 0x8000)?;
    ensure!(u32_at(header, 0)? == 0x8000, "Unsupported SDU header size");
    ensure!(
        u32_at(header, 8)? as usize == data.len(),
        "SDU file size mismatch"
    );
    check_header(header, 4).context("SDU header")?;
    let mut manifest = Manifest {
        model: string_at(header, 0x1c, 0x60)?,
        size: data.len(),
        segments: Vec::new(),
        packages: Vec::new(),
    };
    let mut offset = header.len();
    for (name, field) in [
        ("updater.bin", 0x7c),
        ("boot.bin", 0xb4),
        ("system.bin", 0xd0),
        ("updates.bin", 0xec),
    ] {
        let size = u32_at(header, field)? as usize;
        let segment = bytes(data, offset, size)?;
        if field == 0x7c {
            let checksum = u32_at(header, 0x10)?;
            ensure!(
                checksum as u16 == !((checksum >> 16) as u16),
                "Invalid updater checksum complement"
            );
            ensure!(
                sum(segment) as u16 == checksum as u16,
                "Updater checksum mismatch"
            );
        }
        manifest.segments.push(Segment { name, offset, size });
        offset += size;
    }
    ensure!(offset == data.len(), "Unexpected data after SDU segments");
    offset = manifest.segments[3].offset;
    while offset < data.len() {
        let header = bytes(data, offset, 0x400)?;
        check_header(header, 0x3fc).with_context(|| format!("Package at {offset:#x}"))?;
        ensure!(
            u32_at(header, 0)? == 0x400 && u32_at(header, 4)? == 1,
            "Expected package header at {offset:#x}"
        );
        let size = 0x400 + u32_at(header, 0x20)? as usize;
        bytes(data, offset, size)?;
        let end = offset + size;
        let mut package = Package {
            model: string_at(header, 8, 8)?,
            from_version: string_at(header, 0x18, 8)?,
            to_version: string_at(header, 0x10, 8)?,
            offset,
            size,
            entries: Vec::new(),
        };
        ensure!(package.model == manifest.model, "Package model mismatch");
        let mut paths = HashSet::new();
        offset += 0x400;
        while offset < end {
            let header = bytes(&data[..end], offset, 0x400)?;
            check_header(header, 0x3fc).with_context(|| format!("Entry at {offset:#x}"))?;
            ensure!(
                u32_at(header, 0)? == 0x400 && u32_at(header, 4)? == 2,
                "Expected entry header at {offset:#x}"
            );
            let path = string_at(header, 8, 0x208)?;
            let safe_path = relative_path(&path)?;
            ensure!(paths.insert(safe_path), "Duplicate path in package: {path}");
            let entry = Entry {
                path,
                action: u32_at(header, 0x22c)?,
                header_offset: offset,
                data_offset: offset + 0x400,
                size: u32_at(header, 0x210)? as usize,
                checksum: u32_at(header, 0x214)?,
                timestamp: u32_at(header, 0x218)?,
                secondary_size: u32_at(header, 0x224)?,
                secondary_checksum: u32_at(header, 0x228)?,
                attributes: u32_at(header, 0x230)?,
            };
            let payload = bytes(&data[..end], entry.data_offset, entry.size)?;
            match entry.action {
                1 => ensure!(
                    sum(payload) == entry.checksum,
                    "Payload checksum mismatch: {}",
                    entry.path
                ),
                2 | 3 => ensure!(
                    entry.size == 0,
                    "Unexpected payload for action {}",
                    entry.action
                ),
                action => bail!("Unsupported action {action} at {offset:#x}"),
            }
            let padded_size = entry.size.div_ceil(0x200) * 0x200;
            bytes(&data[..end], entry.data_offset, padded_size)?;
            offset = entry.data_offset + padded_size;
            package.entries.push(entry);
        }
        manifest.packages.push(package);
    }
    Ok(manifest)
}

fn write_new(path: &Path, data: &[u8]) -> Result<()> {
    let parent = path.parent().context("Missing parent directory")?;
    fs::create_dir_all(parent)?;
    File::create_new(path)?.write_all(data)?;
    Ok(())
}

fn extract(data: &[u8], manifest: &Manifest, output: &Path) -> Result<()> {
    fs::create_dir(output)
        .with_context(|| format!("Output must be a new directory: {}", output.display()))?;
    write_new(
        &output.join("manifest.json"),
        &serde_json::to_vec_pretty(manifest)?,
    )?;
    write_new(&output.join("header.bin"), &data[..0x8000])?;
    for segment in &manifest.segments {
        write_new(
            &output.join(segment.name),
            bytes(data, segment.offset, segment.size)?,
        )?;
    }
    for (index, package) in manifest.packages.iter().enumerate() {
        let directory = output.join("packages").join(format!("{:03}", index + 1));
        for entry in &package.entries {
            if entry.action == 1 {
                write_new(
                    &directory.join(relative_path(&entry.path)?),
                    bytes(data, entry.data_offset, entry.size)?,
                )?;
            }
        }
    }
    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let input = match &cli.command {
        Command::Info { input } | Command::Extract { input, .. } => input,
    };
    let data = fs::read(input).with_context(|| format!("Reading {}", input.display()))?;
    let manifest = parse(&data)?;
    match cli.command {
        Command::Info { .. } => {
            let mut stdout = io::stdout().lock();
            serde_json::to_writer_pretty(&mut stdout, &manifest)?;
            writeln!(stdout)?;
        }
        Command::Extract { output, .. } => {
            extract(&data, &manifest, &output)?;
            let count = manifest
                .packages
                .iter()
                .flat_map(|p| &p.entries)
                .filter(|e| e.action == 1)
                .count();
            println!(
                "Extracted {count} files from {} packages to {}",
                manifest.packages.len(),
                output.display()
            );
        }
    }
    Ok(())
}
