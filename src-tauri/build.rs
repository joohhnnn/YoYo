fn main() {
    // Compile Swift OCR helper binary
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let ocr_binary = format!("{}/yoyo-ocr", out_dir);

    println!("cargo:rerun-if-changed=swift/ocr.swift");
    println!("cargo:rustc-env=YOYO_OCR_BINARY={}", ocr_binary);

    let status = std::process::Command::new("swiftc")
        .args(["swift/ocr.swift", "-o", &ocr_binary, "-O"])
        .status()
        .expect("swiftc is required — install Xcode Command Line Tools");

    assert!(status.success(), "Failed to compile Swift OCR helper");

    tauri_build::build()
}
