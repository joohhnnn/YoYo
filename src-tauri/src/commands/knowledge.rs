use crate::user_data::{self, KnowledgeRecord};

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
