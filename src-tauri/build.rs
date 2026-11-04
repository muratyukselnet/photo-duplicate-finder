use std::{env, path::PathBuf, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=../native/photo-analysis/Sources/PhotoAnalysis/main.swift");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        let target = env::var("TARGET").expect("Cargo target");
        let arch = if target.starts_with("aarch64") {
            "arm64"
        } else if target.starts_with("x86_64") {
            "x86_64"
        } else {
            panic!("unsupported macOS target: {target}")
        };
        let output = PathBuf::from("binaries").join(format!("photo-analysis-{target}"));
        std::fs::create_dir_all("binaries").expect("create sidecar directory");
        let cache = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo output directory"))
            .join("swift-module-cache");
        let status = Command::new("xcrun")
            .args([
                "swiftc",
                "-O",
                "-target",
                &format!("{arch}-apple-macosx12.0"),
                "-module-cache-path",
            ])
            .arg(cache)
            .arg("../native/photo-analysis/Sources/PhotoAnalysis/main.swift")
            .arg("-o")
            .arg(output)
            .status()
            .expect("Swift compiler is required to build the ImageIO helper");
        assert!(status.success(), "failed to build ImageIO helper");
    }
    tauri_build::build()
}
