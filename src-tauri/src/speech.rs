//! macOS Speech framework integration.
//! Permission checking via raw objc2 FFI.
//! Transcription via compiled Swift helper binary (yoyo-speech).

use objc2::msg_send;
use objc2::runtime::{AnyClass, Bool};
use objc2_foundation::NSString;
use std::sync::mpsc;
use std::time::Duration;

// Link required macOS frameworks for permission checking
#[link(name = "Speech", kind = "framework")]
extern "C" {}

#[link(name = "AVFoundation", kind = "framework")]
extern "C" {}

/// Permission status for speech recognition or microphone access.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PermissionStatus {
    NotDetermined,
    Denied,
    Restricted,
    Authorized,
}

impl PermissionStatus {
    fn from_raw(value: isize) -> Self {
        match value {
            0 => Self::NotDetermined,
            1 => Self::Denied,
            2 => Self::Restricted,
            3 => Self::Authorized,
            _ => Self::Denied,
        }
    }
}

impl std::fmt::Display for PermissionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Authorized => write!(f, "granted"),
            Self::Denied => write!(f, "denied"),
            Self::NotDetermined => write!(f, "not_determined"),
            Self::Restricted => write!(f, "restricted"),
        }
    }
}

/// Check SFSpeechRecognizer authorization status.
pub fn check_speech_permission() -> PermissionStatus {
    unsafe {
        let class = match AnyClass::get(c"SFSpeechRecognizer") {
            Some(c) => c,
            None => return PermissionStatus::Denied,
        };
        let status: isize = msg_send![class, authorizationStatus];
        PermissionStatus::from_raw(status)
    }
}

/// Check microphone (AVCaptureDevice) authorization status.
pub fn check_mic_permission() -> PermissionStatus {
    unsafe {
        let class = match AnyClass::get(c"AVCaptureDevice") {
            Some(c) => c,
            None => return PermissionStatus::Denied,
        };
        // AVMediaTypeAudio = "soun"
        let media_type = NSString::from_str("soun");
        let status: isize = msg_send![class, authorizationStatusForMediaType: &*media_type];
        PermissionStatus::from_raw(status)
    }
}

/// Request speech recognition permission. Blocks until user responds.
pub fn request_speech_permission() -> Result<bool, String> {
    let (tx, rx) = mpsc::channel();

    unsafe {
        let class =
            AnyClass::get(c"SFSpeechRecognizer").ok_or("SFSpeechRecognizer not available")?;

        let block = block2::RcBlock::new(move |status: isize| {
            let _ = tx.send(status == 3); // 3 = authorized
        });

        let _: () = msg_send![class, requestAuthorization: &*block];
    }

    rx.recv_timeout(Duration::from_secs(60))
        .map_err(|_| "Permission request timed out".to_string())
}

/// Request microphone permission. Blocks until user responds.
pub fn request_mic_permission() -> Result<bool, String> {
    let (tx, rx) = mpsc::channel();

    unsafe {
        let class = AnyClass::get(c"AVCaptureDevice").ok_or("AVCaptureDevice not available")?;

        let media_type = NSString::from_str("soun");

        let block = block2::RcBlock::new(move |granted: Bool| {
            let _ = tx.send(granted.as_bool());
        });

        let _: () =
            msg_send![class, requestAccessForMediaType: &*media_type, completionHandler: &*block];
    }

    rx.recv_timeout(Duration::from_secs(60))
        .map_err(|_| "Permission request timed out".to_string())
}

/// Path to the compiled Swift speech recognition helper.
const SPEECH_BINARY: &str = env!("YOYO_SPEECH_BINARY");

/// Transcribe an audio file using the Swift speech recognition helper.
/// The helper uses SFSpeechRecognizer for on-device transcription.
pub fn transcribe_file(path: &str, locale: &str) -> Result<String, String> {
    let output = std::process::Command::new(SPEECH_BINARY)
        .args([path, locale])
        .output()
        .map_err(|e| format!("Failed to run speech helper: {}", e))?;

    if output.status.success() {
        let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(text)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(stderr)
    }
}
