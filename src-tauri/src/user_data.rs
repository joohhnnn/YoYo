use rusqlite::Connection;
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
