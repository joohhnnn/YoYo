use rusqlite::{params, Connection};
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

/// Open (or create) the database and run pending migrations.
fn open_db_fresh() -> Result<Connection, String> {
    let path = db_path()?;
    let conn = Connection::open(&path).map_err(|e| format!("Failed to open database: {}", e))?;
    run_migrations(&conn)?;
    Ok(conn)
}

/// Get a reference to the singleton database connection.
/// Initializes on first call; subsequent calls reuse the same connection.
fn get_db() -> Result<std::sync::MutexGuard<'static, Connection>, String> {
    let mutex = DB_CONN.get_or_init(|| {
        let conn = open_db_fresh().expect("Failed to initialize database");
        Mutex::new(conn)
    });
    mutex
        .lock()
        .map_err(|e| format!("Database lock poisoned: {}", e))
}

// ---------------------------------------------------------------------------
// Versioned migration system
// ---------------------------------------------------------------------------

/// Get current schema version from the meta table. Returns 0 if no version set.
fn get_schema_version(conn: &Connection) -> i64 {
    // meta table may not exist yet
    conn.query_row(
        "SELECT value FROM meta WHERE key = 'schema_version'",
        [],
        |row| row.get::<_, String>(0),
    )
    .ok()
    .and_then(|v| v.parse::<i64>().ok())
    .unwrap_or(0)
}

fn set_schema_version(conn: &Connection, version: i64) -> Result<(), String> {
    conn.execute(
        "INSERT OR REPLACE INTO meta (key, value) VALUES ('schema_version', ?1)",
        params![version.to_string()],
    )
    .map_err(|e| format!("Failed to set schema version: {}", e))?;
    Ok(())
}

/// Run all pending migrations in order.
fn run_migrations(conn: &Connection) -> Result<(), String> {
    // Ensure meta table exists first (needed to track versions)
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS meta (
            key   TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );",
    )
    .map_err(|e| format!("Failed to create meta table: {}", e))?;

    let current = get_schema_version(conn);

    // Migration 1: V1 baseline tables
    if current < 1 {
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS activity_log (
                id           INTEGER PRIMARY KEY AUTOINCREMENT,
                app_name     TEXT NOT NULL DEFAULT '',
                bundle_id    TEXT NOT NULL DEFAULT '',
                context      TEXT NOT NULL,
                actions_json TEXT DEFAULT '[]',
                created_at   TEXT DEFAULT (datetime('now','localtime')),
                updated_at   TEXT DEFAULT (datetime('now','localtime'))
            );
            ",
        )
        .map_err(|e| format!("Migration 1 failed: {}", e))?;
        set_schema_version(conn, 1)?;
        eprintln!("DB migration 1 applied: baseline tables");
    }

    // Migration 2: V2 tables (workflows, executions, knowledge)
    if current < 2 {
        conn.execute_batch(
            "
            -- Drop unused V1 tables
            DROP TABLE IF EXISTS vocab;
            DROP TABLE IF EXISTS learning_progress;

            -- Learned workflows: trigger context + steps
            CREATE TABLE IF NOT EXISTS workflows (
                id              INTEGER PRIMARY KEY AUTOINCREMENT,
                name            TEXT NOT NULL,
                trigger_context TEXT,
                steps_json      TEXT NOT NULL DEFAULT '[]',
                success_count   INTEGER DEFAULT 0,
                fail_count      INTEGER DEFAULT 0,
                created_at      TEXT DEFAULT (datetime('now','localtime')),
                updated_at      TEXT DEFAULT (datetime('now','localtime'))
            );

            -- Execution history for workflows and ad-hoc actions
            CREATE TABLE IF NOT EXISTS executions (
                id            INTEGER PRIMARY KEY AUTOINCREMENT,
                workflow_id   INTEGER,
                input_text    TEXT,
                screen_context TEXT,
                plan_json     TEXT,
                result_json   TEXT,
                status        TEXT DEFAULT 'pending',
                user_feedback TEXT,
                created_at    TEXT DEFAULT (datetime('now','localtime')),
                completed_at  TEXT,
                FOREIGN KEY (workflow_id) REFERENCES workflows(id)
            );

            -- Knowledge store (vocab, notes, etc. for Phase 2)
            CREATE TABLE IF NOT EXISTS knowledge (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                kind        TEXT NOT NULL,
                content     TEXT NOT NULL,
                source      TEXT,
                metadata    TEXT DEFAULT '{}',
                created_at  TEXT DEFAULT (datetime('now','localtime'))
            );
            ",
        )
        .map_err(|e| format!("Migration 2 failed: {}", e))?;
        set_schema_version(conn, 2)?;
        eprintln!("DB migration 2 applied: V2 tables (workflows, executions, knowledge)");
    }

    Ok(())
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

