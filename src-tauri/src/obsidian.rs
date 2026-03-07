use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

/// Check if a path is a valid Obsidian vault (contains .obsidian/ directory).
pub fn is_valid_vault(path: &str) -> bool {
    if path.is_empty() {
        return false;
    }
    Path::new(path).join(".obsidian").is_dir()
}

/// Scan common macOS locations for Obsidian vaults.
/// Returns a list of vault root paths.
pub fn detect_vaults() -> Vec<String> {
    let home = match dirs::home_dir() {
        Some(h) => h,
        None => return vec![],
    };

    let search_roots = [
        home.join("Documents"),
        home.join("Obsidian"),
        home.join("Library/Mobile Documents/iCloud~md~obsidian/Documents"),
        home.join("Desktop"),
    ];

    let mut vaults = Vec::new();
    for root in &search_roots {
        if !root.is_dir() {
            continue;
        }
        if root.join(".obsidian").is_dir() {
            vaults.push(root.to_string_lossy().to_string());
            continue;
        }
        if let Ok(entries) = fs::read_dir(root) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_dir() && p.join(".obsidian").is_dir() {
                    vaults.push(p.to_string_lossy().to_string());
                }
            }
        }
    }
    vaults
}

// --- Write: Reflection → Vault ---

/// Write a reflection summary to the vault's YoYo/ subfolder.
/// Creates `{vault}/YoYo/YYYY-MM-DD.md` and appends if the file already exists.
pub fn sync_reflection(
    vault_path: &str,
    summary: &str,
    activity_count: i64,
    period_start: &str,
    period_end: &str,
) -> Result<(), String> {
    if !is_valid_vault(vault_path) {
        return Err("Invalid vault path".to_string());
    }

    let yoyo_dir = Path::new(vault_path).join("YoYo");
    fs::create_dir_all(&yoyo_dir).map_err(|e| format!("Failed to create YoYo dir: {}", e))?;

    let today = chrono::Local::now();
    let date_str = today.format("%Y-%m-%d").to_string();
    let time_str = today.format("%H:%M").to_string();
    let file_path = yoyo_dir.join(format!("{}.md", date_str));

    let section = format!(
        "\n## Reflection ({})\n{}\n\n- Activities: {}\n- Period: {} ~ {}\n\n---\n",
        time_str, summary, activity_count, period_start, period_end
    );

    if file_path.exists() {
        let mut content = fs::read_to_string(&file_path)
            .map_err(|e| format!("Failed to read {}: {}", file_path.display(), e))?;
        content.push_str(&section);
        fs::write(&file_path, content)
            .map_err(|e| format!("Failed to write {}: {}", file_path.display(), e))?;
    } else {
        let content = format!("# {}\n{}", date_str, section);
        fs::write(&file_path, content)
            .map_err(|e| format!("Failed to write {}: {}", file_path.display(), e))?;
    }

    // Try to append to Obsidian daily note (best-effort)
    if let Err(e) = append_to_daily_note(vault_path, summary) {
        eprintln!("Daily note append skipped: {}", e);
    }

    Ok(())
}

/// Try to detect the daily note configuration and append a YoYo section.
fn append_to_daily_note(vault_path: &str, summary: &str) -> Result<(), String> {
    let config_path = Path::new(vault_path)
        .join(".obsidian")
        .join("daily-notes.json");

    if !config_path.exists() {
        return Err("No daily-notes config found".to_string());
    }

    let config_str = fs::read_to_string(&config_path)
        .map_err(|e| format!("Failed to read daily-notes config: {}", e))?;
    let config: serde_json::Value = serde_json::from_str(&config_str)
        .map_err(|e| format!("Failed to parse daily-notes config: {}", e))?;

    let folder = config["folder"].as_str().unwrap_or("");
    let format_str = config["format"].as_str().unwrap_or("YYYY-MM-DD");

    let chrono_fmt = format_str
        .replace("YYYY", "%Y")
        .replace("MM", "%m")
        .replace("DD", "%d");
    let today_name = chrono::Local::now().format(&chrono_fmt).to_string();

    let daily_dir = if folder.is_empty() {
        PathBuf::from(vault_path)
    } else {
        Path::new(vault_path).join(folder)
    };
    let daily_path = daily_dir.join(format!("{}.md", today_name));

    if !daily_path.exists() {
        return Err("Today's daily note doesn't exist yet".to_string());
    }

    let mut content =
        fs::read_to_string(&daily_path).map_err(|e| format!("Failed to read daily note: {}", e))?;

    if content.contains("## YoYo") {
        return Ok(());
    }

    // Truncate summary for daily note (first 200 chars)
    let brief = if summary.len() > 200 {
        let end = summary
            .char_indices()
            .take_while(|(i, _)| *i < 200)
            .last()
            .map(|(i, c)| i + c.len_utf8())
            .unwrap_or(200);
        format!("{}...", &summary[..end])
    } else {
        summary.to_string()
    };

    content.push_str(&format!("\n\n## YoYo\n{}\n", brief));
    fs::write(&daily_path, content).map_err(|e| format!("Failed to write daily note: {}", e))?;

    Ok(())
}

