# Session-Driven YoYo Redesign — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Transform YoYo from passive screen monitoring into an interactive, session-based workflow assistant with speech bubbles.

**Architecture:** Add `sessions` + `session_timeline` SQLite tables. Add `active_session` to `AppState`. Session context is injected into the existing AI prompt pipeline. TrayApp becomes idle/session toggle. BubbleApp becomes session dashboard. New `speech-bubble` Tauri window for AI messages.

**Tech Stack:** Tauri v2 (Rust), React, TypeScript, Tailwind CSS, SQLite

**Design Doc:** `docs/plans/2026-03-08-session-mode-design.md`

---

### Task 1: SQLite Tables + Rust Data Structs

Add `sessions` and `session_timeline` tables, plus Rust structs for the session data model.

**Files:**
- Modify: `src-tauri/src/user_data.rs:110-159` (init_tables)
- Modify: `src-tauri/src/user_data.rs` (add new structs + CRUD functions)

**Step 1: Add Session structs to user_data.rs**

Add after the existing `ReflectionRecord` struct (~line 194):

```rust
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Session {
    pub id: String,
    pub goal: String,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub summary: Option<String>,
    pub status: String, // "active" | "completed" | "abandoned"
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TimelineEntry {
    pub id: i64,
    pub session_id: String,
    pub timestamp: String,
    pub context: String,
    pub app_name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SessionSummary {
    pub session: Session,
    pub timeline: Vec<TimelineEntry>,
}
```

**Step 2: Add CREATE TABLE statements to init_tables()**

Append inside the `execute_batch` string at `user_data.rs:111-156`, before the closing `"`:

```sql
CREATE TABLE IF NOT EXISTS sessions (
    id         TEXT PRIMARY KEY,
    goal       TEXT NOT NULL,
    started_at TEXT NOT NULL DEFAULT (datetime('now','localtime')),
    ended_at   TEXT,
    summary    TEXT,
    status     TEXT DEFAULT 'active'
);

CREATE TABLE IF NOT EXISTS session_timeline (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL,
    timestamp  TEXT NOT NULL DEFAULT (datetime('now','localtime')),
    context    TEXT NOT NULL,
    app_name   TEXT NOT NULL DEFAULT '',
    FOREIGN KEY (session_id) REFERENCES sessions(id)
);
```

**Step 3: Add session CRUD functions to user_data.rs**

```rust
/// Create a new session. Returns the Session struct.
pub fn create_session(goal: &str) -> Result<Session, String> {
    let conn = get_db()?;
    let id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO sessions (id, goal) VALUES (?1, ?2)",
        rusqlite::params![id, goal],
    ).map_err(|e| format!("Failed to create session: {}", e))?;

    Ok(Session {
        id,
        goal: goal.to_string(),
        started_at: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        ended_at: None,
        summary: None,
        status: "active".to_string(),
    })
}

/// End the active session — set status + ended_at + summary.
pub fn end_session(session_id: &str, summary: &str, status: &str) -> Result<(), String> {
    let conn = get_db()?;
    conn.execute(
        "UPDATE sessions SET ended_at = datetime('now','localtime'), summary = ?1, status = ?2 WHERE id = ?3",
        rusqlite::params![summary, status, session_id],
    ).map_err(|e| format!("Failed to end session: {}", e))?;
    Ok(())
}

/// Get a session by ID.
pub fn get_session(session_id: &str) -> Result<Option<Session>, String> {
    let conn = get_db()?;
    let mut stmt = conn.prepare(
        "SELECT id, goal, started_at, ended_at, summary, status FROM sessions WHERE id = ?1"
    ).map_err(|e| format!("Failed to prepare query: {}", e))?;

    let mut rows = stmt.query_map(rusqlite::params![session_id], |row| {
        Ok(Session {
            id: row.get(0)?,
            goal: row.get(1)?,
            started_at: row.get(2)?,
            ended_at: row.get(3)?,
            summary: row.get(4)?,
            status: row.get(5)?,
        })
    }).map_err(|e| format!("Failed to query session: {}", e))?;

    match rows.next() {
        Some(Ok(s)) => Ok(Some(s)),
        _ => Ok(None),
    }
}

/// Get recent completed sessions for the idle view.
pub fn get_session_history(limit: u32) -> Result<Vec<Session>, String> {
    let conn = get_db()?;
    let mut stmt = conn.prepare(
        "SELECT id, goal, started_at, ended_at, summary, status FROM sessions ORDER BY started_at DESC LIMIT ?1"
    ).map_err(|e| format!("Failed to prepare query: {}", e))?;

    let sessions = stmt.query_map(rusqlite::params![limit], |row| {
        Ok(Session {
            id: row.get(0)?,
            goal: row.get(1)?,
            started_at: row.get(2)?,
            ended_at: row.get(3)?,
            summary: row.get(4)?,
            status: row.get(5)?,
        })
    }).map_err(|e| format!("Failed to query sessions: {}", e))?
    .filter_map(|r| r.ok())
    .collect();

    Ok(sessions)
}

/// Add a timeline entry to a session.
pub fn add_timeline_entry(session_id: &str, context: &str, app_name: &str) -> Result<TimelineEntry, String> {
    let conn = get_db()?;
    conn.execute(
        "INSERT INTO session_timeline (session_id, context, app_name) VALUES (?1, ?2, ?3)",
        rusqlite::params![session_id, context, app_name],
    ).map_err(|e| format!("Failed to add timeline entry: {}", e))?;

    let id = conn.last_insert_rowid();
    Ok(TimelineEntry {
        id,
        session_id: session_id.to_string(),
        timestamp: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        context: context.to_string(),
        app_name: app_name.to_string(),
    })
}

/// Get all timeline entries for a session.
pub fn get_session_timeline(session_id: &str) -> Result<Vec<TimelineEntry>, String> {
    let conn = get_db()?;
    let mut stmt = conn.prepare(
        "SELECT id, session_id, timestamp, context, app_name FROM session_timeline WHERE session_id = ?1 ORDER BY timestamp ASC"
    ).map_err(|e| format!("Failed to prepare query: {}", e))?;

    let entries = stmt.query_map(rusqlite::params![session_id], |row| {
        Ok(TimelineEntry {
            id: row.get(0)?,
            session_id: row.get(1)?,
            timestamp: row.get(2)?,
            context: row.get(3)?,
            app_name: row.get(4)?,
        })
    }).map_err(|e| format!("Failed to query timeline: {}", e))?
    .filter_map(|r| r.ok())
    .collect();

    Ok(entries)
}
```

