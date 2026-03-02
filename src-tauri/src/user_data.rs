use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

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
pub fn open_db() -> Result<Connection, String> {
    let path = db_path()?;
    let conn =
        Connection::open(&path).map_err(|e| format!("Failed to open database: {}", e))?;
    init_tables(&conn)?;
    Ok(conn)
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
        ",
    )
    .map_err(|e| format!("Failed to initialize database tables: {}", e))
}

/// Initialize the ~/.yoyo directory and all default files on app startup.
pub fn ensure_initialized() -> Result<(), String> {
    // Creates dir + default profile + default context + db with tables
    read_profile()?;
    read_context()?;
    open_db()?;
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

/// Jaccard word-set similarity for short text comparison.
fn jaccard_similarity(a: &str, b: &str) -> f64 {
    let set_a: HashSet<&str> = a.split_whitespace().collect();
    let set_b: HashSet<&str> = b.split_whitespace().collect();
    let intersection = set_a.intersection(&set_b).count() as f64;
    let union = set_a.union(&set_b).count() as f64;
    if union == 0.0 {
        1.0
    } else {
        intersection / union
    }
}

/// Record an activity, deduplicating against the last entry.
/// Returns true if a new record was inserted, false if deduplicated.
pub fn record_activity(
    app_name: &str,
    bundle_id: &str,
    context: &str,
    actions_json: &str,
) -> Result<bool, String> {
    let conn = open_db()?;

    let last: Option<(i64, String, String)> = conn
        .query_row(
            "SELECT id, app_name, context FROM activity_log ORDER BY id DESC LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()
        .map_err(|e| e.to_string())?;

    match last {
        Some((last_id, last_app, last_context)) => {
            if last_app != app_name {
                // Different app — new activity
                insert_activity(&conn, app_name, bundle_id, context, actions_json)?;
                Ok(true)
            } else {
                let similarity = jaccard_similarity(&last_context, context);
                if similarity > 0.7 {
                    // Similar context in same app — just update timestamp
                    conn.execute(
                        "UPDATE activity_log SET updated_at = datetime('now','localtime') WHERE id = ?1",
                        params![last_id],
                    )
                    .map_err(|e| e.to_string())?;
                    Ok(false)
                } else {
                    // Different activity in same app
                    insert_activity(&conn, app_name, bundle_id, context, actions_json)?;
                    Ok(true)
                }
            }
        }
        None => {
            insert_activity(&conn, app_name, bundle_id, context, actions_json)?;
            Ok(true)
        }
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
    let conn = open_db()?;
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
    let conn = open_db()?;
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
    let conn = open_db()?;
    conn.execute(
        "INSERT INTO reflection_log (summary, activity_count, period_start, period_end) VALUES (?1, ?2, ?3, ?4)",
        params![summary, activity_count, period_start, period_end],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Get the latest reflection record.
pub fn get_latest_reflection() -> Result<Option<ReflectionRecord>, String> {
    let conn = open_db()?;
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
