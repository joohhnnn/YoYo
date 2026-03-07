use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

/// Singleton database connection to avoid repeated open/init_tables overhead
/// and prevent "database is locked" errors under high-frequency access.
static DB_CONN: OnceLock<Mutex<Connection>> = OnceLock::new();

/// Returns ~/.yoyo/, creating it if it doesn't exist.
pub fn yoyo_dir() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or("Cannot determine home directory")?;
    let dir = home.join(".yoyo");
    fs::create_dir_all(&dir).map_err(|e| format!("Failed to create ~/.yoyo: {}", e))?;
    Ok(dir)
}

// ---------------------------------------------------------------------------
// Markdown files
// ---------------------------------------------------------------------------

const DEFAULT_PROFILE: &str = r#"# YoYo User Profile

## About Me
<!-- Describe yourself: role, profession, interests -->

## Tools I Use
<!-- List your frequently used apps and tools -->

## Preferences
<!-- Language preference, workflow habits, etc. -->
"#;

const DEFAULT_CONTEXT: &str = r#"# Current Context

<!-- Dynamic context that changes frequently.
     Edit this file to tell YoYo what you're working on right now. -->
"#;

fn profile_path() -> Result<PathBuf, String> {
    Ok(yoyo_dir()?.join("profile.md"))
}

fn context_path() -> Result<PathBuf, String> {
    Ok(yoyo_dir()?.join("context.md"))
}

/// Read profile.md, creating the default template if it doesn't exist.
pub fn read_profile() -> Result<String, String> {
    let path = profile_path()?;
    if !path.exists() {
        fs::write(&path, DEFAULT_PROFILE)
            .map_err(|e| format!("Failed to write default profile: {}", e))?;
    }
    fs::read_to_string(&path).map_err(|e| format!("Failed to read profile: {}", e))
}

/// Write profile.md.
pub fn write_profile(content: &str) -> Result<(), String> {
    let path = profile_path()?;
    fs::write(&path, content).map_err(|e| format!("Failed to write profile: {}", e))
}

/// Read context.md, creating the default template if it doesn't exist.
pub fn read_context() -> Result<String, String> {
    let path = context_path()?;
    if !path.exists() {
        fs::write(&path, DEFAULT_CONTEXT)
            .map_err(|e| format!("Failed to write default context: {}", e))?;
    }
    fs::read_to_string(&path).map_err(|e| format!("Failed to read context: {}", e))
}

/// Write context.md.
pub fn write_context(content: &str) -> Result<(), String> {
    let path = context_path()?;
    fs::write(&path, content).map_err(|e| format!("Failed to write context: {}", e))
}

// ---------------------------------------------------------------------------
// SQLite
// ---------------------------------------------------------------------------

fn db_path() -> Result<PathBuf, String> {
    Ok(yoyo_dir()?.join("yoyo.db"))
}

/// Open (or create) the database and ensure all tables exist.
/// Kept for initial setup in `ensure_initialized()`.
fn open_db_fresh() -> Result<Connection, String> {
    let path = db_path()?;
    let conn =
        Connection::open(&path).map_err(|e| format!("Failed to open database: {}", e))?;
    init_tables(&conn)?;
    Ok(conn)
}

/// Get a reference to the singleton database connection.
/// Initializes on first call; subsequent calls reuse the same connection.
fn get_db() -> Result<std::sync::MutexGuard<'static, Connection>, String> {
    let mutex = DB_CONN.get_or_init(|| {
        let conn = open_db_fresh().expect("Failed to initialize database");
        Mutex::new(conn)
    });
    mutex.lock().map_err(|e| format!("Database lock poisoned: {}", e))
}

