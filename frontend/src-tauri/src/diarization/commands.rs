//! Tauri surface for speaker diarization.
//!
//! Thin by design: these resolve identity and paths and delegate. The decisions
//! — is this meeting diarizable, what does an empty result mean, what gets
//! written — live in [`super::service`], where they are testable without a
//! running app.
//!
//! Nothing here runs during capture (`CLAUDE.md` §4). Identity comes from
//! `crate::context::current()`, never from the frontend (`docs/CONTRACTS.md`).

use tauri::{AppHandle, Manager, Runtime};

use crate::diarization::{models, service};
use crate::state::AppState;
use crate::{log_error, log_info};

async fn folder_path_for(
    pool: &sqlx::SqlitePool,
    ctx: &crate::context::AuthContext,
    meeting_id: &str,
) -> Result<Option<String>, String> {
    sqlx::query_scalar::<_, Option<String>>(
        "SELECT folder_path FROM meetings WHERE id = ? AND workspace_id = ?",
    )
    .bind(meeting_id)
    .bind(ctx.tenant_id.as_str())
    .fetch_optional(pool)
    .await
    .map(Option::flatten)
    .map_err(|e| format!("read meeting: {e}"))
}

fn models_dir<R: Runtime>(app: &AppHandle<R>) -> Result<std::path::PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("resolve app data directory: {e}"))?;
    Ok(models::models_dir(&dir))
}

/// Whether this meeting can be diarized, and what has already happened.
///
/// Four distinct states, so the UI can offer a pass, report one that found
/// nothing, or explain that a transcripts-only recording has no audio to work
/// from — rather than collapsing them into one ambiguous message.
#[tauri::command]
pub async fn api_diarization_availability<R: Runtime>(
    app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
    meeting_id: String,
) -> Result<service::Availability, String> {
    let pool = state.db_manager.pool();
    let ctx = crate::context::current();
    let folder = folder_path_for(pool, &ctx, &meeting_id).await?;
    let dir = models_dir(&app)?;

    service::availability(pool, &ctx, &meeting_id, folder.as_deref(), &dir)
        .await
        .map_err(|e| {
            log_error!("diarization availability failed: {e:#}");
            format!("{e:#}")
        })
}

/// Download and verify the diarization models.
///
/// Separate from running a pass on purpose: this is the only part that touches
/// the network, so a user can be asked before ~35 MB is fetched, and a machine
/// that already has them never reaches out at all.
#[tauri::command]
pub async fn api_diarization_download_models<R: Runtime>(app: AppHandle<R>) -> Result<(), String> {
    let dir = models_dir(&app)?;
    models::ensure(&dir, |label, done, total| {
        if total > 0 && done == total {
            log_info!("diarization model fetched: {label} ({done} bytes)");
        }
    })
    .await
    .map(|_| ())
    .map_err(|e| {
        log_error!("diarization model download failed: {e:#}");
        format!("{e:#}")
    })
}

/// Run one post-hoc diarization pass and store the result.
///
/// Returns how many turns were stored. **Zero is a success**, not a failure: it
/// means the pass ran and separated nothing, and `diarized_at` now records that
/// so the UI stops offering a pass that has already been tried.
#[tauri::command]
pub async fn api_diarize_meeting<R: Runtime>(
    app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
    meeting_id: String,
) -> Result<usize, String> {
    log_info!("api_diarize_meeting called");

    let pool = state.db_manager.pool();
    let ctx = crate::context::current();
    let folder = folder_path_for(pool, &ctx, &meeting_id).await?;
    let dir = models_dir(&app)?;

    service::diarize_meeting(pool, &ctx, &meeting_id, folder.as_deref(), &dir)
        .await
        .map_err(|e| {
            log_error!("diarization failed: {e:#}");
            format!("{e:#}")
        })
}

/// This meeting's stored speaker turns.
#[tauri::command]
pub async fn api_get_speaker_turns(
    state: tauri::State<'_, AppState>,
    meeting_id: String,
) -> Result<Vec<crate::database::repositories::speaker_turn::SpeakerTurn>, String> {
    let ctx = crate::context::current();
    crate::database::repositories::speaker_turn::SpeakerTurnsRepository::list_for_meeting(
        state.db_manager.pool(),
        &ctx,
        &meeting_id,
    )
    .await
    .map_err(|e| format!("{e:#}"))
}