**Step 4: Add `uuid` dependency to Cargo.toml**

Run: `cd src-tauri && cargo add uuid --features v4`

**Step 5: Verify compilation**

Run: `cd src-tauri && cargo check`
Expected: Success with no errors

**Step 6: Commit**

```
feat: session data model — SQLite tables + CRUD functions
```

---

### Task 2: AppState + Session Tauri Commands

Add `active_session` to `AppState` and create Tauri commands for session management.

**Files:**
- Modify: `src-tauri/src/lib.rs:25-56` (AppState struct + init)
- Modify: `src-tauri/src/commands.rs` (new Tauri commands)
- Modify: `src-tauri/src/lib.rs:226-247` (register commands)

**Step 1: Add active_session to AppState**

In `lib.rs:25-38`, add field to `AppState`:

```rust
pub struct AppState {
    pub last_analysis: Mutex<Option<AnalysisResult>>,
    pub debounce_counter: AtomicI64,
    pub last_analysis_time: AtomicI64,
    pub current_app_name: Mutex<String>,
    pub current_bundle_id: Mutex<String>,
    pub current_app_pid: AtomicI64,
    pub activities_since_reflection: AtomicI64,
    pub onboarding_active: Mutex<bool>,
    pub onboarding_history: Mutex<Vec<ChatMessage>>,
    // Session mode
    pub active_session: Mutex<Option<user_data::Session>>,
}
```

In `lib.rs:46-56`, add init value:

```rust
active_session: Mutex::new(None),
```

**Step 2: Add session Tauri commands in commands.rs**

```rust
#[tauri::command]
pub async fn start_session(app: AppHandle, goal: String) -> Result<user_data::Session, String> {
    // End any existing active session first
    {
        let state = app.state::<AppState>();
        if let Ok(mut active) = state.active_session.lock() {
            if let Some(ref session) = *active {
                let _ = user_data::end_session(&session.id, "Auto-ended by new session", "abandoned");
            }
            *active = None;
        }
    }

    let session = user_data::create_session(&goal)?;
    let state = app.state::<AppState>();
    if let Ok(mut active) = state.active_session.lock() {
        *active = Some(session.clone());
    }
    let _ = app.emit("session-started", &session);
    Ok(session)
}

#[tauri::command]
pub async fn end_session(app: AppHandle) -> Result<user_data::SessionSummary, String> {
    let state = app.state::<AppState>();
    let session = {
        let mut active = state.active_session.lock().map_err(|e| e.to_string())?;
        active.take().ok_or("No active session")?
    };

    let timeline = user_data::get_session_timeline(&session.id)?;

    // Generate summary via AI
    let data = load_data(&app);
    let summary_text = generate_session_summary(&app, &session, &timeline, &data).await?;

    user_data::end_session(&session.id, &summary_text, "completed")?;

    // Sync to Obsidian if enabled
    if data.settings.obsidian_enabled && !data.settings.obsidian_vault_path.is_empty() {
        if let Err(e) = crate::obsidian::sync_reflection(
            &data.settings.obsidian_vault_path,
            &summary_text,
            timeline.len() as i64,
            &session.started_at,
            &chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        ) {
            eprintln!("Obsidian session sync failed (non-fatal): {}", e);
        }
    }

    let ended_session = user_data::get_session(&session.id)?
        .unwrap_or(session);

    let summary = user_data::SessionSummary {
        session: ended_session,
        timeline,
    };
    let _ = app.emit("session-ended", &summary);
    Ok(summary)
}

#[tauri::command]
pub fn get_active_session(app: AppHandle) -> Result<Option<user_data::Session>, String> {
    let state = app.state::<AppState>();
    let active = state.active_session.lock().map_err(|e| e.to_string())?;
    Ok(active.clone())
}

#[tauri::command]
pub fn get_session_history(limit: u32) -> Result<Vec<user_data::Session>, String> {
    user_data::get_session_history(limit)
}

#[tauri::command]
pub fn get_session_timeline(session_id: String) -> Result<Vec<user_data::TimelineEntry>, String> {
    user_data::get_session_timeline(&session_id)
}

#[tauri::command]
pub async fn send_session_message(app: AppHandle, message: String) -> Result<String, String> {
    let state = app.state::<AppState>();
    let session = {
        let active = state.active_session.lock().map_err(|e| e.to_string())?;
        active.clone().ok_or("No active session")?
    };

    let data = load_data(&app);
    let timeline = user_data::get_session_timeline(&session.id)?;

    // Build a chat prompt with session context
    let response = generate_session_chat(&app, &session, &timeline, &message, &data).await?;

    // Show response as speech bubble
    let _ = app.emit("speech-bubble", serde_json::json!({
        "text": response,
        "auto_dismiss_secs": 12
    }));

    Ok(response)
}
```

