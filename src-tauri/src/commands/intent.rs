use crate::ai_engine::{self, IntentResult, PlanStep};
use crate::screen_context;
use crate::user_data;
use tauri::{AppHandle, Emitter};

use super::settings::load_data;

/// Simple bigram similarity for workflow trigger matching.
fn bigram_similarity(a: &str, b: &str) -> f64 {
    use std::collections::HashSet;
    let a = a.to_lowercase();
    let b = b.to_lowercase();
    let bigrams_a: HashSet<(char, char)> = a.chars().zip(a.chars().skip(1)).collect();
    let bigrams_b: HashSet<(char, char)> = b.chars().zip(b.chars().skip(1)).collect();
    let intersection = bigrams_a.intersection(&bigrams_b).count() as f64;
    let union = bigrams_a.union(&bigrams_b).count() as f64;
    if union == 0.0 {
        0.0
    } else {
        intersection / union
    }
}

/// Try to match user input against saved workflows.
/// Returns the best matching workflow if similarity > 0.5.
fn match_workflow(user_input: &str) -> Option<user_data::WorkflowRecord> {
    let workflows = user_data::get_all_workflows().unwrap_or_default();
    if workflows.is_empty() {
        return None;
    }

    let mut best: Option<(user_data::WorkflowRecord, f64)> = None;
    for wf in workflows {
        let sim = bigram_similarity(user_input, &wf.trigger_context);
        if sim > 0.5 {
            if best.is_none() || sim > best.as_ref().unwrap().1 {
                best = Some((wf, sim));
            }
        }
    }

    best.map(|(wf, _)| wf)
}

/// Understand user intent: takes natural language input + screen context, returns a plan.
#[tauri::command]
pub async fn understand_intent(app: AppHandle, user_input: String) -> Result<IntentResult, String> {
    let _ = app.emit("analysis-progress", "Understanding...");
    let data = load_data(&app);

    // Check saved workflows for a match first
    if let Some(wf) = match_workflow(&user_input) {
        let steps: Vec<PlanStep> = serde_json::from_str(&wf.steps_json).unwrap_or_default();
        if !steps.is_empty() {
            let result = IntentResult {
                understanding: format!("Matched workflow: {}", wf.name),
                plan: steps,
                needs_confirmation: true,
                workflow_id: Some(wf.id),
            };
            let _ = app.emit("intent-complete", &result);
            return Ok(result);
        }
    }

    // Capture screen context (fast, no screenshot needed for intent)
    let ctx = screen_context::capture(&app);

    // Gather activity context — use summary if available + recent 5
    let summary = user_data::get_latest_summary().unwrap_or(None);
    let summary_text = summary.as_ref().map(|s| s.summary_text.as_str());
    let recent = user_data::get_recent_activities(5).unwrap_or_default();

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
    let current_scene = data.settings.current_scene.as_deref();

    // Call AI for intent understanding
    let result = if data.settings.ai_mode == "api" && !data.settings.api_key.is_empty() {
        ai_engine::intent_with_api(
            &user_input,
            &data.settings.api_key,
            &data.settings.model,
            &data.settings.language,
            summary_text,
            &recent,
            main_quest.as_deref(),
            current_scene,
            &ctx,
        )
        .await
    } else {
        ai_engine::intent_with_cli(
            &user_input,
            &data.settings.model,
            &data.settings.language,
            summary_text,
            &recent,
            main_quest.as_deref(),
            current_scene,
            &ctx,
        )
        .await
    }?;

    // Emit completion event for any listeners
    let _ = app.emit("intent-complete", &result);

    Ok(result)
}
