# Obsidian Integration Design

## Overview

Add optional Obsidian vault integration to YoYo. Obsidian is a **sync channel**, not a dependency — YoYo's core storage (`~/.yoyo/`) remains the source of truth. When enabled, YoYo mirrors reflection data to the vault and reads vault notes as additional AI context.

## Core Principle

- Obsidian OFF → everything works exactly as before
- Obsidian ON → reflections sync to vault, vault notes enrich AI analysis
- Pure file system approach — no Obsidian plugins required, no Obsidian process dependency

## Settings

New field in `Settings` struct:

```rust
#[serde(default)]
pub obsidian_vault_path: String  // empty = disabled
```

### Vault Discovery

1. Auto-scan common locations (`~/Documents`, `~/Library/Mobile Documents/iCloud~md~obsidian`, `~/Obsidian`) for directories containing `.obsidian/`
2. Present found vaults to user for confirmation
3. User can also manually input/browse for vault path
4. Validate by checking `.obsidian/` subdirectory exists

### Settings UI

Add "Obsidian" section to SettingsPanel:
- Path input + Browse button (Tauri file dialog)
- Green/gray status dot (valid vault / not configured)
- "Detect" button to trigger auto-scan

## Write: Reflection → Vault

### Trigger

Only on `trigger_reflection()` — when 30+ new activities accumulate and AI generates a reflection summary. No high-frequency writes.

### File Structure

```
{vault}/
  YoYo/
    2026-03-07.md
    2026-03-08.md
    ...
```

### File Format

```markdown
# 2026-03-07

## Reflection (14:30)
{reflection.summary}

- Activities: {activity_count}
- Period: {period_start} ~ {period_end}

---
```

If the day's file already exists, append a new `## Reflection (HH:MM)` section (don't overwrite).

### Daily Note Integration

1. Read `.obsidian/daily-notes.json` to detect daily note folder and date format
2. If daily note for today exists, append a `## YoYo` section with brief summary
3. If daily note doesn't exist or config not found, skip silently (no error)

## Read: Vault → AI Context

### Trigger

During `do_analyze()` when building the prompt, if `obsidian_vault_path` is non-empty.

### Search Strategy

1. Extract keywords from: current app name, current analysis context (from last analysis), active main quest text
2. Walk vault `.md` files (skip `.obsidian/`, `.trash/`, `node_modules/`, `YoYo/`)
3. Score files by: filename keyword match (weight 2x) + content keyword match (weight 1x)
4. Take top 1-3 matching files
5. Extract first 500 chars from each matched file
6. Inject as `[Obsidian Notes]` section in prompt

### Performance Constraints

- Max 500 files scanned per search
- Skip files > 50KB (likely not user notes)
- Cache file list for 60 seconds (don't re-walk on every analysis)
- Total injected text capped at 1500 chars

## Data Flow

```
do_analyze() completes
  → record_activity()              // unchanged, writes SQLite
  → activities >= 30?
    → trigger_reflection()
      → save_reflection()          // unchanged: SQLite + context.md
      → sync_to_obsidian()         // NEW: write vault/YoYo/YYYY-MM-DD.md
                                   //      append to daily note if exists

do_analyze() builds prompt
  → read_profile()                 // unchanged
  → read_context()                 // unchanged
  → search_obsidian_vault()        // NEW: grep-match relevant notes
  → build_full_prompt()            // inject [Obsidian Notes] section
```

## New Code

| Location | What |
|----------|------|
| `src-tauri/src/obsidian.rs` | New module: vault validation, file scanning, keyword search, reflection write, daily note detection |
| `src-tauri/src/commands.rs` | New commands: `detect_obsidian_vaults`, `validate_vault_path`; modify `trigger_reflection` to call `sync_to_obsidian` |
| `src-tauri/src/ai_engine.rs` | `build_full_prompt_with_history()` gains `obsidian_context: Option<&str>` parameter |
| `src-tauri/src/lib.rs` | Register new commands |
| `src/types/index.ts` | `Settings.obsidian_vault_path: string` |
| `src/components/SettingsPanel.tsx` | Obsidian config UI section |
| `src/services/storage.ts` | No changes needed (Settings already serialized generically) |

## Error Handling

- Vault path invalid/deleted → log warning, skip sync, don't error the analysis
- File write fails (permissions) → log warning, continue
- No matching notes found → skip `[Obsidian Notes]` section silently
- Daily note config not found → skip daily note integration silently

## Security

- Only read `.md` files from vault (no arbitrary file access)
- Vault path must contain `.obsidian/` subdirectory (validation)
- No execution of vault content — only read as text context
