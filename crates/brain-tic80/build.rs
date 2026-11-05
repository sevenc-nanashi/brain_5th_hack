use std::{env, path::PathBuf, process::Command};

fn main() {
    println!("cargo:rerun-if-env-changed=TIC80_INPUTS");
    println!("cargo:rerun-if-env-changed=LUAJIT_HOST_CC");
    println!("cargo:rerun-if-changed=engine");
    println!("cargo:rerun-if-changed=assets/demo.lua");
    println!("cargo:rerun-if-changed=../../scripts/build-tic80.rb");
    println!("cargo:rerun-if-changed=../../scripts/build-tic80-luajit.rb");
    if env::var_os("CARGO_FEATURE_ENGINE").is_none() {
        return;
    }
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("../..");
    let inputs = PathBuf::from(
        env::var_os("TIC80_INPUTS").expect("Set TIC80_INPUTS; run make tic80-fetch first"),
    );
    let arm = env::var("TARGET").unwrap() == "armv7a-none-eabi";
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    assert!(
        Command::new("ruby")
            .arg(root.join("scripts/build-tic80.rb"))
            .arg(&inputs)
            .arg(if arm { "arm" } else { "host" })
            .arg(&out)
            .status()
            .expect("build TIC-80")
            .success()
    );
    println!("cargo:rustc-link-search=native={}", out.display());
    println!("cargo:rustc-link-lib=static=brain-tic80");
    println!("cargo:rustc-link-lib=static=brain-luajit");
    if arm {
        println!(
            "cargo:rustc-link-search=native={}/newlib/usr/lib/arm-none-eabi/newlib/thumb/v7-a+fp/softfp",
            inputs.display()
        );
        for lib in ["c", "m", "nosys"] {
            println!("cargo:rustc-link-lib=static={lib}");
        }
    } else {
        println!("cargo:rustc-link-lib=m");
    }
}
