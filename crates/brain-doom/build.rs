use std::{env, fs, path::PathBuf, process::Command};

fn run(command: &mut Command) {
    assert!(
        command.status().expect("start C toolchain").success(),
        "C toolchain failed: {command:?}"
    );
}

fn main() {
    for name in ["DOOM_INPUTS", "DOOM_CC", "DOOM_AR", "CARGO_FEATURE_ENGINE"] {
        println!("cargo:rerun-if-env-changed={name}");
    }
    println!("cargo:rerun-if-changed=engine");
    if env::var_os("CARGO_FEATURE_ENGINE").is_none() {
        return;
    }
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let inputs = match env::var_os("DOOM_INPUTS") {
        Some(path) => PathBuf::from(path),
        None => root.join("../../target/doom-inputs"),
    }
    .canonicalize()
    .expect("Run ruby scripts/fetch-doom.rb first (or set DOOM_INPUTS)");
    let wad = inputs.join("DOOM1.WAD");
    let bytes = fs::read(&wad).expect("read DOOM1.WAD");
    assert!(
        bytes.len() >= 12 && &bytes[..4] == b"IWAD",
        "DOOM1.WAD must be an IWAD"
    );
    let word = |offset| u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize;
    let count = word(4);
    let directory = word(8);
    assert!(
        count > 0
            && directory
                .checked_add(count.checked_mul(16).unwrap())
                .unwrap()
                <= bytes.len(),
        "Invalid WAD directory"
    );
    for i in 0..count {
        let record = directory + i * 16;
        assert!(
            word(record).checked_add(word(record + 4)).unwrap() <= bytes.len(),
            "Invalid WAD lump"
        );
    }
    assert!(
        (0..count)
            .any(|i| &bytes[directory + i * 16 + 8..directory + i * 16 + 16] == b"E1M1\0\0\0\0"),
        "Expected Doom episode 1 IWAD"
    );
    println!("cargo:rustc-env=DOOM_WAD={}", wad.display());
    println!("cargo:rerun-if-changed={}", wad.display());
    println!(
        "cargo:rerun-if-changed={}",
        inputs.join("PureDOOM.h").display()
    );
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let bank = (0..count)
        .map(|i| directory + i * 16)
        .find(|&r| &bytes[r + 8..r + 16] == b"GENMIDI\0")
        .expect("WAD lacks GENMIDI instruments");
    let bank = &bytes[word(bank)..word(bank) + word(bank + 4)];
    assert!(
        bank.len() >= 8 + 175 * 36 && &bank[..8] == b"#OPL_II#",
        "Invalid GENMIDI"
    );
    fs::write(out.join("genmidi.bin"), &bank[..8 + 175 * 36]).unwrap();
    println!(
        "cargo:rustc-env=DOOM_GENMIDI={}",
        out.join("genmidi.bin").display()
    );
    let frequencies: Vec<u16> = (0..4096)
        .map(|pitch| {
            let hz = 440.0 * 2.0f64.powf((pitch as f64 / 32.0 - 69.0) / 12.0);
            let mut block = 0u16;
            let mut number = hz * 1048576.0 / 49716.0;
            while number > 1023.0 && block < 7 {
                number /= 2.0;
                block += 1;
            }
            (number.round().min(1023.0) as u16) | (block << 10)
        })
        .collect();
    fs::write(out.join("opl-pitch.rs"), format!("{frequencies:?}")).unwrap();
    let levels: Vec<u8> = (0..128)
        .map(|v| {
            if v == 0 {
                63
            } else {
                (-20.0 * (v as f64 / 127.0).log10() / 0.75)
                    .round()
                    .min(63.0) as u8
            }
        })
        .collect();
    fs::write(out.join("opl-volume.rs"), format!("{levels:?}")).unwrap();
    let target = env::var("TARGET").unwrap();
    let arm = target == "armv7a-none-eabi";
    let cc = env::var("DOOM_CC").unwrap_or_else(|_| "clang-20".into());
    let ar = env::var("DOOM_AR").unwrap_or_else(|_| "llvm-ar-20".into());
    let mut compile = Command::new(&cc);
    compile.args([
        "-std=gnu11",
        "-O2",
        "-fno-strict-aliasing",
        "-fwrapv",
        // Doom's ticcmd_t stores negative movement in plain char fields.
        "-fsigned-char",
        "-ffunction-sections",
        "-fdata-sections",
        "-Wno-parentheses",
        "-Wno-pointer-sign",
        "-Wno-shift-negative-value",
    ]);
    if arm {
        compile.args([
            "--target=armv7a-none-eabi",
            "-mcpu=cortex-a7",
            "-mfloat-abi=soft",
            "-marm",
            "-ffreestanding",
            "-fno-builtin",
            "-fno-stack-protector",
            "-DBRAIN_ARM",
        ]);
    }
    for (source, name) in [
        (inputs.join("opal.c"), "opal"),
        (root.join("engine/opl.c"), "opl"),
    ] {
        println!("cargo:rerun-if-changed={}", source.display());
        let mut command = Command::new(&cc);
        command.args(compile.get_args());
        // OPL synthesis dominates audio refill time on the device.
        command.arg("-O3");
        run(command
            .arg("-I")
            .arg(&inputs)
            .arg("-c")
            .arg(source)
            .arg("-o")
            .arg(out.join(format!("{name}.o"))));
    }
    println!("cargo:rerun-if-changed={}", inputs.join("opal.h").display());
    run(compile
        .arg("-I")
        .arg(&inputs)
        .arg("-c")
        .arg(root.join("engine/bridge.c"))
        .arg("-o")
        .arg(out.join("doom.o")));
    let mut archive = Command::new(ar);
    if out.join("libdoom.a").exists() {
        fs::remove_file(out.join("libdoom.a")).unwrap();
    }
    archive
        .arg("crs")
        .arg(out.join("libdoom.a"))
        .arg(out.join("doom.o"))
        .arg(out.join("opal.o"))
        .arg(out.join("opl.o"));
    if arm {
        run(Command::new(cc)
            .args([
                "--target=armv7a-none-eabi",
                "-mcpu=cortex-a7",
                "-mfloat-abi=soft",
                "-c",
            ])
            .arg(root.join("engine/jump.S"))
            .arg("-o")
            .arg(out.join("jump.o")));
        archive.arg(out.join("jump.o"));
    }
    run(&mut archive);
    println!("cargo:rustc-link-search=native={}", out.display());
    println!("cargo:rustc-link-lib=static=doom");
}
