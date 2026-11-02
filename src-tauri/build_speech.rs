use std::{env, fs, path::PathBuf, process::Command};

pub fn build() {
    let source = PathBuf::from("src/transcription/SpeechHelper.swift");
    let info = PathBuf::from("src/transcription/HelperInfo.plist");
    println!("cargo:rerun-if-changed={}", source.display());
    println!("cargo:rerun-if-changed={}", info.display());
    println!("cargo:rerun-if-changed=build_speech.rs");
    let target = env::var("TARGET").expect("Cargo target");
    let architecture = match target.as_str() {
        "aarch64-apple-darwin" => "arm64",
        "x86_64-apple-darwin" => "x86_64",
        other => panic!("Unsupported speech helper target: {other}"),
    };
    fs::create_dir_all("binaries").expect("Create helper output directory");
    let output = PathBuf::from("binaries").join(format!("tarantino-speech-{target}"));
    let cache =
        PathBuf::from(env::var_os("OUT_DIR").expect("Cargo build output")).join("swift-cache");
    let status = Command::new("xcrun")
        .args(["swiftc", "-parse-as-library", "-O", "-target"])
        .arg(format!("{architecture}-apple-macos12.3"))
        .arg("-module-cache-path")
        .arg(cache)
        .arg(&source)
        .args([
            "-Xlinker",
            "-sectcreate",
            "-Xlinker",
            "__TEXT",
            "-Xlinker",
            "__info_plist",
            "-Xlinker",
        ])
        .arg(&info)
        .arg("-o")
        .arg(&output)
        .status()
        .expect("Run Swift compiler for speech helper");
    assert!(status.success(), "Speech helper compilation failed");
}