// --- Read: Vault → AI Context ---

// File list cache to avoid re-walking vault on every analysis
static VAULT_CACHE: std::sync::OnceLock<Mutex<(Vec<PathBuf>, SystemTime)>> =
    std::sync::OnceLock::new();

const MAX_FILES: usize = 500;
const MAX_FILE_SIZE: u64 = 50_000; // 50KB
const MAX_CONTEXT_CHARS: usize = 1500;
const CACHE_TTL_SECS: u64 = 60;

const SKIP_DIRS: &[&str] = &[".obsidian", ".trash", "node_modules", ".git", "YoYo"];

/// Search the Obsidian vault for notes matching the given keywords.
/// Returns a combined string of relevant note excerpts, or None if no matches.
pub fn search_vault(vault_path: &str, keywords: &[&str]) -> Option<String> {
    if !is_valid_vault(vault_path) || keywords.is_empty() {
        return None;
    }

    let files = get_vault_files(vault_path);
    if files.is_empty() {
        return None;
    }

    let mut scored: Vec<(PathBuf, usize)> = Vec::new();
    for file_path in &files {
        let filename = file_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase();

        let mut score: usize = 0;

        // Filename matches worth 2x
        for kw in keywords {
            if kw.len() < 2 {
                continue;
            }
            let kw_lower = kw.to_lowercase();
            if filename.contains(&kw_lower) {
                score += 2;
            }
        }

        // Check file size before reading content
        if let Ok(metadata) = fs::metadata(file_path) {
            if metadata.len() > MAX_FILE_SIZE {
                continue;
            }
        }

        // Content matches worth 1x
        if let Ok(content) = fs::read_to_string(file_path) {
            let content_lower = content.to_lowercase();
            for kw in keywords {
                if kw.len() < 2 {
                    continue;
                }
                let kw_lower = kw.to_lowercase();
                if content_lower.contains(&kw_lower) {
                    score += 1;
                }
            }
        }

        if score > 0 {
            scored.push((file_path.clone(), score));
        }
    }

    if scored.is_empty() {
        return None;
    }

    scored.sort_by(|a, b| b.1.cmp(&a.1));
    scored.truncate(3);

    let mut result = String::new();
    let mut total_chars = 0;

    for (path, _score) in &scored {
        if total_chars >= MAX_CONTEXT_CHARS {
            break;
        }
        let note_name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("note");
        if let Ok(content) = fs::read_to_string(path) {
            let remaining = MAX_CONTEXT_CHARS - total_chars;
            let excerpt = if content.len() > remaining {
                let end = content
                    .char_indices()
                    .take_while(|(i, _)| *i < remaining)
                    .last()
                    .map(|(i, c)| i + c.len_utf8())
                    .unwrap_or(remaining);
                format!("{}...", &content[..end])
            } else {
                content
            };
            result.push_str(&format!("### {}\n{}\n\n", note_name, excerpt));
            total_chars += excerpt.len();
        }
    }

    if result.is_empty() {
        None
    } else {
        Some(result)
    }
}

/// Get the list of .md files in the vault, using a cache.
fn get_vault_files(vault_path: &str) -> Vec<PathBuf> {
    let cache = VAULT_CACHE.get_or_init(|| Mutex::new((Vec::new(), SystemTime::UNIX_EPOCH)));
    let mut guard = match cache.lock() {
        Ok(g) => g,
        Err(_) => return Vec::new(),
    };

    if let Ok(elapsed) = guard.1.elapsed() {
        if elapsed < Duration::from_secs(CACHE_TTL_SECS) && !guard.0.is_empty() {
            return guard.0.clone();
        }
    }

    let mut files = Vec::new();
    walk_md_files(Path::new(vault_path), &mut files, 0);
    files.truncate(MAX_FILES);

    guard.0 = files.clone();
    guard.1 = SystemTime::now();

    files
}

/// Recursively walk directory for .md files, skipping excluded dirs.
fn walk_md_files(dir: &Path, result: &mut Vec<PathBuf>, depth: usize) {
    if depth > 5 || result.len() >= MAX_FILES {
        return;
    }
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if SKIP_DIRS.contains(&name) || name.starts_with('.') {
                continue;
            }
            walk_md_files(&path, result, depth + 1);
        } else if path.extension().and_then(|e| e.to_str()) == Some("md") {
            result.push(path);
        }
        if result.len() >= MAX_FILES {
            return;
        }
    }
}
