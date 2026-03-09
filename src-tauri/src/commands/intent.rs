use crate::ai_engine::{self, IntentResult};
use crate::screen_context;
use crate::user_data;
use tauri::{AppHandle, Emitter};

use super::settings::load_data;

/// Understand user intent: takes natural language input + screen context, returns a plan.
#[tauri::command]
pub async fn understand_intent(app: AppHandle, user_input: String) -> Result<IntentResult, String> {
    let _ = app.emit("analysis-progress", "Understanding...");
    let data = load_data(&app);

    // Capture screen context (fast, no screenshot needed for intent)
    let ctx = screen_context::capture(&app);

    // Gather activity history and quests (same pattern as do_analyze)
    let recent = user_data::get_recent_activities(30).unwrap_or_default();

    let main_quests: Vec<String> = data
        .tasks
        .iter()
        .filter(|t| t.quest_type == "main" && !t.done)
        .map(|t| t.text.clone())
        .collect();
    let main_quest = if main_quests.is_empty() {
        None
    } else {
        Some(main_quests.join("\n- "))
    };

    // Call AI for intent understanding
    let result = if data.settings.ai_mode == "api" && !data.settings.api_key.is_empty() {
        ai_engine::intent_with_api(
            &user_input,
            &data.settings.api_key,
            &data.settings.model,
            &data.settings.language,
            &recent,
            main_quest.as_deref(),
            &ctx,
        )
        .await
    } else {
        ai_engine::intent_with_cli(
            &user_input,
            &data.settings.model,
            &data.settings.language,
            &recent,
            main_quest.as_deref(),
            &ctx,
        )
        .await
    }?;

    // Emit completion event for any listeners
    let _ = app.emit("intent-complete", &result);

    Ok(result)
}