fn init_tables(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS meta (
            key   TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS vocab (
            id           INTEGER PRIMARY KEY AUTOINCREMENT,
            word         TEXT NOT NULL,
            meaning      TEXT,
            context      TEXT,
            source       TEXT,
            learned_at   TEXT DEFAULT (datetime('now')),
            review_count INTEGER DEFAULT 0,
            next_review  TEXT
        );

        CREATE TABLE IF NOT EXISTS learning_progress (
            id         INTEGER PRIMARY KEY AUTOINCREMENT,
            topic      TEXT NOT NULL,
            item       TEXT NOT NULL,
            status     TEXT DEFAULT 'new',
            notes      TEXT,
            updated_at TEXT DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS activity_log (
            id           INTEGER PRIMARY KEY AUTOINCREMENT,
            app_name     TEXT NOT NULL DEFAULT '',
            bundle_id    TEXT NOT NULL DEFAULT '',
            context      TEXT NOT NULL,
            actions_json TEXT DEFAULT '[]',
            created_at   TEXT DEFAULT (datetime('now','localtime')),
            updated_at   TEXT DEFAULT (datetime('now','localtime'))
        );

        CREATE TABLE IF NOT EXISTS reflection_log (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            summary         TEXT NOT NULL,
            activity_count  INTEGER NOT NULL,
            period_start    TEXT,
            period_end      TEXT,
            created_at      TEXT DEFAULT (datetime('now','localtime'))
        );

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
        ",
    )
    .map_err(|e| format!("Failed to initialize database tables: {}", e))
}

/// Initialize the ~/.yoyo directory and all default files on app startup.
/// Also eagerly initializes the singleton DB connection.
pub fn ensure_initialized() -> Result<(), String> {
    // Creates dir + default profile + default context
    read_profile()?;
    read_context()?;
    // Initialize the singleton DB connection (creates tables on first call)
    drop(get_db()?);
    Ok(())
}

// ---------------------------------------------------------------------------
// Activity log (observation mode)
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ActivityRecord {
    pub id: i64,
    pub app_name: String,
    pub bundle_id: String,
    pub context: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ReflectionRecord {
    pub id: i64,
    pub summary: String,
    pub activity_count: i64,
    pub period_start: String,
    pub period_end: String,
    pub created_at: String,
}

// ---------------------------------------------------------------------------
// Session mode
// ---------------------------------------------------------------------------

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

/// Character bigram set similarity — works well for both Chinese and English text.
fn bigram_similarity(a: &str, b: &str) -> f64 {
    let bigrams_a: HashSet<(char, char)> = a.chars().zip(a.chars().skip(1)).collect();
    let bigrams_b: HashSet<(char, char)> = b.chars().zip(b.chars().skip(1)).collect();
    let intersection = bigrams_a.intersection(&bigrams_b).count() as f64;
    let union = bigrams_a.union(&bigrams_b).count() as f64;
    if union == 0.0 {
        1.0
    } else {
        intersection / union
    }
}

/// Record an activity, deduplicating against recent entries.
///
/// Dedup strategy:
/// 1. Check the last 3 records (regardless of app) for context similarity.
///    If any has bigram similarity > 0.6 → update that record's timestamp (dedup).
/// 2. Otherwise insert a new record.
///
/// Returns true if a new record was inserted, false if deduplicated.
pub fn record_activity(
    app_name: &str,
    bundle_id: &str,
    context: &str,
    actions_json: &str,
) -> Result<bool, String> {
    let conn = get_db()?;

    // Fetch last 3 records to compare against (cross-app dedup)
    let mut stmt = conn
        .prepare("SELECT id, app_name, context FROM activity_log ORDER BY id DESC LIMIT 3")
        .map_err(|e| e.to_string())?;

    let recent: Vec<(i64, String, String)> = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    // Find the best matching recent record
    let mut best_match: Option<(i64, f64)> = None;
    for (id, _prev_app, prev_context) in &recent {
        let sim = bigram_similarity(prev_context, context);
        if sim > 0.6 {
            if best_match.is_none() || sim > best_match.unwrap().1 {
                best_match = Some((*id, sim));
            }
        }
    }

    if let Some((match_id, _)) = best_match {
        // Similar context found — update timestamp and app info
        conn.execute(
            "UPDATE activity_log SET app_name = ?1, bundle_id = ?2, updated_at = datetime('now','localtime') WHERE id = ?3",
            params![app_name, bundle_id, match_id],
        )
        .map_err(|e| e.to_string())?;
        Ok(false)
    } else {
        insert_activity(&conn, app_name, bundle_id, context, actions_json)?;
        Ok(true)
    }
}

fn insert_activity(
    conn: &Connection,
    app_name: &str,
    bundle_id: &str,
    context: &str,
    actions_json: &str,
) -> Result<(), String> {
    conn.execute(
        "INSERT INTO activity_log (app_name, bundle_id, context, actions_json) VALUES (?1, ?2, ?3, ?4)",
        params![app_name, bundle_id, context, actions_json],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Get the most recent N distinct activities (newest first).
pub fn get_recent_activities(limit: usize) -> Result<Vec<ActivityRecord>, String> {
    let conn = get_db()?;
    let mut stmt = conn
        .prepare(
            "SELECT id, app_name, bundle_id, context, created_at, updated_at
             FROM activity_log ORDER BY id DESC LIMIT ?1",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map(params![limit as i64], |row| {
            Ok(ActivityRecord {
                id: row.get(0)?,
                app_name: row.get(1)?,
                bundle_id: row.get(2)?,
                context: row.get(3)?,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
            })
        })
        .map_err(|e| e.to_string())?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row.map_err(|e| e.to_string())?);
    }
    Ok(result)
}

/// Get total number of activity records.
pub fn get_total_activity_count() -> Result<i64, String> {
    let conn = get_db()?;
    conn.query_row("SELECT COUNT(*) FROM activity_log", [], |row| row.get(0))
        .map_err(|e| e.to_string())
}

/// Save a reflection summary.
pub fn save_reflection(
    summary: &str,
    activity_count: i64,
    period_start: &str,
    period_end: &str,
) -> Result<(), String> {
    let conn = get_db()?;
    conn.execute(
        "INSERT INTO reflection_log (summary, activity_count, period_start, period_end) VALUES (?1, ?2, ?3, ?4)",
        params![summary, activity_count, period_start, period_end],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Get the latest reflection record.
pub fn get_latest_reflection() -> Result<Option<ReflectionRecord>, String> {
    let conn = get_db()?;
    conn.query_row(
        "SELECT id, summary, activity_count, period_start, period_end, created_at
         FROM reflection_log ORDER BY id DESC LIMIT 1",
        [],
        |row| {
            Ok(ReflectionRecord {
                id: row.get(0)?,
                summary: row.get(1)?,
                activity_count: row.get(2)?,
                period_start: row.get(3)?,
                period_end: row.get(4)?,
                created_at: row.get(5)?,
            })
        },
    )
    .optional()
    .map_err(|e| e.to_string())
}

/// Check whether profile.md is still the default template (no user content).
pub fn is_profile_default() -> Result<bool, String> {
    let content = read_profile()?;
    let has_user_content = content.lines().any(|line| {
        let trimmed = line.trim();
        !trimmed.is_empty()
            && !trimmed.starts_with('#')
            && !trimmed.starts_with("<!--")
            && !trimmed.ends_with("-->")
    });
    Ok(!has_user_content)
}

/// Update context.md with the latest reflection summary (replace, not append).
pub fn update_context_with_reflection(summary: &str, timestamp: &str) -> Result<(), String> {
    let content = read_context()?;
    // Remove existing AI Observed Patterns section
    let cleaned: String = content
        .split("## AI Observed Patterns")
        .next()
        .unwrap_or(&content)
        .trim_end()
        .to_string();
    let updated = format!(
        "{}\n\n## AI Observed Patterns (auto-updated)\n{}\n_Last updated: {}_\n",
        cleaned, summary, timestamp
    );
    write_context(&updated)
}

// ---------------------------------------------------------------------------
// Session CRUD
// ---------------------------------------------------------------------------

/// Create a new session. Returns the Session struct.
pub fn create_session(goal: &str) -> Result<Session, String> {
    let conn = get_db()?;
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    conn.execute(
        "INSERT INTO sessions (id, goal, started_at) VALUES (?1, ?2, ?3)",
        params![id, goal, now],
    )
    .map_err(|e| format!("Failed to create session: {}", e))?;

    Ok(Session {
        id,
        goal: goal.to_string(),
        started_at: now,
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
        params![summary, status, session_id],
    )
    .map_err(|e| format!("Failed to end session: {}", e))?;
    Ok(())
}

/// Get a session by ID.
pub fn get_session(session_id: &str) -> Result<Option<Session>, String> {
    let conn = get_db()?;
    conn.query_row(
        "SELECT id, goal, started_at, ended_at, summary, status FROM sessions WHERE id = ?1",
        params![session_id],
        |row| {
            Ok(Session {
                id: row.get(0)?,
                goal: row.get(1)?,
                started_at: row.get(2)?,
                ended_at: row.get(3)?,
                summary: row.get(4)?,
                status: row.get(5)?,
            })
        },
    )
    .optional()
    .map_err(|e| e.to_string())
}

/// Get the currently active session from DB (for app restart recovery).
pub fn get_active_session_from_db() -> Result<Option<Session>, String> {
    let conn = get_db()?;
    conn.query_row(
        "SELECT id, goal, started_at, ended_at, summary, status FROM sessions WHERE status = 'active' ORDER BY started_at DESC LIMIT 1",
        [],
        |row| {
            Ok(Session {
                id: row.get(0)?,
                goal: row.get(1)?,
                started_at: row.get(2)?,
                ended_at: row.get(3)?,
                summary: row.get(4)?,
                status: row.get(5)?,
            })
        },
    )
    .optional()
    .map_err(|e| e.to_string())
}

/// Get recent sessions for the idle view.
pub fn get_session_history(limit: u32) -> Result<Vec<Session>, String> {
    let conn = get_db()?;
    let mut stmt = conn
        .prepare(
            "SELECT id, goal, started_at, ended_at, summary, status FROM sessions ORDER BY started_at DESC LIMIT ?1",
        )
        .map_err(|e| format!("Failed to prepare query: {}", e))?;

    let sessions = stmt
        .query_map(params![limit], |row| {
            Ok(Session {
                id: row.get(0)?,
                goal: row.get(1)?,
                started_at: row.get(2)?,
                ended_at: row.get(3)?,
                summary: row.get(4)?,
                status: row.get(5)?,
            })
        })
        .map_err(|e| format!("Failed to query sessions: {}", e))?
        .filter_map(|r| r.ok())
        .collect();

    Ok(sessions)
}

/// Add a timeline entry to a session.
pub fn add_timeline_entry(
    session_id: &str,
    context: &str,
    app_name: &str,
) -> Result<TimelineEntry, String> {
    let conn = get_db()?;
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    conn.execute(
        "INSERT INTO session_timeline (session_id, timestamp, context, app_name) VALUES (?1, ?2, ?3, ?4)",
        params![session_id, now, context, app_name],
    )
    .map_err(|e| format!("Failed to add timeline entry: {}", e))?;

    let id = conn.last_insert_rowid();
    Ok(TimelineEntry {
        id,
        session_id: session_id.to_string(),
        timestamp: now,
        context: context.to_string(),
        app_name: app_name.to_string(),
    })
}

/// Get all timeline entries for a session.
pub fn get_session_timeline(session_id: &str) -> Result<Vec<TimelineEntry>, String> {
    let conn = get_db()?;
    let mut stmt = conn
        .prepare(
            "SELECT id, session_id, timestamp, context, app_name FROM session_timeline WHERE session_id = ?1 ORDER BY timestamp ASC",
        )
        .map_err(|e| format!("Failed to prepare query: {}", e))?;

    let entries = stmt
        .query_map(params![session_id], |row| {
            Ok(TimelineEntry {
                id: row.get(0)?,
                session_id: row.get(1)?,
                timestamp: row.get(2)?,
                context: row.get(3)?,
                app_name: row.get(4)?,
            })
        })
        .map_err(|e| format!("Failed to query timeline: {}", e))?
        .filter_map(|r| r.ok())
        .collect();

    Ok(entries)
}
