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
    run(compile
        .arg("-I")
        .arg(&inputs)
        .arg("-c")
        .arg(root.join("engine/bridge.c"))
        .arg("-o")
        .arg(out.join("doom.o")));
    let mut archive = Command::new(ar);
    archive
        .arg("crs")
        .arg(out.join("libdoom.a"))
        .arg(out.join("doom.o"));
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
