use std::{env, path::PathBuf, process::Command};

fn main() {
    println!("cargo:rerun-if-env-changed=NETSURF_INPUTS");
    println!("cargo:rerun-if-changed=engine");
    println!("cargo:rerun-if-changed=../../scripts/build-netsurf.rb");
    println!("cargo:rerun-if-changed=../../scripts/netsurf-resources.rb");
    if env::var_os("CARGO_FEATURE_DEVICE").is_none() {
        return;
    }
    assert_eq!(env::var("TARGET").unwrap(), "armv7a-none-eabi");
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("../..");
    let inputs = PathBuf::from(
        env::var_os("NETSURF_INPUTS")
            .expect("Set NETSURF_INPUTS; run scripts/fetch-netsurf.rb first"),
    );
    assert!(
        Command::new("ruby")
            .arg(root.join("scripts/build-netsurf.rb"))
            .arg(&inputs)
            .arg("arm")
            .status()
            .expect("build NetSurf")
            .success()
    );
    println!(
        "cargo:rustc-link-search=native={}/arm/lib",
        inputs.display()
    );
    for lib in [
        "brain-netsurf",
        "dom",
        "css",
        "hubbub",
        "parserutils",
        "wapcaplet",
        "nsutils",
        "nsbmp",
        "nsgif",
        "jpeg",
        "iconv",
        "z",
    ] {
        println!("cargo:rustc-link-lib=static={lib}");
    }
    println!(
        "cargo:rustc-link-search=native={}/newlib/usr/lib/arm-none-eabi/newlib/thumb/v7-a/nofp",
        inputs.display()
    );
    println!("cargo:rustc-link-lib=static=c");
    println!("cargo:rustc-link-lib=static=m");
}