**Step 3: Add helper functions for AI summary/chat generation**

```rust
async fn generate_session_summary(
    app: &AppHandle,
    session: &user_data::Session,
    timeline: &[user_data::TimelineEntry],
    data: &AppData,
) -> Result<String, String> {
    let timeline_text: String = timeline.iter().map(|e| {
        format!("- {} {} ({})", e.timestamp, e.context, e.app_name)
    }).collect::<Vec<_>>().join("\n");

    let prompt = format!(
        "Summarize this work session in 2-3 sentences. Include: what was accomplished, any blockers, and suggested next steps.\n\nGoal: {}\nDuration: {} → now\nTimeline:\n{}\n\nRespond in {}. Plain text only, no JSON.",
        session.goal, session.started_at, timeline_text,
        if data.settings.language == "zh" { "Chinese" } else { "English" }
    );

    if data.settings.ai_mode == "api" && !data.settings.api_key.is_empty() {
        ai_engine::simple_chat_api(&prompt, &data.settings.api_key, &data.settings.model).await
    } else {
        ai_engine::simple_chat_cli(&prompt, &data.settings.model).await
    }
}

async fn generate_session_chat(
    app: &AppHandle,
    session: &user_data::Session,
    timeline: &[user_data::TimelineEntry],
    user_message: &str,
    data: &AppData,
) -> Result<String, String> {
    let timeline_text: String = timeline.iter().rev().take(10).map(|e| {
        format!("- {} {} ({})", e.timestamp, e.context, e.app_name)
    }).collect::<Vec<_>>().join("\n");

    let prompt = format!(
        "You are YoYo, a workflow assistant. The user is in a session with goal: \"{}\"\nRecent timeline:\n{}\n\nUser says: {}\n\nRespond in 1-3 sentences, concise and actionable. Respond in {}.",
        session.goal, timeline_text, user_message,
        if data.settings.language == "zh" { "Chinese" } else { "English" }
    );

    if data.settings.ai_mode == "api" && !data.settings.api_key.is_empty() {
        ai_engine::simple_chat_api(&prompt, &data.settings.api_key, &data.settings.model).await
    } else {
        ai_engine::simple_chat_cli(&prompt, &data.settings.model).await
    }
}
```

**Step 4: Add `simple_chat_cli` and `simple_chat_api` to ai_engine.rs**

These are lightweight text-only AI calls (no image) for session summary and chat:

```rust
/// Simple text-only chat via CLI (no image).
pub async fn simple_chat_cli(prompt: &str, model: &str) -> Result<String, String> {
    let output = tokio::process::Command::new("claude")
        .args(["-p", prompt, "--model", model, "--no-input"])
        .output()
        .await
        .map_err(|e| format!("Claude CLI failed: {}", e))?;

    if !output.status.success() {
        return Err(format!("Claude CLI error: {}", String::from_utf8_lossy(&output.stderr)));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Simple text-only chat via API (no image).
pub async fn simple_chat_api(prompt: &str, api_key: &str, model: &str) -> Result<String, String> {
    let client = reqwest::Client::new();
    let body = serde_json::json!({
        "model": model,
        "max_tokens": 300,
        "messages": [{"role": "user", "content": prompt}]
    });

    let resp = client.post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("API request failed: {}", e))?;

    let json: serde_json::Value = resp.json().await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    json["content"][0]["text"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "No text in API response".to_string())
}
```

**Step 5: Register new commands in lib.rs**

At `lib.rs:226-247`, add to `invoke_handler`:

```rust
commands::start_session,
commands::end_session,
commands::get_active_session,
commands::get_session_history,
commands::get_session_timeline,
commands::send_session_message,
```

**Step 6: Verify compilation**

Run: `cd src-tauri && cargo check`
Expected: Success

**Step 7: Commit**

```
feat: session management — AppState, Tauri commands, AI summary/chat
```

---

### Task 3: Session-Aware Analysis Pipeline

Inject session context into AI prompts and add `on_track`/`drift_message` to analysis results.

**Files:**
- Modify: `src-tauri/src/ai_engine.rs:30-39` (AnalysisResult)
- Modify: `src-tauri/src/ai_engine.rs:41-66` (ANALYSIS_PROMPT)
- Modify: `src-tauri/src/ai_engine.rs:133-144` (build_full_prompt_with_history signature)
- Modify: `src-tauri/src/commands.rs:207-469` (do_analyze — inject session + emit timeline)
- Modify: `src-tauri/src/lib.rs:150-161` (post-analysis — emit drift event)

**Step 1: Add session fields to AnalysisResult**

In `ai_engine.rs:30-39`:

```rust
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AnalysisResult {
    pub context: String,
    pub actions: Vec<SuggestedAction>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suggested_quest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key_concepts: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub need_full_context: Option<bool>,
    // Session-aware fields
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_track: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub drift_message: Option<String>,
}
```

**Step 2: Add session context parameter to build_full_prompt_with_history**

