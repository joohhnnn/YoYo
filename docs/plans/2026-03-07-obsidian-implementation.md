# Obsidian Integration Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add optional Obsidian vault integration — sync reflections to vault, read vault notes as AI context.

**Architecture:** Pure file-system approach. New `obsidian.rs` module handles all vault I/O. Obsidian is an optional sync channel; when `obsidian_vault_path` is empty, all code paths no-op. The reflection pipeline gains a `sync_to_obsidian()` call at the end; the analysis pipeline gains an `obsidian_context` parameter threaded through prompt building.

**Tech Stack:** Rust (std::fs, walkdir-like manual recursion), TypeScript/React (SettingsPanel UI)

---

### Task 1: Settings field — Rust side

**Files:**
- Modify: `src-tauri/src/commands.rs:22-40` (Settings struct)

**Step 1: Add obsidian_vault_path to Settings**

In `src-tauri/src/commands.rs`, add a new field to the `Settings` struct after `scene_mode`:

```rust
    #[serde(default = "default_scene_mode")]
    pub scene_mode: String, // "general" | "learning" | "working"
    #[serde(default)]
    pub obsidian_vault_path: String,
}
```

Also update `Default for Settings` (in `impl Default for AppData`) — the `#[serde(default)]` with no custom function defaults to empty String, which is correct (disabled state).

**Step 2: Verify it compiles**

Run: `cd src-tauri && cargo check`
Expected: compiles cleanly (serde default handles missing field in existing JSON)

**Step 3: Commit**

```bash
git add src-tauri/src/commands.rs
git commit -m "feat(obsidian): add obsidian_vault_path to Settings"
```

---

### Task 2: Settings field — TypeScript side

**Files:**
- Modify: `src/types/index.ts:32-44` (Settings interface)

**Step 1: Add obsidian_vault_path to Settings interface**

```typescript
export interface Settings {
  ai_mode: string;
  api_key: string;
  model: string;
  shortcut_toggle: string;
  shortcut_analyze: string;
  analysis_cooldown_secs: number;
  bubble_opacity: number;
  language: string;
  auto_analyze: boolean;
  analysis_depth: "casual" | "normal" | "deep";
  scene_mode: "general" | "learning" | "working";
  obsidian_vault_path: string;
}
```

**Step 2: Verify it compiles**

