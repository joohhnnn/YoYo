use crate::user_data::{self, ActivityRecord};

#[tauri::command]
pub fn get_recent_activities(limit: Option<usize>) -> Result<Vec<ActivityRecord>, String> {
    user_data::get_recent_activities(limit.unwrap_or(30))
}
