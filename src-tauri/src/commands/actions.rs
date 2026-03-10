use std::sync::atomic::Ordering;
use tauri::{AppHandle, Emitter, Manager};

/// Reset the abort flag — call at the start of plan execution.
#[tauri::command]
pub fn start_execution(app: AppHandle) -> Result<(), String> {
    let state = app.state::<crate::AppState>();
    state.abort_flag.store(false, Ordering::Relaxed);
    Ok(())
}

/// Set the abort flag — call to cancel mid-execution.
#[tauri::command]
pub fn cancel_execution(app: AppHandle) -> Result<(), String> {
    let state = app.state::<crate::AppState>();
    state.abort_flag.store(true, Ordering::Relaxed);
    Ok(())
}

#[tauri::command]
pub async fn execute_action(
    app: AppHandle,
    action_type: String,
    params: serde_json::Value,
) -> Result<(), String> {
    // Check abort flag before each step
    let state = app.state::<crate::AppState>();
    if state.abort_flag.load(Ordering::Relaxed) {
        return Err("Execution cancelled".to_string());
    }

    // Emit step progress event
    let _ = app.emit("step-progress", &action_type);

    match action_type.as_str() {
        "open_url" => {
            let url = params["url"].as_str().ok_or("Missing url parameter")?;
            // Only allow http/https URLs to prevent file:// or custom scheme attacks
            if !url.starts_with("http://") && !url.starts_with("https://") {
                return Err(format!("Blocked URL with unsupported scheme: {}", url));
            }
            open::that(url).map_err(|e| format!("Failed to open URL: {}", e))
        }
        "open_app" => {
            let app_id = params["app"].as_str().ok_or("Missing app parameter")?;
            // Sanitize: only allow alphanumeric, spaces, dots, hyphens
            if !app_id
                .chars()
                .all(|c| c.is_alphanumeric() || c == ' ' || c == '.' || c == '-')
            {
                return Err("Invalid app identifier".to_string());
            }
            // Use bundle_id (-b) for reverse-DNS identifiers, app name (-a) for plain names
            let is_bundle_id = app_id.contains('.') && !app_id.contains(' ');
            let flag = if is_bundle_id { "-b" } else { "-a" };
            let output = std::process::Command::new("open")
                .args([flag, app_id])
                .output()
                .map_err(|e| format!("Failed to open app: {}", e))?;
            if output.status.success() {
                // Give the app time to activate and focus before next step
                std::thread::sleep(std::time::Duration::from_millis(800));
                Ok(())
            } else {
                Err(String::from_utf8_lossy(&output.stderr).to_string())
            }
        }
        "copy_to_clipboard" => {
            let text = params["text"].as_str().ok_or("Missing text parameter")?;
            let mut child = std::process::Command::new("pbcopy")
                .stdin(std::process::Stdio::piped())
                .spawn()
                .map_err(|e| format!("Failed to run pbcopy: {}", e))?;
            use std::io::Write;
            child
                .stdin
                .as_mut()
                .unwrap()
                .write_all(text.as_bytes())
                .map_err(|e| format!("Failed to write to pbcopy: {}", e))?;
            child.wait().map_err(|e| e.to_string())?;
            Ok(())
        }
        "run_command" => {
            let cmd = params["command"]
                .as_str()
                .ok_or("Missing command parameter")?;
            validate_command(cmd)?;
            let output = std::process::Command::new("sh")
                .args(["-c", cmd])
                .output()
                .map_err(|e| format!("Failed to run command: {}", e))?;
            if output.status.success() {
                Ok(())
            } else {
                Err(String::from_utf8_lossy(&output.stderr).to_string())
            }
        }
        "notify" => {
            let message = params["message"]
                .as_str()
                .ok_or("Missing message parameter")?;
            // Use tauri-plugin-notification — no shell injection risk
            tauri_plugin_notification::NotificationExt::notification(&app)
                .builder()
                .title("YoYo")
                .body(message)
                .show()
                .map_err(|e| format!("Failed to send notification: {}", e))?;
            Ok(())
        }
        "claude_code" => {
            let prompt = params["prompt"]
                .as_str()
                .ok_or("Missing prompt parameter")?;
            let dir = params["directory"].as_str().unwrap_or(".");

            // Validate directory exists
            let dir_path = std::path::Path::new(dir);
            if !dir_path.exists() {
                return Err(format!("Directory not found: {}", dir));
            }

            // Spawn Claude CLI as async subprocess
            let output = tokio::process::Command::new("claude")
                .args(["-p", prompt])
                .current_dir(dir_path)
                .output()
                .await
                .map_err(|e| format!("Failed to run claude: {}", e))?;

            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let _ = app.emit("step-output", &stdout);
                Ok(())
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                Err(format!("Claude CLI failed: {}", stderr))
            }
        }
        "insert_text" => {
            let text = params["text"].as_str().ok_or("Missing text parameter")?;
            if text.len() > 50_000 {
                return Err("Text too long (max 50,000 chars)".to_string());
            }
            // Copy to clipboard
            let mut child = std::process::Command::new("pbcopy")
                .stdin(std::process::Stdio::piped())
                .spawn()
                .map_err(|e| format!("Failed to run pbcopy: {}", e))?;
            use std::io::Write;
            child
                .stdin
                .as_mut()
                .unwrap()
                .write_all(text.as_bytes())
                .map_err(|e| format!("Failed to write to pbcopy: {}", e))?;
            child.wait().map_err(|e| e.to_string())?;

            // Brief pause to ensure clipboard is ready
            std::thread::sleep(std::time::Duration::from_millis(100));

            // Simulate Cmd+V paste via osascript (controlled action, not user input)
            let output = std::process::Command::new("osascript")
                .args([
                    "-e",
                    r#"tell application "System Events" to keystroke "v" using command down"#,
                ])
                .output()
                .map_err(|e| format!("Failed to paste: {}", e))?;
            if !output.status.success() {
                return Err(format!(
                    "Paste failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                ));
            }
            Ok(())
        }
        _ => Err(format!("Unknown action type: {}", action_type)),
    }
}

/// Check if inserted text is still present in the focused app's accessibility tree.
#[tauri::command]
pub fn check_inserted_text(
    app: AppHandle,
    original_text: String,
) -> Result<serde_json::Value, String> {
    let state = app.state::<crate::AppState>();
    let pid = state.current_app_pid.load(Ordering::Relaxed) as i32;
    if pid <= 0 {
        return Ok(serde_json::json!({ "found": false, "reason": "no_app" }));
    }

    match crate::accessibility::extract_text(pid) {
        Ok(result) if result.error.is_none() => {
            let ax_text = result.text;
            let found = if original_text.len() > 100 {
                ax_text.contains(&original_text[..100])
            } else {
                ax_text.contains(&original_text)
            };
            Ok(serde_json::json!({
                "found": found,
                "reverted": !found,
            }))
        }
        _ => Ok(serde_json::json!({ "found": false, "reason": "ax_failed" })),
    }
}

/// Validate a shell command against dangerous patterns.
/// Uses an expanded blocklist + structural pattern detection.
fn validate_command(cmd: &str) -> Result<(), String> {
    let lower = cmd.to_lowercase();

    // Blocked command patterns (case-insensitive)
    let blocked_patterns = [
        "rm -rf",
        "rm -r -f",
        "rm -fr",
        "sudo",
        "su -",
        "mkfs",
        "fdisk",
        "parted",
        "dd if=",
        "dd of=",
        "> /dev/",
        ">/dev/",
        "chmod -r 777",
        "chmod 777",
        "curl | sh",
        "curl |sh",
        "curl|sh",
        "wget | sh",
        "wget |sh",
        "wget|sh",
        "curl | bash",
        "curl |bash",
        "curl|bash",
        "wget | bash",
        "wget |bash",
        "wget|bash",
        "eval ",
        "exec ",
        ":(){ ",
        ":(){", // fork bomb
        "/etc/passwd",
        "/etc/shadow",
        "launchctl",
        "defaults write",
        "networksetup",
        "systemsetup",
        "osascript", // prevent AppleScript via command
        "security delete",
        "security add",
        "killall",
        "pkill -9",
        "shutdown",
        "reboot",
        "halt",
    ];

    for pattern in &blocked_patterns {
        if lower.contains(pattern) {
            return Err(format!("Blocked dangerous command pattern: {}", pattern));
        }
    }

    // Block shell injection patterns: $(...), `...`, ${...}
    if cmd.contains("$(") || cmd.contains('`') || cmd.contains("${") {
        return Err("Blocked: command substitution not allowed".to_string());
    }

    // Block output redirection to arbitrary files (allow /dev/null)
    let stripped = cmd.replace("/dev/null", "");
    if stripped.contains(">>") || stripped.contains("> /") || stripped.contains(">/") {
        return Err("Blocked: output redirection not allowed".to_string());
    }

    // Block piping to interpreters
    let pipe_targets = ["sh", "bash", "zsh", "python", "perl", "ruby", "node"];
    if let Some(pipe_pos) = cmd.find('|') {
        let after_pipe = cmd[pipe_pos + 1..].trim();
        for target in &pipe_targets {
            if after_pipe.starts_with(target)
                && after_pipe[target.len()..].starts_with(|c: char| c.is_whitespace() || c == '\0')
                || after_pipe == *target
            {
                return Err(format!("Blocked: piping to {} not allowed", target));
            }
        }
    }

    Ok(())
}
