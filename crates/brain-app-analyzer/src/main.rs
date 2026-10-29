use std::{
    fs,
    io::{self, Write},
    path::PathBuf,
};

use anyhow::{Context, Result, ensure};
use clap::Parser;
use serde::Serialize;

// Match the actual handlers before interpreting their private data layout.
const ZERO_HANDLER: &[u8] = b"\x30\xb4\x00\x25\x13\xe0\x50\xf8\x04\x2b\xd3\x07\x44\xbf\xa9\xf1\x01\x04\xa2\x18\x09\x1f\x04\x29\x42\xf8\x04\x5b\xfa\xd2\x13\x46\x8c\x07\x44\xbf\x15\x80\x9b\x1c\xc9\x07\x48\xbf\x1d\x70\x50\xf8\x04\x1b\x00\x29\xe7\xd1\x30\xbc\x70\x47";
const DECOMPRESSOR: &[u8] = b"\x02\x46\xf0\xb4\x10\x68\x95\x68\x13\x18\x50\x68\x03\xeb\x50\x04\xc0\x07\x1c\xd5\x4d\x44\x1a\xe0\x13\xf8\x01\x7b\x05\xf8\x01\x7b\x49\x1e\xf9\xd1\x98\xb1\xc6\xf3\x81\x06\x13\xf8\x01\x1b\x03\x2e\x08\xbf\x13\xf8\x01\x6b\x01\xeb\x06\x21\x49\x42\x80\x1c\x29\x44\x05\xd0\x11\xf8\x01\x6b\x40\x1e\x05\xf8\x01\x6b\xf9\xd1\xa3\x42\x0e\xd0\x13\xf8\x01\x6b\x16\xf0\x03\x01\x04\xbf\x13\xf8\x01\x1b\xc9\x1c\x30\x09\x0f\x28\xdb\xd1\x13\xf8\x01\x0b\x0f\x30\xd7\xe7\x02\xf1\x0c\x00\xf0\xbc\x70\x47";

#[derive(Parser)]
#[command(
    version,
    about = "Analyze known PW-H1 AppMain.bin startup layouts; print JSON"
)]
struct Cli {
    input: PathBuf,
    /// Write the decompressed RAM initialization data to a new file.
    #[arg(long)]
    data_output: Option<PathBuf>,
}

#[derive(Serialize)]
struct Analysis {
    file_size: usize,
    entry_offset: usize,
    inferred_load_address: Option<u32>,
    context_pointer_address: u32,
    main_branch_target_offset: usize,
    thumb_initializer_offset: usize,
    initialization_table_offset: usize,
    initialization_table_end: usize,
    zero_handler_offset: usize,
    zero_address: u32,
    zero_size: u32,
    decompressor_offset: usize,
    compressed_offset: usize,
    compressed_size: usize,
    data_address: u32,
    decompressed_size: usize,
    gap_before_zero_region: usize,
    additional_handler_offsets: Vec<usize>,
}

fn bytes(data: &[u8], offset: usize, size: usize) -> Result<&[u8]> {
    let end = offset.checked_add(size).context("Offset overflow")?;
    data.get(offset..end)
        .with_context(|| format!("Truncated data at {offset:#x}"))
}

fn word(data: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(bytes(data, offset, 4)?.try_into()?))
}

fn relative(offset: usize, displacement: i64) -> Result<usize> {
    usize::try_from(offset as i64 + displacement).context("Negative file offset")
}

fn branch_target(offset: usize, instruction: u32, thumb: bool) -> Result<usize> {
    let displacement = ((instruction << 8) as i32 >> 6) as i64;
    relative(
        offset + 8,
        displacement
            + if thumb {
                ((instruction >> 23) & 2) as i64
            } else {
                0
            },
    )
}

fn movw(data: &[u8], offset: usize, register: u16) -> Result<usize> {
    let raw = bytes(data, offset, 4)?;
    let first = u16::from_le_bytes(raw[..2].try_into()?);
    let second = u16::from_le_bytes(raw[2..].try_into()?);
    ensure!(
        first & 0xfbf0 == 0xf240 && second & 0x8f00 == register << 8,
        "Unsupported MOVW at {offset:#x}"
    );
    Ok(
        (((first & 15) << 12) | ((first & 0x400) << 1) | ((second & 0x7000) >> 4) | (second & 255))
            as usize,
    )
}

fn handler(data: &[u8], offset: usize) -> Result<usize> {
    let address = relative(offset, word(data, offset)? as i32 as i64)?;
    ensure!(address & 1 == 1, "Expected a Thumb initialization handler");
    let address = address & !1;
    bytes(data, address, 2)?;
    Ok(address)
}

fn next(input: &mut std::slice::Iter<'_, u8>) -> Result<usize> {
    Ok(usize::from(
        *input.next().context("Truncated compressed token")?,
    ))
}

fn decompress(data: &[u8], output_limit: usize) -> Result<Vec<u8>> {
    let mut input = data.iter();
    let mut output = Vec::new();
    while !input.as_slice().is_empty() {
        let token = next(&mut input)?;
        let literals = if token & 3 == 0 {
            next(&mut input)? + 2
        } else {
            (token & 3) - 1
        };
        let length = if token >> 4 == 15 {
            next(&mut input)? + 15
        } else {
            token >> 4
        };
        ensure!(
            literals <= output_limit - output.len(),
            "Literal data overlaps zero region"
        );
        for _ in 0..literals {
            output.push(next(&mut input)? as u8);
        }
        if length != 0 {
            let low = next(&mut input)?;
            let high = if (token >> 2) & 3 == 3 {
                next(&mut input)?
            } else {
                (token >> 2) & 3
            };
            let distance = low | (high << 8);
            ensure!(
                distance > 0 && distance <= output.len(),
                "Invalid back-reference distance {distance}"
            );
            ensure!(
                length + 2 <= output_limit - output.len(),
                "Back-reference overlaps zero region"
            );
            for _ in 0..length + 2 {
                output.push(output[output.len() - distance]);
            }
        }
    }
    Ok(output)
}

