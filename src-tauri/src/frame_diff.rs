use image::imageops::FilterType;
use std::path::PathBuf;

/// Capture a lightweight screenshot for stability comparison.
/// Uses a separate path from the main analysis screenshot to avoid conflicts.
fn capture_stability_frame() -> Result<PathBuf, String> {
    let path = std::env::temp_dir().join("yoyo-stability.png");
    let path_str = path.to_str().ok_or("Invalid path")?.to_string();

    let output = std::process::Command::new("screencapture")
        .args(["-x", "-C", &path_str])
        .output()
        .map_err(|e| format!("Failed to run screencapture: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "screencapture failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(path)
}

/// Compute an average hash (aHash) of a screenshot.
/// Resizes to 8×8 grayscale (64 pixels) and produces a 64-bit hash
/// where each bit indicates whether the pixel is above the mean brightness.
fn compute_ahash(path: &std::path::Path) -> Result<u64, String> {
    let img = image::open(path).map_err(|e| format!("Failed to open image: {}", e))?;

    // Resize to 8×8 grayscale
    let small = img.resize_exact(8, 8, FilterType::Lanczos3).to_luma8();

    // Compute mean pixel value
    let pixels: Vec<u8> = small.pixels().map(|p| p.0[0]).collect();
    let mean: f64 = pixels.iter().map(|&p| p as f64).sum::<f64>() / pixels.len() as f64;

    // Build hash: bit = 1 if pixel > mean
    let mut hash: u64 = 0;
    for (i, &pixel) in pixels.iter().enumerate() {
        if pixel as f64 > mean {
            hash |= 1u64 << i;
        }
    }

    Ok(hash)
}

/// Hamming distance between two aHash values.
fn hamming_distance(a: u64, b: u64) -> u32 {
    (a ^ b).count_ones()
}

/// Quick check: is the screen actively changing?
/// Takes two frames 500ms apart and compares their perceptual hashes.
/// Returns true if the screen is changing significantly (hamming distance > threshold).
///
/// A threshold of ~12 (out of 64 bits, ~19%) catches rapid changes like typing
/// or scrolling, while ignoring minor updates like cursor blinks or clock ticks.
pub fn is_screen_changing(threshold: u32) -> Result<bool, String> {
    let frame1 = capture_stability_frame()?;
    let hash1 = compute_ahash(&frame1)?;

    std::thread::sleep(std::time::Duration::from_millis(500));

    let frame2 = capture_stability_frame()?;
    let hash2 = compute_ahash(&frame2)?;

    let dist = hamming_distance(hash1, hash2);
    Ok(dist > threshold)
}
