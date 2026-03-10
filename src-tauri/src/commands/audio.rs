use crate::speech;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, Manager};

/// List available audio input devices.
#[tauri::command]
pub fn list_audio_devices() -> Result<Vec<serde_json::Value>, String> {
    let host = cpal::default_host();
    let mut devices = Vec::new();
    for device in host.input_devices().map_err(|e| e.to_string())? {
        if let Ok(name) = device.name() {
            devices.push(serde_json::json!({ "name": name }));
        }
    }
    Ok(devices)
}

/// Check both microphone and speech recognition permissions.
/// Returns "granted", "denied", or "not_determined".
#[tauri::command]
pub fn check_voice_permission() -> Result<String, String> {
    let mic = speech::check_mic_permission();
    let speech = speech::check_speech_permission();

    // Return the most restrictive status
    if mic == speech::PermissionStatus::Authorized && speech == speech::PermissionStatus::Authorized
    {
        Ok("granted".to_string())
    } else if mic == speech::PermissionStatus::NotDetermined
        || speech == speech::PermissionStatus::NotDetermined
    {
        Ok("not_determined".to_string())
    } else {
        Ok("denied".to_string())
    }
}

/// Request both microphone and speech recognition permissions.
/// Returns true if both are granted.
#[tauri::command]
pub async fn request_voice_permission() -> Result<bool, String> {
    // Request mic permission first (blocking, runs in tokio blocking pool)
    let mic_granted = tokio::task::spawn_blocking(|| speech::request_mic_permission())
        .await
        .map_err(|e| format!("Task join error: {}", e))??;

    if !mic_granted {
        return Ok(false);
    }

    // Then request speech recognition permission
    let speech_granted = tokio::task::spawn_blocking(|| speech::request_speech_permission())
        .await
        .map_err(|e| format!("Task join error: {}", e))??;

    Ok(speech_granted)
}

/// Start recording from the default microphone.
/// Saves audio as 16kHz mono WAV to a temp file.
/// Returns the temp file path.
#[tauri::command]
pub fn start_recording(app: AppHandle) -> Result<String, String> {
    let state = app.state::<crate::AppState>();

    // Don't start if already recording
    if state.recording.lock().unwrap().is_some() {
        return Err("Already recording".to_string());
    }

    // Generate temp file path
    let temp_dir = std::env::temp_dir();
    let file_path = temp_dir
        .join(format!("yoyo_recording_{}.wav", uuid::Uuid::new_v4()))
        .to_string_lossy()
        .to_string();

    // Set up cpal input — use preferred device if configured
    let host = cpal::default_host();
    let preferred = {
        let data = super::settings::load_data(&app);
        data.settings.preferred_mic_device.clone()
    };
    let device = if !preferred.is_empty() {
        host.input_devices()
            .map_err(|e| e.to_string())?
            .find(|d| d.name().map(|n| n == preferred).unwrap_or(false))
            .unwrap_or(
                host.default_input_device()
                    .ok_or("No input device available")?,
            )
    } else {
        host.default_input_device()
            .ok_or("No input device available")?
    };

    // Use a config suitable for speech: mono 16kHz
    let config = cpal::StreamConfig {
        channels: 1,
        sample_rate: cpal::SampleRate(16000),
        buffer_size: cpal::BufferSize::Default,
    };

    // Create WAV writer
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 16000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let writer = hound::WavWriter::create(&file_path, spec)
        .map_err(|e| format!("Failed to create WAV file: {}", e))?;
    let writer = Arc::new(Mutex::new(Some(writer)));

    // Build input stream
    let writer_clone = writer.clone();
    let err_fn = |err: cpal::StreamError| {
        log::error!("Audio stream error: {}", err);
    };

    let stream = device
        .build_input_stream(
            &config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                if let Ok(mut guard) = writer_clone.lock() {
                    if let Some(ref mut w) = *guard {
                        for &sample in data {
                            // Convert f32 [-1.0, 1.0] to i16
                            let s = (sample * 32767.0).clamp(-32768.0, 32767.0) as i16;
                            let _ = w.write_sample(s);
                        }
                    }
                }
            },
            err_fn,
            None,
        )
        .map_err(|e| format!("Failed to build input stream: {}", e))?;

    stream
        .play()
        .map_err(|e| format!("Failed to start recording: {}", e))?;

    // Store recording state
    let recording_state = crate::RecordingState {
        _stream: stream,
        writer,
        file_path: file_path.clone(),
        start_time: std::time::Instant::now(),
    };

    *state.recording.lock().unwrap() = Some(recording_state);

    let _ = app.emit("recording-started", ());

    Ok(file_path)
}

/// Stop recording, finalize WAV file, transcribe with SFSpeechRecognizer.
/// Returns the transcribed text.
#[tauri::command]
pub async fn stop_and_transcribe(app: AppHandle) -> Result<String, String> {
    let (file_path, language) = {
        let state = app.state::<crate::AppState>();
        let recording = state
            .recording
            .lock()
            .unwrap()
            .take()
            .ok_or("Not recording")?;

        // Drop the stream (stops recording) and finalize WAV
        drop(recording._stream);
        if let Ok(mut guard) = recording.writer.lock() {
            if let Some(writer) = guard.take() {
                let _ = writer.finalize();
            }
        }

        // Get language setting
        let data = super::settings::load_data(&app);
        let locale = match data.settings.language.as_str() {
            "zh" => "zh-Hans",
            "en" => "en-US",
            _ => "en-US",
        };

        (recording.file_path, locale.to_string())
    };

    let _ = app.emit("analysis-progress", "Transcribing...");

    // Run transcription in a blocking task (FFI calls)
    let file_path_clone = file_path.clone();
    let result =
        tokio::task::spawn_blocking(move || speech::transcribe_file(&file_path_clone, &language))
            .await
            .map_err(|e| format!("Task join error: {}", e))?;

    // Clean up temp file
    let _ = std::fs::remove_file(&file_path);

    result
}