// ---------------------------------------------------------------------------
// Executions (execution history)
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ExecutionRecord {
    pub id: i64,
    pub workflow_id: Option<i64>,
    pub input_text: Option<String>,
    pub plan_json: Option<String>,
    pub result_json: Option<String>,
    pub status: String,
    pub user_feedback: Option<String>,
    pub created_at: String,
    pub completed_at: Option<String>,
}

/// Insert a new execution record. Returns the new row id.
pub fn insert_execution(
    workflow_id: Option<i64>,
    input_text: &str,
    plan_json: &str,
) -> Result<i64, String> {
    let conn = get_db()?;
    conn.execute(
        "INSERT INTO executions (workflow_id, input_text, plan_json, status) VALUES (?1, ?2, ?3, 'pending')",
        params![workflow_id, input_text, plan_json],
    )
    .map_err(|e| e.to_string())?;
    Ok(conn.last_insert_rowid())
}

/// Update execution status and optional result.
pub fn update_execution_status(
    id: i64,
    status: &str,
    result_json: Option<&str>,
) -> Result<(), String> {
    let conn = get_db()?;
    conn.execute(
        "UPDATE executions SET status = ?1, result_json = ?2, completed_at = datetime('now','localtime') WHERE id = ?3",
        params![status, result_json, id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Update user feedback on an execution.
pub fn update_execution_feedback(id: i64, feedback: &str) -> Result<(), String> {
    let conn = get_db()?;
    conn.execute(
        "UPDATE executions SET user_feedback = ?1 WHERE id = ?2",
        params![feedback, id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Get recent executions, newest first.
pub fn get_recent_executions(limit: usize) -> Result<Vec<ExecutionRecord>, String> {
    let conn = get_db()?;
    let mut stmt = conn
        .prepare(
            "SELECT id, workflow_id, input_text, plan_json, result_json, status, user_feedback, created_at, completed_at
             FROM executions ORDER BY id DESC LIMIT ?1",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map(params![limit as i64], |row| {
            Ok(ExecutionRecord {
                id: row.get(0)?,
                workflow_id: row.get(1)?,
                input_text: row.get(2)?,
                plan_json: row.get(3)?,
                result_json: row.get(4)?,
                status: row.get(5)?,
                user_feedback: row.get(6)?,
                created_at: row.get(7)?,
                completed_at: row.get(8)?,
            })
        })
        .map_err(|e| e.to_string())?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row.map_err(|e| e.to_string())?);
    }
    Ok(result)
}

// ---------------------------------------------------------------------------
// Workflows (learned workflows)
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WorkflowRecord {
    pub id: i64,
    pub name: String,
    pub trigger_context: String,
    pub steps_json: String,
    pub success_count: i64,
    pub fail_count: i64,
    pub created_at: String,
    pub updated_at: String,
}

/// Insert a new workflow. Returns the new row id.
pub fn insert_workflow(name: &str, trigger_context: &str, steps_json: &str) -> Result<i64, String> {
    let conn = get_db()?;
    conn.execute(
        "INSERT INTO workflows (name, trigger_context, steps_json) VALUES (?1, ?2, ?3)",
        params![name, trigger_context, steps_json],
    )
    .map_err(|e| e.to_string())?;
    Ok(conn.last_insert_rowid())
}

/// Get all saved workflows.
pub fn get_all_workflows() -> Result<Vec<WorkflowRecord>, String> {
    let conn = get_db()?;
    let mut stmt = conn
        .prepare(
            "SELECT id, name, trigger_context, steps_json, success_count, fail_count, created_at, updated_at
             FROM workflows ORDER BY updated_at DESC",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([], |row| {
            Ok(WorkflowRecord {
                id: row.get(0)?,
                name: row.get(1)?,
                trigger_context: row.get(2).unwrap_or_default(),
                steps_json: row.get(3)?,
                success_count: row.get(4)?,
                fail_count: row.get(5)?,
                created_at: row.get(6)?,
                updated_at: row.get(7)?,
            })
        })
        .map_err(|e| e.to_string())?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row.map_err(|e| e.to_string())?);
    }
    Ok(result)
}

/// Increment success or fail count for a workflow.
pub fn increment_workflow_count(id: i64, success: bool) -> Result<(), String> {
    let conn = get_db()?;
    let field = if success {
        "success_count"
    } else {
        "fail_count"
    };
    conn.execute(
        &format!(
            "UPDATE workflows SET {} = {} + 1, updated_at = datetime('now','localtime') WHERE id = ?1",
            field, field
        ),
        params![id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Delete a workflow by id.
pub fn delete_workflow(id: i64) -> Result<(), String> {
    let conn = get_db()?;
    conn.execute("DELETE FROM workflows WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    Ok(())
}