In `ai_engine.rs:133-144`, add a new parameter:

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
    obsidian_context: Option<&str>,
    session_context: Option<&str>,  // NEW
) -> String
```

Inside the function body, add session context injection before the main prompt (before the `ANALYSIS_PROMPT` append). Insert after the Obsidian context block:

```rust
// Session context injection
if let Some(session_ctx) = session_context {
    prompt.push_str(&format!("\n{}\n", session_ctx));
}
```

**Step 3: Update ANALYSIS_PROMPT for session-aware JSON output**

Append to the JSON example in `ANALYSIS_PROMPT` (at `ai_engine.rs:54-63`):

```
The "on_track" field is OPTIONAL. Only include when an [Active Session] section is present. Set to true if the user's current activity relates to the session goal, false if they appear to be drifting.
The "drift_message" field is OPTIONAL. Only include when on_track is false. Write a brief, friendly nudge (1 sentence) reminding the user of their goal.
```

And add to the JSON example:

```json
"on_track": true,
"drift_message": null
```

**Step 4: Update all call sites of build_full_prompt_with_history**

There are 3 call sites in `commands.rs` (lines ~370, ~417, ~435) and the matching calls in `analyze_with_cli`/`analyze_with_api`. Each needs the new `session_context` parameter.

In `do_analyze()`, build the session context string before the AI call:

```rust
// Build session context for prompt injection
let session_context = {
    let state_ref = app.try_state::<AppState>();
    if let Some(state) = state_ref {
        if let Ok(active) = state.active_session.lock() {
            if let Some(ref session) = *active {
                let timeline = user_data::get_session_timeline(&session.id).unwrap_or_default();
                let elapsed = {
                    if let Ok(start) = chrono::NaiveDateTime::parse_from_str(&session.started_at, "%Y-%m-%d %H:%M:%S") {
                        let now = chrono::Local::now().naive_local();
                        let dur = now - start;
                        let hours = dur.num_hours();
                        let mins = dur.num_minutes() % 60;
                        if hours > 0 { format!("{}h{}m", hours, mins) } else { format!("{}m", mins) }
                    } else {
                        "?".to_string()
                    }
                };
                let recent_timeline: String = timeline.iter().rev().take(10).map(|e| {
                    format!("- {} {} ({})", e.timestamp, e.context, e.app_name)
                }).collect::<Vec<_>>().join("\n");

                Some(format!(
                    "[Active Session]\nGoal: {}\nDuration: {}\nRecent timeline:\n{}\n\nAnalyze the user's current screen in the context of this goal.\n- Is the user on-track or drifting from the goal?\n- What specific suggestion would help them progress?",
                    session.goal, elapsed, recent_timeline
                ))
            } else { None }
        } else { None }
    } else { None }
};
```

Then pass `session_context.as_deref()` to every `analyze_with_api`/`analyze_with_cli` call, and thread it through to `build_full_prompt_with_history`.

**Step 5: After analysis, record timeline entry + handle drift**

In `lib.rs`, inside the `app-switched` handler's success block (after line ~161 where `analysis-complete` is emitted), add:

```rust
// Record timeline entry if session is active
{
    let active = state.active_session.lock().ok()
        .and_then(|a| a.as_ref().map(|s| s.id.clone()));
    if let Some(session_id) = active {
        let _ = user_data::add_timeline_entry(
            &session_id,
            &result.context,
            &app_name,
        );
        let _ = app.emit("session-timeline-update", serde_json::json!({
            "session_id": session_id,
            "context": result.context,
            "app_name": app_name,
        }));

        // Drift detection
        if result.on_track == Some(false) {
            if let Some(ref msg) = result.drift_message {
                let _ = app.emit("session-drift", serde_json::json!({
                    "message": msg
                }));
                let _ = app.emit("speech-bubble", serde_json::json!({
                    "text": msg,
                    "auto_dismiss_secs": 10
                }));
            }
        }
    }
}
```

**Step 6: Verify compilation**

Run: `cd src-tauri && cargo check`
Expected: Success

**Step 7: Commit**

```
feat: session-aware analysis — prompt injection, on_track/drift detection
```

---

### Task 4: TypeScript Types + Service Layer

Add session-related TypeScript types and Tauri invoke wrappers.

**Files:**
- Modify: `src/types/index.ts`
- Create: `src/services/sessions.ts`

**Step 1: Update types/index.ts**

Add session types and update `AnalysisResult`:

```typescript
// Add to AnalysisResult
export interface AnalysisResult {
  context: string;
  actions: SuggestedAction[];
  suggested_quest?: string;
  key_concepts?: string[];
  need_full_context?: boolean;
  on_track?: boolean;        // NEW
  drift_message?: string;    // NEW
}

// New session types
export interface Session {
  id: string;
  goal: string;
  started_at: string;
  ended_at?: string;
  summary?: string;
  status: string;
}

export interface TimelineEntry {
  id: number;
  session_id: string;
  timestamp: string;
  context: string;
  app_name: string;
}

export interface SessionSummary {
  session: Session;
  timeline: TimelineEntry[];
}

export interface SpeechBubbleEvent {
  text: string;
  auto_dismiss_secs: number;
}

export interface SessionDriftEvent {
  message: string;
}
```

**Step 2: Create src/services/sessions.ts**

```typescript
import { invoke } from "@tauri-apps/api/core";
import type { Session, SessionSummary, TimelineEntry } from "../types";

export async function startSession(goal: string): Promise<Session> {
  return await invoke<Session>("start_session", { goal });
}

export async function endSession(): Promise<SessionSummary> {
  return await invoke<SessionSummary>("end_session");
}

export async function getActiveSession(): Promise<Session | null> {
  return await invoke<Session | null>("get_active_session");
}

export async function getSessionHistory(limit: number = 10): Promise<Session[]> {
  return await invoke<Session[]>("get_session_history", { limit });
}

export async function getSessionTimeline(sessionId: string): Promise<TimelineEntry[]> {
  return await invoke<TimelineEntry[]>("get_session_timeline", { sessionId });
}

