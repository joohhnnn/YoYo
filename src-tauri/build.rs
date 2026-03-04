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

    // Compile Swift window list helper binary
    let windows_binary = format!("{}/yoyo-windows", out_dir);
    println!("cargo:rerun-if-changed=swift/window_list.swift");
    println!("cargo:rustc-env=YOYO_WINDOWS_BINARY={}", windows_binary);

    let status = std::process::Command::new("swiftc")
        .args([
            "swift/window_list.swift",
            "-o",
            &windows_binary,
            "-O",
            "-framework",
            "AppKit",
            "-framework",
            "CoreGraphics",
        ])
        .status()
        .expect("swiftc is required — install Xcode Command Line Tools");
    assert!(status.success(), "Failed to compile Swift window list helper");

    // Compile Swift accessibility helper binary
    let ax_binary = format!("{}/yoyo-ax", out_dir);
    println!("cargo:rerun-if-changed=swift/accessibility.swift");
    println!("cargo:rustc-env=YOYO_AX_BINARY={}", ax_binary);

    let status = std::process::Command::new("swiftc")
        .args([
            "swift/accessibility.swift",
            "-o",
            &ax_binary,
            "-O",
            "-framework",
            "AppKit",
            "-framework",
            "ApplicationServices",
            "-framework",
            "CoreGraphics",
        ])
        .status()
        .expect("swiftc is required — install Xcode Command Line Tools");
    assert!(status.success(), "Failed to compile Swift accessibility helper");

    tauri_build::build()
}
