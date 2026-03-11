use crate::ai_engine;
use crate::user_data::{self, KnowledgeRecord};

use super::settings::load_data;
use tauri::AppHandle;

/// Spaced repetition intervals in seconds: 1h, 4h, 1d, 3d, 7d, 14d, 30d
const REVIEW_INTERVALS: &[i64] = &[3600, 14400, 86400, 259200, 604800, 1209600, 2592000];

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct KnowledgeStats {
    pub total: i64,
    pub due: i64,
}

/// Get knowledge items due for review.
#[tauri::command]
pub fn get_due_knowledge(limit: Option<usize>) -> Result<Vec<KnowledgeRecord>, String> {
    user_data::get_due_knowledge(limit.unwrap_or(1))
}

/// Get all knowledge items by kind.
#[tauri::command]
pub fn get_knowledge_by_kind(
    kind: String,
    limit: Option<usize>,
) -> Result<Vec<KnowledgeRecord>, String> {
    user_data::get_knowledge_by_kind(&kind, limit.unwrap_or(50))
}

/// Record a review result: update spaced repetition metadata.
#[tauri::command]
pub fn review_knowledge(id: i64, success: bool) -> Result<(), String> {
    let record = user_data::get_knowledge(id)?;
    let mut meta: serde_json::Value =
        serde_json::from_str(&record.metadata).unwrap_or(serde_json::json!({}));

    let review_count = meta["review_count"].as_i64().unwrap_or(0) + 1;
    let mut interval_level = meta["interval_level"].as_i64().unwrap_or(0);

    if success {
        // Advance to next interval (cap at max)
        interval_level = (interval_level + 1).min((REVIEW_INTERVALS.len() - 1) as i64);
    } else {
        // Reset to beginning
        interval_level = 0;
    }

    let next_secs = REVIEW_INTERVALS[interval_level as usize];
    let now = chrono::Local::now().naive_local();
    let next_review = now + chrono::Duration::seconds(next_secs);

    meta["review_count"] = serde_json::json!(review_count);
    meta["interval_level"] = serde_json::json!(interval_level);
    meta["next_review"] = serde_json::json!(next_review.format("%Y-%m-%d %H:%M:%S").to_string());
    meta["last_reviewed"] = serde_json::json!(now.format("%Y-%m-%d %H:%M:%S").to_string());

    user_data::update_knowledge_metadata(id, &meta.to_string())
}

/// Delete a knowledge item.
#[tauri::command]
pub fn delete_knowledge(id: i64) -> Result<(), String> {
    user_data::delete_knowledge(id)
}

/// Get knowledge stats (total count, due count).
#[tauri::command]
pub fn get_knowledge_stats() -> Result<KnowledgeStats, String> {
    let total = user_data::get_knowledge_count()?;
    let due = user_data::get_due_knowledge(100)?.len() as i64;
    Ok(KnowledgeStats { total, due })
}

/// Generate a learning note for the most recently ended session.
/// Returns the file path of the generated note, or None if no items found.
#[tauri::command]
pub async fn generate_note(app: AppHandle) -> Result<Option<String>, String> {
    let session = user_data::get_last_ended_session()?.ok_or("No ended session found")?;

    // Only generate for named sessions (not observation mode)
    let scene_name = match &session.scene_name {
        Some(name) => name.clone(),
        None => return Ok(None),
    };

    let ended_at = session.ended_at.as_deref().unwrap_or("");
    if ended_at.is_empty() {
        return Err("Session has no end time".to_string());
    }

    // Query knowledge items in session time range
    let items = user_data::get_knowledge_in_range(&session.started_at, ended_at)?;
    let raw_contexts = user_data::get_raw_context_in_range(&session.started_at, ended_at)?;
    if items.is_empty() && raw_contexts.is_empty() {
        return Ok(None);
    }

    // Calculate duration
    let duration_str = calc_duration(&session.started_at, ended_at);

    // Convert KnowledgeRecord -> KnowledgeItem for the prompt
    let knowledge_items: Vec<ai_engine::KnowledgeItem> = items
        .iter()
        .map(|r| {
            let meta: serde_json::Value =
                serde_json::from_str(&r.metadata).unwrap_or(serde_json::json!({}));
            ai_engine::KnowledgeItem {
                kind: r.kind.clone(),
                content: r.content.clone(),
                definition: meta["definition"].as_str().map(|s| s.to_string()),
            }
        })
        .collect();

    // Build prompt and call AI
    let prompt = ai_engine::build_note_prompt_with_context(
        &scene_name,
        &duration_str,
        &knowledge_items,
        &raw_contexts,
    );

    let data = load_data(&app);
    let note_content = if data.settings.ai_mode == "api" && !data.settings.api_key.is_empty() {
        ai_engine::simple_chat_api(
            &prompt,
            &data.settings.api_key,
            &data.settings.model,
            Some(2000),
        )
        .await?
    } else {
        ai_engine::simple_chat_cli(&prompt, &data.settings.model, Some(2000)).await?
    };

    // Build filename: YYYY-MM-DD-scene-name.md
    let date_prefix = &session.started_at[..10]; // "2026-03-12"
    let safe_name: String = scene_name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();
    let filename = format!("{}-{}.md", date_prefix, safe_name);

    // Write file
    let notes_dir = user_data::notes_dir()?;
    let file_path = notes_dir.join(&filename);

    // Build final note with header
    let header = format!(
        "# {}\n> {} — {}（{}）\n\n",
        scene_name, session.started_at, ended_at, duration_str
    );
    let full_content = format!("{}{}", header, note_content);

    std::fs::write(&file_path, &full_content)
        .map_err(|e| format!("Failed to write note: {}", e))?;

    Ok(Some(file_path.to_string_lossy().to_string()))
}

/// Calculate human-readable duration between two datetime strings.
fn calc_duration(start: &str, end: &str) -> String {
    let fmt = "%Y-%m-%d %H:%M:%S";
    let start_dt = chrono::NaiveDateTime::parse_from_str(start, fmt);
    let end_dt = chrono::NaiveDateTime::parse_from_str(end, fmt);
    match (start_dt, end_dt) {
        (Ok(s), Ok(e)) => {
            let dur = e - s;
            let hours = dur.num_hours();
            let mins = dur.num_minutes() % 60;
            if hours > 0 {
                format!("{}h{}min", hours, mins)
            } else {
                format!("{}min", mins)
            }
        }
        _ => "unknown".to_string(),
    }
}