Run: `npx tsc --noEmit`
Expected: passes (existing code doesn't reference the new field yet)

**Step 3: Commit**

```bash
git add src/types/index.ts
git commit -m "feat(obsidian): add obsidian_vault_path to TS Settings type"
```

---

### Task 3: obsidian.rs — vault validation and detection

**Files:**
- Create: `src-tauri/src/obsidian.rs`
- Modify: `src-tauri/src/lib.rs:1-10` (add `mod obsidian;`)

**Step 1: Create obsidian.rs with validation and detection**

Create `src-tauri/src/obsidian.rs`:

```rust
use std::fs;
use std::path::{Path, PathBuf};

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
        // Check if root itself is a vault
        if root.join(".obsidian").is_dir() {
            vaults.push(root.to_string_lossy().to_string());
            continue;
        }
        // Check immediate subdirectories (don't recurse deeply)
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
```

**Step 2: Add mod declaration in lib.rs**

In `src-tauri/src/lib.rs`, after `mod window_monitor;` (line 10), add:

```rust
mod obsidian;
```

**Step 3: Verify it compiles**

Run: `cd src-tauri && cargo check`
Expected: compiles (obsidian module not yet used, but that's fine — Rust warns but doesn't error)

**Step 4: Commit**

```bash
git add src-tauri/src/obsidian.rs src-tauri/src/lib.rs
git commit -m "feat(obsidian): add obsidian.rs with vault detection and validation"
```

---

### Task 4: Tauri commands for vault detection

**Files:**
- Modify: `src-tauri/src/commands.rs` (add two commands)
- Modify: `src-tauri/src/lib.rs:225-244` (register commands)

**Step 1: Add detect and validate commands to commands.rs**

Add at the end of `src-tauri/src/commands.rs` (before the closing, after `trigger_reflection`):

```rust
#[tauri::command]
pub fn detect_obsidian_vaults() -> Vec<String> {
    crate::obsidian::detect_vaults()
}

#[tauri::command]
pub fn validate_vault_path(path: String) -> bool {
    crate::obsidian::is_valid_vault(&path)
}
```

**Step 2: Register commands in lib.rs**

In `src-tauri/src/lib.rs`, add to the `invoke_handler` list (after `get_latest_reflection`):

```rust
            commands::detect_obsidian_vaults,
            commands::validate_vault_path,
```

**Step 3: Verify it compiles**

Run: `cd src-tauri && cargo check`
Expected: compiles cleanly

**Step 4: Commit**

```bash
git add src-tauri/src/commands.rs src-tauri/src/lib.rs
git commit -m "feat(obsidian): add detect_obsidian_vaults and validate_vault_path commands"
```

---

### Task 5: Settings UI — Obsidian section

**Files:**
- Modify: `src/components/SettingsPanel.tsx` (add Obsidian UI after Opacity setting)

**Step 1: Add Obsidian settings section**

In `src/components/SettingsPanel.tsx`, after the Opacity `</SettingRow>` (around line 335) and before `</div>` that closes the settings tab content, add:

```tsx
          {/* Obsidian */}
          <SettingRow label="Obsidian">
            <div className="flex items-center gap-1.5">
              <span
                className={`w-1.5 h-1.5 rounded-full flex-shrink-0 ${
                  settings.obsidian_vault_path
                    ? "bg-green-500"
                    : "bg-zinc-600"
                }`}
              />
              <input
                type="text"
                value={settings.obsidian_vault_path}
                onChange={(e) =>
                  update({ obsidian_vault_path: e.target.value })
                }
                placeholder="Vault path (empty = disabled)"
                className="flex-1 bg-zinc-800 border border-zinc-700 rounded px-2 py-1
                  text-[10px] text-white placeholder-zinc-600 outline-none
                  focus:border-blue-500/50"
              />
              <button
                onClick={async () => {
                  const vaults = await invoke<string[]>(
                    "detect_obsidian_vaults"
                  );
                  if (vaults.length > 0) {
                    update({ obsidian_vault_path: vaults[0] });
                  }
                }}
                className="px-1.5 py-1 text-[9px] bg-zinc-800 text-zinc-400
                  hover:bg-zinc-700 hover:text-zinc-200 rounded transition-colors"
                title="Auto-detect vault"
              >
                Detect
              </button>
            </div>
            <p className="text-[9px] text-zinc-600 mt-1">
              {settings.obsidian_vault_path
                ? "Reflections sync to vault/YoYo/"
                : "Set path to enable Obsidian sync"}
            </p>
          </SettingRow>
```

Make sure `invoke` is imported at the top of the file (check existing imports — it's already imported).

**Step 2: Verify it compiles**

Run: `npx tsc --noEmit`
Expected: passes

**Step 3: Commit**

```bash
git add src/components/SettingsPanel.tsx
git commit -m "feat(obsidian): add Obsidian vault config UI to SettingsPanel"
```

---

### Task 6: obsidian.rs — reflection sync (write to vault)

**Files:**
- Modify: `src-tauri/src/obsidian.rs` (add sync_reflection function)

**Step 1: Add reflection sync function**

Append to `src-tauri/src/obsidian.rs`:

```rust
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
        // Append to existing file
        let mut content = fs::read_to_string(&file_path)
            .map_err(|e| format!("Failed to read {}: {}", file_path.display(), e))?;
        content.push_str(&section);
        fs::write(&file_path, content)
            .map_err(|e| format!("Failed to write {}: {}", file_path.display(), e))?;
    } else {
        // Create new file with header
        let content = format!("# {}\n{}", date_str, section);
        fs::write(&file_path, content)
            .map_err(|e| format!("Failed to write {}: {}", file_path.display(), e))?;
    }

    // Try to append to Obsidian daily note
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

    // Convert Obsidian date format to chrono format
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

    let mut content = fs::read_to_string(&daily_path)
        .map_err(|e| format!("Failed to read daily note: {}", e))?;

    // Don't append if we already added a YoYo section today
    if content.contains("## YoYo") {
        return Ok(());
    }

    // Truncate summary for daily note (first 200 chars)
    let brief = if summary.len() > 200 {
        format!("{}...", &summary[..summary.floor_char_boundary(200)])
    } else {
        summary.to_string()
    };

    content.push_str(&format!("\n\n## YoYo\n{}\n", brief));
    fs::write(&daily_path, content)
        .map_err(|e| format!("Failed to write daily note: {}", e))?;

    Ok(())
}
```

**Step 2: Verify it compiles**

Run: `cd src-tauri && cargo check`
Expected: compiles (chrono and serde_json are already dependencies)

Note: `floor_char_boundary` requires Rust 1.73+. If compilation fails on that, replace with a simple byte-safe truncation:
```rust
let brief = if summary.len() > 200 {
    let end = summary.char_indices().take_while(|(i, _)| *i < 200).last().map(|(i, c)| i + c.len_utf8()).unwrap_or(200);
    format!("{}...", &summary[..end])
} else {
    summary.to_string()
};
```

**Step 3: Commit**

```bash
git add src-tauri/src/obsidian.rs
git commit -m "feat(obsidian): add sync_reflection and daily note append"
```

---

### Task 7: Wire reflection sync into trigger_reflection

**Files:**
- Modify: `src-tauri/src/commands.rs:832-865` (trigger_reflection function)

**Step 1: Add obsidian sync call after save_reflection**

In `trigger_reflection()`, after line 862 (`user_data::update_context_with_reflection(...)`), add:

```rust
    // Sync to Obsidian vault if configured
    let vault_path = &data.settings.obsidian_vault_path;
    if !vault_path.is_empty() {
        if let Err(e) = crate::obsidian::sync_reflection(
            vault_path,
            &summary,
            total,
            &period_start,
            &period_end,
        ) {
            eprintln!("Obsidian sync failed (non-fatal): {}", e);
        }
    }
```

Note: `data` is already loaded at line 838. The `total` is `i64` from `get_total_activity_count()`.

**Step 2: Verify it compiles**

Run: `cd src-tauri && cargo check`
Expected: compiles cleanly

**Step 3: Commit**

```bash
git add src-tauri/src/commands.rs
git commit -m "feat(obsidian): wire sync_reflection into trigger_reflection pipeline"
```

---

### Task 8: obsidian.rs — vault search (read from vault)

**Files:**
- Modify: `src-tauri/src/obsidian.rs` (add search_vault function)

**Step 1: Add keyword search function**

Append to `src-tauri/src/obsidian.rs`:

```rust
use std::time::{SystemTime, Duration};
use std::sync::Mutex;

// Simple file list cache to avoid re-walking vault on every analysis
static VAULT_CACHE: std::sync::OnceLock<Mutex<(Vec<PathBuf>, SystemTime)>> = std::sync::OnceLock::new();

const MAX_FILES: usize = 500;
const MAX_FILE_SIZE: u64 = 50_000; // 50KB
const MAX_CONTEXT_CHARS: usize = 1500;
const CACHE_TTL_SECS: u64 = 60;

/// Directories to skip when walking the vault.
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

    // Score each file by keyword matches
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
            let kw_lower = kw.to_lowercase();
            if filename.contains(&kw_lower) {
                score += 2;
            }
        }

        // Content matches worth 1x (only read if filename had some match, or sample all)
        if let Ok(metadata) = fs::metadata(file_path) {
            if metadata.len() > MAX_FILE_SIZE {
                continue;
            }
        }
        if let Ok(content) = fs::read_to_string(file_path) {
            let content_lower = content.to_lowercase();
            for kw in keywords {
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

    // Sort by score descending, take top 3
    scored.sort_by(|a, b| b.1.cmp(&a.1));
    scored.truncate(3);

    // Build context string from top matches
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
                let end = content.char_indices()
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

    // Check if cache is still valid
    if let Ok(elapsed) = guard.1.elapsed() {
        if elapsed < Duration::from_secs(CACHE_TTL_SECS) && !guard.0.is_empty() {
            return guard.0.clone();
        }
    }

    // Re-scan vault
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
```

**Step 2: Verify it compiles**

Run: `cd src-tauri && cargo check`
Expected: compiles cleanly

**Step 3: Commit**

```bash
git add src-tauri/src/obsidian.rs
git commit -m "feat(obsidian): add vault keyword search with file caching"
```

---

### Task 9: Thread obsidian context into AI prompt

**Files:**
- Modify: `src-tauri/src/ai_engine.rs:133-248` (build_full_prompt_with_history)
- Modify: `src-tauri/src/ai_engine.rs:252-265` (analyze_with_cli signature)
- Modify: `src-tauri/src/ai_engine.rs:321-335` (analyze_with_api signature)
- Modify: `src-tauri/src/commands.rs` (do_analyze — pass obsidian context)

**Step 1: Add obsidian_context param to build_full_prompt_with_history**

In `src-tauri/src/ai_engine.rs`, add `obsidian_context: Option<&str>` as the last parameter of `build_full_prompt_with_history`:

```rust
pub fn build_full_prompt_with_history(
    language: &str,
    recent_activities: &[ActivityRecord],
    main_quest: Option<&str>,
    analysis_depth: &str,
    ocr_text: Option<&str>,
    scene_mode: &str,
    is_focus_crop: bool,
    app_name: Option<&str>,
    open_windows: Option<&str>,
    obsidian_context: Option<&str>,  // NEW
) -> String {
```

Then, before the `parts.push(ANALYSIS_PROMPT.to_string());` line (around line 245), inject obsidian context:

```rust
    // Inject relevant Obsidian vault notes
    if let Some(notes) = obsidian_context {
        if !notes.is_empty() {
            parts.push(format!(
                "[Obsidian Notes]\nRelevant notes from the user's knowledge base:\n{}",
                notes
            ));
        }
    }
```

**Step 2: Add obsidian_context to analyze_with_cli and analyze_with_api**

Add `obsidian_context: Option<&str>` as the last parameter to both functions, and pass it through to `build_full_prompt_with_history`.

For `analyze_with_cli`:
```rust
pub async fn analyze_with_cli(
    screenshot_path: &Path,
    model: &str,
    language: &str,
    recent_activities: &[ActivityRecord],
    main_quest: Option<&str>,
    analysis_depth: &str,
    ocr_text: Option<&str>,
    send_image: bool,
    scene_mode: &str,
    is_focus_crop: bool,
    app_name: Option<&str>,
    open_windows: Option<&str>,
    obsidian_context: Option<&str>,  // NEW
) -> Result<AnalysisResult, String> {
    let full_prompt = build_full_prompt_with_history(
        language,
        recent_activities,
        main_quest,
        analysis_depth,
        ocr_text,
        scene_mode,
        is_focus_crop,
        app_name,
        open_windows,
        obsidian_context,  // NEW
    );
```

Same pattern for `analyze_with_api`.

**Step 3: Update all call sites in commands.rs**

In `src-tauri/src/commands.rs`, `do_analyze()` calls `analyze_with_cli` / `analyze_with_api` in 4 places (2 initial + 2 second-round). Add obsidian search before the first call, then pass it to all calls.

Before the first `ai_engine::analyze_with_*` call (around line 347), add:

```rust
    // Search Obsidian vault for relevant notes
    let obsidian_context = if !data.settings.obsidian_vault_path.is_empty() {
        // Build keywords from app name + main quest text
        let mut keywords: Vec<&str> = Vec::new();
        if let Some(name) = app_name_ref {
            keywords.push(name);
        }
        for quest in &main_quests {
            // Split quest text into words as keywords
            keywords.extend(quest.split_whitespace().take(3));
        }
        crate::obsidian::search_vault(&data.settings.obsidian_vault_path, &keywords)
    } else {
        None
    };
```

Then pass `obsidian_context.as_deref()` as the last argument to all 4 `analyze_with_*` calls.

**Step 4: Verify it compiles**

Run: `cd src-tauri && cargo check`
Expected: compiles cleanly

**Step 5: Commit**

```bash
git add src-tauri/src/ai_engine.rs src-tauri/src/commands.rs
git commit -m "feat(obsidian): inject vault notes as AI context during analysis"
```

---

### Task 10: Verify end-to-end + final commit

**Step 1: Full Rust check**

Run: `cd src-tauri && cargo check`
Expected: compiles cleanly, no warnings about unused code

**Step 2: Full TypeScript check**

Run: `npx tsc --noEmit`
Expected: passes

**Step 3: Test vault detection manually**

Run: `cd src-tauri && cargo test` (if there are existing tests)
If no tests exist, that's fine — this is file I/O that needs manual testing.

**Step 4: Verify git status is clean**

Run: `git status`
Expected: all changes committed

**Step 5: Final summary commit (if any cleanup needed)**

If there are any remaining uncommitted changes:
```bash
git add -A
git commit -m "feat(obsidian): complete integration — vault sync + context search"
```
