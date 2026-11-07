use std::{env, fs, path::PathBuf, process::Command};
fn main() {
    println!("cargo:rerun-if-env-changed=LLM_INPUTS");
    println!("cargo:rerun-if-changed=prepare-elan.rb");
    if env::var_os("CARGO_FEATURE_DEVICE").is_none() {
        return;
    }
    let root =
        PathBuf::from(env::var_os("LLM_INPUTS").expect("Build with reflex-cli to download models"));
    let inputs = root.join("downloads");
    for name in [
        "elan.safetensors",
        "elan-source.spm",
        "elan-vocab.json",
        "elan.bin",
        "elan.tok",
        "elan.bin.sha256",
        "elan.tok.sha256",
    ] {
        println!("cargo:rerun-if-changed={}", inputs.join(name).display());
    }
    assert!(
        Command::new("ruby")
            .arg("prepare-elan.rb")
            .arg(&inputs)
            .status()
            .expect("start Ruby")
            .success(),
        "Elan conversion failed"
    );
    println!(
        "cargo:rustc-env=ELAN_MODEL_HASH={}",
        inputs.join("elan.bin.sha256").display()
    );
    println!(
        "cargo:rustc-env=ELAN_TOKENIZER_HASH={}",
        inputs.join("elan.tok.sha256").display()
    );
    // Remove only the previous generated Qwen payload, so deploy no longer copies 429 MB.
    for name in [
        "qwen2.5-0.5b-instruct-q4_0.gguf",
        "qwen.meta",
        "LICENSE-QWEN.txt",
    ] {
        let path = root.join("APP/LLM").join(name);
        if path.exists() {
            fs::remove_file(&path).expect("remove obsolete generated Qwen asset");
        }
    }
}
