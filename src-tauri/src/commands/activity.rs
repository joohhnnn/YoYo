use crate::user_data::{self, ActivityRecord, RawContextRecord};

#[tauri::command]
pub fn get_recent_activities(limit: Option<usize>) -> Result<Vec<ActivityRecord>, String> {
    user_data::get_recent_activities(limit.unwrap_or(30))
}

#[tauri::command]
pub fn search_raw_context(
    query: String,
    limit: Option<usize>,
) -> Result<Vec<RawContextRecord>, String> {
    user_data::search_raw_context(&query, limit.unwrap_or(50))
}
