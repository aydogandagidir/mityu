//! Tauri surface for "Ask This Meeting".
//!
//! Thin by design: the command resolves identity and paths, then delegates. All
//! of the retrieval, grounding and refusal logic lives in [`super::service`],
//! where it is testable without a running app.

use tauri::{AppHandle, Manager, Runtime};

use crate::ask::service::{ask_meeting, AskOutcome};
use crate::state::AppState;
use crate::{log_error, log_info};

/// Answer a question about one meeting from that meeting's own transcript.
///
/// Read-only: nothing is written, and the result cannot become approved
/// content. Identity comes from `crate::context::current()`, never from the
/// frontend (`docs/CONTRACTS.md`).
#[tauri::command]
pub async fn api_ask_meeting<R: Runtime>(
    app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
    meeting_id: String,
    question: String,
    model_provider: String,
    model_name: String,
) -> Result<AskOutcome, String> {
    // The question is meeting-adjacent user content: log its shape, never its text.
    log_info!(
        "api_ask_meeting called (question_chars={}, provider={})",
        question.chars().count(),
        model_provider
    );

    let pool = state.db_manager.pool();
    let ctx = crate::context::current();
    let app_data_dir = app.path().app_data_dir().ok();

    match ask_meeting(
        pool,
        &ctx,
        app_data_dir.as_ref(),
        &meeting_id,
        &question,
        &model_provider,
        &model_name,
    )
    .await
    {
        Ok(outcome) => {
            // Shape only -- claim text is meeting content.
            let shape = match &outcome {
                AskOutcome::NoEvidence => "no_evidence".to_string(),
                AskOutcome::Answered {
                    claims, dropped, ..
                } => format!("answered kept={} dropped={}", claims.len(), dropped.len()),
                AskOutcome::Refused { dropped, .. } => format!("refused dropped={}", dropped.len()),
            };
            log_info!("api_ask_meeting completed ({shape})");
            Ok(outcome)
        }
        Err(e) => {
            log_error!("api_ask_meeting failed: {}", e);
            Err(e.to_string())
        }
    }
}
