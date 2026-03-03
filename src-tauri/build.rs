fn main() {
    let out_dir = std::env::var("OUT_DIR").unwrap();

    // Compile Swift OCR helper binary
    let ocr_binary = format!("{}/yoyo-ocr", out_dir);
    println!("cargo:rerun-if-changed=swift/ocr.swift");
    println!("cargo:rustc-env=YOYO_OCR_BINARY={}", ocr_binary);

    let status = std::process::Command::new("swiftc")
        .args(["swift/ocr.swift", "-o", &ocr_binary, "-O"])
        .status()
        .expect("swiftc is required — install Xcode Command Line Tools");
    assert!(status.success(), "Failed to compile Swift OCR helper");

    // Compile Swift focus capture helper binary
    let focus_binary = format!("{}/yoyo-focus", out_dir);
    println!("cargo:rerun-if-changed=swift/focus_capture.swift");
    println!("cargo:rustc-env=YOYO_FOCUS_BINARY={}", focus_binary);

    let status = std::process::Command::new("swiftc")
        .args([
            "swift/focus_capture.swift",
            "-o",
            &focus_binary,
            "-O",
            "-framework",
            "AppKit",
            "-framework",
            "CoreGraphics",
        ])
        .status()
        .expect("swiftc is required — install Xcode Command Line Tools");
    assert!(status.success(), "Failed to compile Swift focus capture helper");

    tauri_build::build()
}