fn analyze(data: &[u8]) -> Result<(Analysis, Vec<u8>)> {
    ensure!(
        word(data, 0)? == 0xe92d4010,
        "Unsupported AppMain entry point"
    );
    let variant = word(data, 4)?;
    ensure!(
        matches!(variant, 0xe1a04000 | 0xe1b04000),
        "Unsupported AppMain startup"
    );
    let wrapper: &[u8] = if variant == 0xe1a04000 {
        b"\x0c\x00\x9f\xe5\x00\x40\x80\xe5\x08\x00\x94\xe5\x10\x40\xbd\xe8"
    } else {
        ensure!(
            word(data, 0x20)? == 0xe8bd8010,
            "Unsupported startup return"
        );
        b"\x10\x00\x9f\xe5\x00\x40\x80\xe5\x00\x00\x90\xe5\x08\x00\x90\xe5"
    };
    ensure!(
        bytes(data, 0x0c, 16)? == wrapper,
        "Unsupported context pointer access"
    );
    let blx = word(data, 8)?;
    ensure!(
        blx & 0xfe000000 == 0xfa000000,
        "Expected BLX to Thumb initializer"
    );
    let init = branch_target(8, blx, true)?;
    ensure!(
        bytes(data, init, 2)? == b"\x10\xb5",
        "Unsupported initializer"
    );
    ensure!(
        bytes(data, init + 6, 10)? == b"\xc0\xf2\x00\x01\x79\x44\xa1\xf1\x0c\x01",
        "Unsupported table start calculation"
    );
    ensure!(bytes(data, init + 20, 28)? == b"\xc0\xf2\x00\x04\x7c\x44\xa4\xf1\x0c\x04\x04\xe0\x0a\x68\x08\x1d\x11\x44\x88\x47\x01\x46\xa1\x42\xf8\xd1\x10\xbd", "Unsupported table dispatcher");
    let start = init + movw(data, init + 2, 1)? + 2;
    let end = init + movw(data, init + 16, 4)? + 16;
    ensure!(
        end >= start && matches!(end - start, 32 | 36),
        "Unsupported initialization table length"
    );
    bytes(data, start, end - start)?;
    let zero_handler = handler(data, start)?;
    ensure!(
        bytes(data, zero_handler, ZERO_HANDLER.len())? == ZERO_HANDLER,
        "Unknown zero initialization handler"
    );
    let zero_size = word(data, start + 4)?;
    let zero_address = word(data, start + 8)?;
    ensure!(
        zero_size > 0 && word(data, start + 12)? == 0,
        "Unsupported zero initialization list"
    );
    ensure!(
        zero_address & 1 == 0,
        "r9-relative zero address is not supported"
    );
    zero_address
        .checked_add(zero_size)
        .context("Zero region address overflow")?;
    let decompressor = handler(data, start + 16)?;
    ensure!(
        bytes(data, decompressor, DECOMPRESSOR.len())? == DECOMPRESSOR,
        "Unknown decompression handler"
    );
    let compressed_offset = relative(start + 20, word(data, start + 20)? as i32 as i64)?;
    let packed_length = word(data, start + 24)?;
    ensure!(
        packed_length & 1 == 0,
        "r9-relative data address is not supported"
    );
    let compressed_size = (packed_length >> 1) as usize;
    let data_address = word(data, start + 28)?;
    let limit = zero_address
        .checked_sub(data_address)
        .context("Invalid RAM region order")? as usize;
    let decoded = decompress(bytes(data, compressed_offset, compressed_size)?, limit)?;
    let main_branch = word(data, 0x1c)?;
    ensure!(
        main_branch & 0xfe000000 == 0xea000000,
        "Unsupported main branch"
    );
    let main_target = branch_target(0x1c, main_branch, false)?;
    bytes(data, main_target, 4)?;
    let load_address = if bytes(data, init + 0x30, 4)? == b"\xdf\xf8\x00\xf0" {
        Some(
            word(data, init + 0x34)?
                .checked_sub((init + 0x3c) as u32)
                .context("Invalid ARM veneer address")?,
        )
    } else {
        None
    };
    let extra = if end - start == 36 {
        vec![handler(data, start + 32)?]
    } else {
        Vec::new()
    };
    Ok((
        Analysis {
            file_size: data.len(),
            entry_offset: 0,
            inferred_load_address: load_address,
            context_pointer_address: word(data, if variant == 0xe1a04000 { 0x20 } else { 0x24 })?,
            main_branch_target_offset: main_target,
            thumb_initializer_offset: init,
            initialization_table_offset: start,
            initialization_table_end: end,
            zero_handler_offset: zero_handler,
            zero_address,
            zero_size,
            decompressor_offset: decompressor,
            compressed_offset,
            compressed_size,
            data_address,
            decompressed_size: decoded.len(),
            gap_before_zero_region: limit - decoded.len(),
            additional_handler_offsets: extra,
        },
        decoded,
    ))
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let data = fs::read(&cli.input).with_context(|| format!("Reading {}", cli.input.display()))?;
    let (analysis, decoded) = analyze(&data)?;
    if let Some(output) = cli.data_output {
        fs::File::create_new(&output)
            .with_context(|| format!("Output must be a new file: {}", output.display()))?
            .write_all(&decoded)?;
    }
    let mut stdout = io::stdout().lock();
    serde_json::to_writer_pretty(&mut stdout, &analysis)?;
    writeln!(stdout)?;
    Ok(())
}