export async function sendSessionMessage(message: string): Promise<string> {
  return await invoke<string>("send_session_message", { message });
}
```

**Step 3: Verify TypeScript compiles**

Run: `cd /Users/john/workspace/john/YoYo && npm run build`
Expected: No type errors

**Step 4: Commit**

```
feat: session TypeScript types + service layer
```

---

### Task 5: TrayApp Redesign — Idle + Session Views

Replace the current TrayApp with a two-state UI: idle (session history + input) and in-session (timer + timeline + input).

**Files:**
- Modify: `src/TrayApp.tsx` (major rewrite)
- Modify: `src/components/QuestBoard.tsx` (will be replaced/adapted)

**Step 1: Rewrite TrayApp.tsx**

Replace the existing TrayApp with two views. Keep keyboard shortcuts and `analysis-complete` listener intact.

```tsx
import { useState, useEffect, useRef } from "react";
import { listen } from "@tauri-apps/api/event";
import type { Session, TimelineEntry, SessionSummary, SpeechBubbleEvent, AnalysisResult } from "./types";
import { startSession, endSession, getActiveSession, getSessionHistory, getSessionTimeline } from "./services/sessions";
import StatusIndicator from "./components/StatusIndicator";
import SettingsPanel from "./components/SettingsPanel";

export default function TrayApp() {
  const [session, setSession] = useState<Session | null>(null);
  const [timeline, setTimeline] = useState<TimelineEntry[]>([]);
  const [history, setHistory] = useState<Session[]>([]);
  const [input, setInput] = useState("");
  const [showSettings, setShowSettings] = useState(false);
  const [loading, setLoading] = useState(false);
  const [ending, setEnding] = useState(false);
  const timerRef = useRef<ReturnType<typeof setInterval>>();
  const [elapsed, setElapsed] = useState("");

  // Load active session on mount
  useEffect(() => {
    getActiveSession().then(s => {
      setSession(s);
      if (s) {
        getSessionTimeline(s.id).then(setTimeline);
      } else {
        getSessionHistory(5).then(setHistory);
      }
    });
  }, []);

  // Timer tick
  useEffect(() => {
    if (!session) { clearInterval(timerRef.current); return; }
    const tick = () => {
      const start = new Date(session.started_at).getTime();
      const diff = Date.now() - start;
      const h = Math.floor(diff / 3600000);
      const m = Math.floor((diff % 3600000) / 60000);
      setElapsed(h > 0 ? `${h}h${m}m` : `${m}m`);
    };
    tick();
    timerRef.current = setInterval(tick, 10000);
    return () => clearInterval(timerRef.current);
  }, [session]);

  // Listen for timeline updates
  useEffect(() => {
    const unlisten = listen<any>("session-timeline-update", (e) => {
      setTimeline(prev => [...prev, {
        id: Date.now(),
        session_id: e.payload.session_id,
        timestamp: new Date().toLocaleTimeString("zh-CN", { hour: "2-digit", minute: "2-digit" }),
        context: e.payload.context,
        app_name: e.payload.app_name,
      }]);
    });
    return () => { unlisten.then(f => f()); };
  }, []);

  // Listen for session events
  useEffect(() => {
    const u1 = listen<Session>("session-started", (e) => {
      setSession(e.payload);
      setTimeline([]);
    });
    const u2 = listen<SessionSummary>("session-ended", (e) => {
      setSession(null);
      setTimeline([]);
      getSessionHistory(5).then(setHistory);
    });
    return () => { u1.then(f => f()); u2.then(f => f()); };
  }, []);

  const handleStart = async () => {
    if (!input.trim()) return;
    setLoading(true);
    try {
      await startSession(input.trim());
      setInput("");
    } finally {
      setLoading(false);
    }
  };

  const handleEnd = async () => {
    setEnding(true);
    try {
      await endSession();
    } finally {
      setEnding(false);
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleStart();
    }
  };

  if (showSettings) {
    return <SettingsPanel onClose={() => setShowSettings(false)} />;
  }

  // --- IN-SESSION VIEW ---
  if (session) {
    return (
      <div className="h-full flex flex-col bg-gray-950 text-white">
        {/* Header: timer + goal + end button */}
        <div className="flex items-center justify-between px-3 py-2 border-b border-gray-800">
          <div className="flex items-center gap-2">
            <span className="text-green-400 font-mono text-sm">⏱ {elapsed}</span>
          </div>
          <button
            onClick={handleEnd}
            disabled={ending}
            className="text-xs px-2 py-1 rounded bg-red-600 hover:bg-red-500 disabled:opacity-50"
          >
            {ending ? "Ending..." : "End"}
          </button>
        </div>
        <div className="px-3 py-1 text-sm text-gray-300 border-b border-gray-800 truncate">
          {session.goal}
        </div>

        {/* Timeline (scrollable) */}
        <div className="flex-1 overflow-y-auto px-3 py-2 space-y-2 text-xs">
          {timeline.map((entry) => (
            <div key={entry.id} className="flex flex-col">
              <span className="text-gray-500">{entry.timestamp.slice(11, 16)}</span>
              <span className="text-gray-300">{entry.context}</span>
              <span className="text-gray-600 text-[10px]">{entry.app_name}</span>
            </div>
          ))}
          {timeline.length === 0 && (
            <div className="text-gray-600 text-center mt-4">Session started. Activity will appear here...</div>
          )}
        </div>

        {/* Input box */}
        <div className="px-3 py-2 border-t border-gray-800">
          <StatusIndicator />
          {/* Input for chat during session — handled separately */}
        </div>
      </div>
    );
  }

  // --- IDLE VIEW ---
  return (
    <div className="h-full flex flex-col bg-gray-950 text-white">
      <div className="flex items-center justify-between px-3 py-2 border-b border-gray-800">
        <span className="text-sm font-semibold">YoYo</span>
        <button onClick={() => setShowSettings(true)} className="text-gray-400 hover:text-white text-sm">⚙</button>
      </div>

      {/* Session history */}
      <div className="flex-1 overflow-y-auto px-3 py-2">
        <div className="text-xs text-gray-500 mb-2">Recent Sessions</div>
        {history.length === 0 && (
          <div className="text-gray-600 text-xs text-center mt-4">No sessions yet. Type a goal below to start!</div>
        )}
        {history.map((s) => (
          <button
            key={s.id}
            onClick={() => setInput(s.goal)}
            className="w-full text-left px-2 py-1.5 rounded hover:bg-gray-800 transition-colors mb-1"
          >
            <div className="text-sm text-gray-300 truncate">{s.goal}</div>
            <div className="text-[10px] text-gray-600">
              {s.started_at.slice(0, 10)} · {s.status === "completed" ? "✓" : "—"}
              {s.ended_at && s.started_at && (() => {
                const ms = new Date(s.ended_at).getTime() - new Date(s.started_at).getTime();
                const m = Math.floor(ms / 60000);
                return m >= 60 ? ` ${Math.floor(m/60)}h${m%60}m` : ` ${m}m`;
              })()}
            </div>
          </button>
        ))}
      </div>

      {/* Input box */}
      <div className="px-3 py-2 border-t border-gray-800">
        <div className="flex gap-2">
          <input
            type="text"
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="Type a goal to start session..."
            className="flex-1 bg-gray-800 text-white text-sm rounded px-2 py-1.5 outline-none focus:ring-1 focus:ring-blue-500"
          />
          <button
            onClick={handleStart}
            disabled={!input.trim() || loading}
            className="text-sm px-3 py-1.5 rounded bg-blue-600 hover:bg-blue-500 disabled:opacity-40"
          >
            Go
          </button>
        </div>
      </div>
    </div>
  );
}
```

**Step 2: Verify build**

Run: `npm run build`
Expected: Success

**Step 3: Commit**

```
feat: TrayApp redesign — idle view (history + input) + session view (timer + timeline)
```

---

### Task 6: BubbleApp Session Dashboard

Replace the current BubbleApp with a session-focused dashboard that shows context, actions, and session progress.

**Files:**
- Modify: `src/BubbleApp.tsx` (major rewrite)

**Step 1: Rewrite BubbleApp.tsx**

Keep the existing core functionality (listening for `analysis-complete`, showing context + actions) but add session awareness:

- When session is active: show session goal + compact timeline + actions
- When no session: show current behavior (context + actions)

```tsx
import { useState, useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import type { AnalysisResult, Session, TimelineEntry } from "./types";
import { getActiveSession, getSessionTimeline, sendSessionMessage } from "./services/sessions";
import ActionButtons from "./components/ActionButtons";
import StatusIndicator from "./components/StatusIndicator";

export default function BubbleApp() {
  const [result, setResult] = useState<AnalysisResult | null>(null);
  const [session, setSession] = useState<Session | null>(null);
  const [timeline, setTimeline] = useState<TimelineEntry[]>([]);
  const [opacity, setOpacity] = useState(0.85);
  const [visible, setVisible] = useState(false);
  const [chatInput, setChatInput] = useState("");
  const [chatLoading, setChatLoading] = useState(false);

  // Load opacity + session on mount
  useEffect(() => {
    invoke<any>("get_settings").then((s: any) => {
      setOpacity(s.bubble_opacity ?? 0.85);
    });
    getActiveSession().then(s => {
      setSession(s);
      if (s) getSessionTimeline(s.id).then(setTimeline);
    });
    invoke<AnalysisResult | null>("get_last_analysis").then(r => {
      if (r) { setResult(r); setVisible(true); }
    });
  }, []);

  // Listen events
  useEffect(() => {
    const u1 = listen<AnalysisResult>("analysis-complete", (e) => {
      setResult(e.payload);
      setVisible(true);
    });
    const u2 = listen<any>("session-started", (e) => {
      setSession(e.payload);
      setTimeline([]);
    });
    const u3 = listen<any>("session-ended", () => {
      setSession(null);
      setTimeline([]);
    });
    const u4 = listen<any>("session-timeline-update", (e) => {
      setTimeline(prev => [...prev, {
        id: Date.now(),
        session_id: e.payload.session_id,
        timestamp: new Date().toLocaleTimeString("zh-CN", { hour: "2-digit", minute: "2-digit" }),
        context: e.payload.context,
        app_name: e.payload.app_name,
      }]);
    });
    const u5 = listen<number>("bubble-opacity-changed", (e) => setOpacity(e.payload));
    return () => { [u1,u2,u3,u4,u5].forEach(u => u.then(f => f())); };
  }, []);

  const handleChat = async () => {
    if (!chatInput.trim() || chatLoading) return;
    setChatLoading(true);
    try {
      await sendSessionMessage(chatInput.trim());
      setChatInput("");
    } finally {
      setChatLoading(false);
    }
  };

  if (!visible) return null;

  return (
    <div
      className="h-full flex flex-col bg-gray-900 text-white rounded-xl overflow-hidden"
      style={{ opacity }}
    >
      {/* Session header (if active) */}
      {session && (
        <div className="px-3 py-1.5 bg-blue-900/40 border-b border-blue-800/50">
          <div className="text-xs text-blue-300 truncate">{session.goal}</div>
        </div>
      )}

      {/* Context */}
      {result && (
        <div className="px-3 py-2">
          <p className="text-sm text-gray-300">{result.context}</p>
        </div>
      )}

      {/* Session timeline (compact, last 3) */}
      {session && timeline.length > 0 && (
        <div className="px-3 pb-1 space-y-0.5">
          {timeline.slice(-3).map(e => (
            <div key={e.id} className="text-[10px] text-gray-500 truncate">
              {e.timestamp.slice(11,16)} {e.context}
            </div>
          ))}
        </div>
      )}

      {/* Actions */}
      <div className="flex-1 px-3 py-1">
        {result && <ActionButtons actions={result.actions} />}
      </div>

      {/* Chat input (session only) */}
      {session && (
        <div className="px-3 py-1.5 border-t border-gray-800">
          <div className="flex gap-1">
            <input
              type="text"
              value={chatInput}
              onChange={(e) => setChatInput(e.target.value)}
              onKeyDown={(e) => { if (e.key === "Enter") { e.preventDefault(); handleChat(); } }}
              placeholder="Ask YoYo..."
              className="flex-1 bg-gray-800 text-white text-xs rounded px-2 py-1 outline-none"
            />
          </div>
        </div>
      )}

      {/* Status */}
      <div className="px-3 py-1">
        <StatusIndicator />
      </div>
    </div>
  );
}
```

**Step 2: Verify build**

Run: `npm run build`
Expected: Success

**Step 3: Commit**

```
feat: BubbleApp session dashboard — goal, timeline, chat input
```

---

### Task 7: Speech Bubble Window

Create a new transparent Tauri webview window for AI speech bubbles that auto-fade.

**Files:**
- Modify: `src/main.tsx:8-12` (add speech-bubble label routing)
- Create: `src/SpeechBubble.tsx`
- Modify: `src-tauri/src/lib.rs` (add show_speech_bubble function)

**Step 1: Add SpeechBubble component**

```tsx
// src/SpeechBubble.tsx
import { useState, useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";

export default function SpeechBubble() {
  const [text, setText] = useState("");
  const [visible, setVisible] = useState(false);
  const [fading, setFading] = useState(false);

  useEffect(() => {
    const unlisten = listen<{ text: string; auto_dismiss_secs: number }>("speech-bubble", (e) => {
      setText(e.payload.text);
      setVisible(true);
      setFading(false);

      // Auto-dismiss
      const fadeTimer = setTimeout(() => setFading(true), (e.payload.auto_dismiss_secs - 1) * 1000);
      const hideTimer = setTimeout(() => {
        setVisible(false);
        getCurrentWebviewWindow().hide();
      }, e.payload.auto_dismiss_secs * 1000);

      return () => { clearTimeout(fadeTimer); clearTimeout(hideTimer); };
    });
    return () => { unlisten.then(f => f()); };
  }, []);

  const dismiss = () => {
    setVisible(false);
    getCurrentWebviewWindow().hide();
  };

  if (!visible) return null;

  return (
    <div
      onClick={dismiss}
      className={`p-3 bg-gray-900 text-white rounded-xl shadow-2xl border border-gray-700 cursor-pointer max-w-[260px] transition-opacity duration-1000 ${fading ? "opacity-0" : "opacity-100"}`}
    >
      {/* Triangle pointer */}
      <div className="absolute -right-2 top-4 w-0 h-0 border-t-8 border-b-8 border-l-8 border-transparent border-l-gray-900" />
      <p className="text-sm leading-relaxed">{text}</p>
      <p className="text-[10px] text-gray-500 mt-1">Click to dismiss</p>
    </div>
  );
}
```

**Step 2: Update main.tsx routing**

In `src/main.tsx:8-12`:

```tsx
import SpeechBubble from "./SpeechBubble";

const label = getCurrentWebviewWindow().label;

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    {label === "bubble" ? <BubbleApp /> : label === "speech-bubble" ? <SpeechBubble /> : <TrayApp />}
  </React.StrictMode>
);
```

**Step 3: Add show_speech_bubble function in lib.rs**

Add after the existing `show_bubble` function:

```rust
/// Create or show the speech bubble window next to the BubbleApp.
pub fn show_speech_bubble(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("speech-bubble") {
        position_speech_bubble(&window, app);
        let _ = window.show();
    } else {
        let window = WebviewWindowBuilder::new(
            app,
            "speech-bubble",
            WebviewUrl::App("index.html".into()),
        )
        .title("YoYo Speech")
        .inner_size(280.0, 120.0)
        .resizable(false)
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .visible(false)
        .skip_taskbar(true)
        .focused(false)
        .build()
        .expect("Failed to create speech bubble window");

        position_speech_bubble(&window, app);
        let _ = window.show();
    }
}

fn position_speech_bubble(window: &tauri::WebviewWindow, app: &tauri::AppHandle) {
    // Position to the left of the BubbleApp
    if let Some(bubble) = app.get_webview_window("bubble") {
        if let Ok(pos) = bubble.outer_position() {
            let x = (pos.x as f64 / bubble.scale_factor().unwrap_or(1.0)) - 290.0;
            let y = pos.y as f64 / bubble.scale_factor().unwrap_or(1.0) + 40.0;
            let _ = window.set_position(LogicalPosition::new(x, y));
        }
    }
}
```

**Step 4: Wire speech-bubble event to show the window**

In `lib.rs`, inside `setup()`, add a listener:

```rust
let app_for_bubble = app.handle().clone();
app.listen("speech-bubble", move |_event| {
    show_speech_bubble(&app_for_bubble);
});
```

**Step 5: Verify compilation**

Run: `cd src-tauri && cargo check && cd .. && npm run build`

**Step 6: Commit**

```
feat: speech bubble window — transparent overlay with auto-fade
```

---

### Task 8: Session Input Box in TrayApp (Chat During Session)

Add the ability to type messages to YoYo during a session from TrayApp.

**Files:**
- Modify: `src/TrayApp.tsx` (add chat input to session view)

**Step 1: Add chat input to session view**

In the in-session view's footer area (where the `StatusIndicator` is), replace with:

```tsx
{/* Input box for session chat */}
<div className="px-3 py-2 border-t border-gray-800">
  <div className="flex gap-2">
    <input
      type="text"
      value={chatInput}
      onChange={(e) => setChatInput(e.target.value)}
      onKeyDown={(e) => {
        if (e.key === "Enter" && !e.shiftKey) {
          e.preventDefault();
          handleChat();
        }
      }}
      placeholder="Ask YoYo..."
      className="flex-1 bg-gray-800 text-white text-sm rounded px-2 py-1.5 outline-none focus:ring-1 focus:ring-blue-500"
    />
  </div>
  <StatusIndicator />
</div>
```

Add state and handler:

```tsx
const [chatInput, setChatInput] = useState("");
const [chatLoading, setChatLoading] = useState(false);

const handleChat = async () => {
  if (!chatInput.trim() || chatLoading) return;
  setChatLoading(true);
  try {
    await sendSessionMessage(chatInput.trim());
    setChatInput("");
  } finally {
    setChatLoading(false);
  }
};
```

**Step 2: Verify build**

Run: `npm run build`

**Step 3: Commit**

```
feat: session chat input in TrayApp
```

---

### Task 9: Restore Active Session on Startup

Ensure active sessions survive app restart by checking SQLite on startup.

**Files:**
- Modify: `src-tauri/src/user_data.rs` (add get_active_session_from_db)
- Modify: `src-tauri/src/lib.rs` (restore session on startup)

**Step 1: Add database query for active session**

```rust
/// Get the currently active session from DB (for app restart recovery).
pub fn get_active_session_from_db() -> Result<Option<Session>, String> {
    let conn = get_db()?;
    let mut stmt = conn.prepare(
        "SELECT id, goal, started_at, ended_at, summary, status FROM sessions WHERE status = 'active' ORDER BY started_at DESC LIMIT 1"
    ).map_err(|e| format!("Failed to prepare query: {}", e))?;

    let mut rows = stmt.query_map([], |row| {
        Ok(Session {
            id: row.get(0)?,
            goal: row.get(1)?,
            started_at: row.get(2)?,
            ended_at: row.get(3)?,
            summary: row.get(4)?,
            status: row.get(5)?,
        })
    }).map_err(|e| format!("Failed to query active session: {}", e))?;

    match rows.next() {
        Some(Ok(s)) => Ok(Some(s)),
        _ => Ok(None),
    }
}
```

**Step 2: Restore in lib.rs setup()**

After the reflection counter restoration block (around line 84), add:

```rust
// Restore active session from DB on startup
{
    let state = app.state::<AppState>();
    if let Ok(Some(session)) = user_data::get_active_session_from_db() {
        if let Ok(mut active) = state.active_session.lock() {
            *active = Some(session);
        }
    }
}
```

**Step 3: Verify compilation**

Run: `cd src-tauri && cargo check`

**Step 4: Commit**

```
feat: restore active session from SQLite on app restart
```

---

### Task 10: Cleanup — Remove Replaced Features

Clean up features that are now replaced by the session system.

**Files:**
- Modify: `src/TrayApp.tsx` (remove old QuestBoard reference if needed)
- Modify: `src/BubbleApp.tsx` (remove old quest tracker UI)
- Assess: `src/components/QuestBoard.tsx` (deprecate or remove)

**Step 1: Assess what to remove**

The old `suggested_quest` mechanism in TrayApp and the main quest tracker in BubbleApp are now replaced by sessions. However, to avoid breaking changes, keep `suggested_quest` in AnalysisResult for now — it can be used as a "session suggestion" in idle mode.

- Remove the quest acceptance/rejection UI from TrayApp (old lines ~148-176)
- Remove the main quest progress bar from BubbleApp (old lines ~237-265)
- Keep `QuestBoard.tsx` for now (side quests still exist per design doc)

**Step 2: Remove old quest-related UI code**

Remove `suggested_quest` acceptance dialog from the old TrayApp (already done in Task 5's rewrite).
Remove main quest progress bar from the old BubbleApp (already done in Task 6's rewrite).

**Step 3: Verify everything still builds**

Run: `cd src-tauri && cargo check && cd .. && npm run build`

**Step 4: Commit**

```
refactor: clean up replaced quest UI — sessions are the new paradigm
```

---

### Task 11: End-to-End Verification

Verify the full session lifecycle works.

**Step 1: Build the entire project**

Run: `cd /Users/john/workspace/john/YoYo && cd src-tauri && cargo check`
Run: `cd /Users/john/workspace/john/YoYo && npm run build`

Expected: Both succeed with no errors.

**Step 2: Manual test checklist**

- [ ] App launches, shows idle view in tray panel
- [ ] Type goal in input, press Enter → session starts
- [ ] Timer counts up in tray panel
- [ ] Switch apps → analysis triggers → timeline entry appears
- [ ] AI response includes `on_track` field
- [ ] If drifting → speech bubble appears with drift message
- [ ] Type in chat input during session → AI responds via speech bubble
- [ ] Click End → session summary generated → returns to idle view
- [ ] Session appears in history
- [ ] Click history item → pre-fills input
- [ ] Restart app → active session restored

**Step 3: Final commit**

```
feat: session-driven YoYo redesign — complete implementation
```
