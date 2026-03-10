use crate::user_data;

/// Record a new execution. Returns the execution id.
#[tauri::command]
pub fn record_execution(
    input_text: String,
    plan_json: String,
    workflow_id: Option<i64>,
) -> Result<i64, String> {
    user_data::insert_execution(workflow_id, &input_text, &plan_json)
}

/// Update execution status when done.
#[tauri::command]
pub fn complete_execution(
    id: i64,
    status: String,
    result_json: Option<String>,
) -> Result<(), String> {
    user_data::update_execution_status(id, &status, result_json.as_deref())
}

/// Record user feedback on an execution.
#[tauri::command]
pub fn feedback_execution(id: i64, feedback: String) -> Result<(), String> {
    user_data::update_execution_feedback(id, &feedback)
}

/// Save a workflow from a successful execution.
#[tauri::command]
pub fn save_workflow(
    name: String,
    trigger_context: String,
    steps_json: String,
) -> Result<i64, String> {
    user_data::insert_workflow(&name, &trigger_context, &steps_json)
}

/// Get all saved workflows.
#[tauri::command]
pub fn get_workflows() -> Result<Vec<user_data::WorkflowRecord>, String> {
    user_data::get_all_workflows()
}

/// Delete a workflow by id.
#[tauri::command]
pub fn delete_workflow(id: i64) -> Result<(), String> {
    user_data::delete_workflow(id)
}

/// Update workflow success/fail count.
#[tauri::command]
pub fn update_workflow_count(id: i64, success: bool) -> Result<(), String> {
    user_data::increment_workflow_count(id, success)
}

/// Get recent executions (newest first).
#[tauri::command]
pub fn get_recent_executions(
    limit: Option<usize>,
) -> Result<Vec<user_data::ExecutionRecord>, String> {
    user_data::get_recent_executions(limit.unwrap_or(20))
}
