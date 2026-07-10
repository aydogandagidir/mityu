use crate::database::repositories::{
    action_item::ActionItemsRepository, meeting::MeetingsRepository,
    summary::SummaryProcessesRepository, summary_draft::SummariesRepository,
    transcript_chunk::TranscriptChunksRepository,
};
use crate::state::AppState;
use crate::summary::draft::{ActionItemDraft, BlockStatus, MeetingNotesDraft, SummaryStatus};
use crate::summary::language_detection::{detect_summary_language, SummaryLanguageDetection};
use crate::summary::metadata::{
    read_detected_summary_language_from_metadata, read_summary_language_from_metadata,
    write_detected_summary_language_to_metadata, write_summary_language_to_metadata,
};
use crate::summary::service::SummaryService;
use log::{error as log_error, info as log_info, warn as log_warn};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::{AppHandle, Runtime};

#[derive(Debug, Serialize, Deserialize)]
pub struct SummaryResponse {
    pub status: String,
    #[serde(rename = "meetingName")]
    pub meeting_name: Option<String>,
    pub meeting_id: String,
    pub start: Option<String>,
    pub end: Option<String>,
    pub data: Option<serde_json::Value>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProcessTranscriptResponse {
    pub message: String,
    pub process_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SummaryLanguageStorage {
    Metadata,
    LocalFallback,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MeetingSummaryLanguagePreference {
    pub language: Option<String>,
    pub storage: SummaryLanguageStorage,
}

impl MeetingSummaryLanguagePreference {
    fn metadata(language: Option<String>) -> Self {
        Self {
            language,
            storage: SummaryLanguageStorage::Metadata,
        }
    }

    fn local_fallback() -> Self {
        Self {
            language: None,
            storage: SummaryLanguageStorage::LocalFallback,
        }
    }
}

enum MeetingFolderResolution {
    Folder(PathBuf),
    NoFolder,
}

/// Saves a meeting summary (Native SQLx implementation)
///
/// Expected format: { "markdown": "...", "summary_json": [...BlockNote blocks...] }
#[tauri::command]
pub async fn api_save_meeting_summary<R: Runtime>(
    _app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
    meeting_id: String,
    summary: serde_json::Value,
    _auth_token: Option<String>,
) -> Result<serde_json::Value, String> {
    log_info!(
        "api_save_meeting_summary (native) called for meeting_id: {}",
        meeting_id
    );
    let pool = state.db_manager.pool();
    let ctx = crate::context::current();

    match SummaryProcessesRepository::update_meeting_summary(pool, &ctx, &meeting_id, &summary)
        .await
    {
        Ok(true) => {
            log_info!("Summary saved successfully for meeting_id: {}", meeting_id);
            Ok(serde_json::json!({
                "message": "Meeting summary saved successfully"
            }))
        }
        Ok(false) => {
            log_warn!(
                "Meeting not found or invalid JSON for meeting_id: {}",
                meeting_id
            );
            Err("Meeting not found or can't convert the json".into())
        }
        Err(e) => {
            log_error!("Failed to save meeting summary for {}: {}", meeting_id, e);
            Err(e.to_string())
        }
    }
}

/// Gets the per-meeting summary language override from metadata.json.
#[tauri::command]
pub async fn api_get_meeting_summary_language<R: Runtime>(
    _app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
    meeting_id: String,
) -> Result<MeetingSummaryLanguagePreference, String> {
    log_info!(
        "api_get_meeting_summary_language called for meeting_id: {}",
        meeting_id
    );

    match resolve_meeting_folder(state.db_manager.pool(), &meeting_id).await? {
        MeetingFolderResolution::Folder(folder) => read_summary_language_from_metadata(&folder)
            .map(MeetingSummaryLanguagePreference::metadata)
            .map_err(|e| e.to_string()),
        MeetingFolderResolution::NoFolder => Ok(MeetingSummaryLanguagePreference::local_fallback()),
    }
}

/// Saves or clears the per-meeting summary language override in metadata.json.
#[tauri::command]
pub async fn api_save_meeting_summary_language<R: Runtime>(
    _app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
    meeting_id: String,
    summary_language: Option<String>,
) -> Result<MeetingSummaryLanguagePreference, String> {
    log_info!(
        "api_save_meeting_summary_language called for meeting_id: {}, language: {:?}",
        meeting_id,
        summary_language
    );

    match resolve_meeting_folder(state.db_manager.pool(), &meeting_id).await? {
        MeetingFolderResolution::Folder(folder) => {
            write_summary_language_to_metadata(&folder, summary_language.as_deref())
                .map_err(|e| e.to_string())?;
            read_summary_language_from_metadata(&folder)
                .map(MeetingSummaryLanguagePreference::metadata)
                .map_err(|e| e.to_string())
        }
        MeetingFolderResolution::NoFolder => Ok(MeetingSummaryLanguagePreference::local_fallback()),
    }
}

/// Gets the cached Auto-detected summary language from metadata.json.
#[tauri::command]
pub async fn api_get_meeting_detected_summary_language<R: Runtime>(
    _app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
    meeting_id: String,
) -> Result<MeetingSummaryLanguagePreference, String> {
    log_info!(
        "api_get_meeting_detected_summary_language called for meeting_id: {}",
        meeting_id
    );

    match resolve_meeting_folder(state.db_manager.pool(), &meeting_id).await? {
        MeetingFolderResolution::Folder(folder) => {
            read_detected_summary_language_from_metadata(&folder)
                .map(MeetingSummaryLanguagePreference::metadata)
                .map_err(|e| e.to_string())
        }
        MeetingFolderResolution::NoFolder => Ok(MeetingSummaryLanguagePreference::local_fallback()),
    }
}

/// Saves or clears the cached Auto-detected summary language in metadata.json.
#[tauri::command]
pub async fn api_save_meeting_detected_summary_language<R: Runtime>(
    _app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
    meeting_id: String,
    detected_summary_language: Option<String>,
) -> Result<MeetingSummaryLanguagePreference, String> {
    log_info!(
        "api_save_meeting_detected_summary_language called for meeting_id: {}, language: {:?}",
        meeting_id,
        detected_summary_language
    );

    match resolve_meeting_folder(state.db_manager.pool(), &meeting_id).await? {
        MeetingFolderResolution::Folder(folder) => {
            write_detected_summary_language_to_metadata(
                &folder,
                detected_summary_language.as_deref(),
            )
            .map_err(|e| e.to_string())?;
            read_detected_summary_language_from_metadata(&folder)
                .map(MeetingSummaryLanguagePreference::metadata)
                .map_err(|e| e.to_string())
        }
        MeetingFolderResolution::NoFolder => Ok(MeetingSummaryLanguagePreference::local_fallback()),
    }
}

/// Detects the dominant supported summary language from transcript segments.
#[tauri::command]
pub async fn api_detect_transcript_summary_language(
    transcript_texts: Vec<String>,
) -> Result<SummaryLanguageDetection, String> {
    Ok(detect_summary_language(&transcript_texts))
}

async fn resolve_meeting_folder(
    pool: &sqlx::SqlitePool,
    meeting_id: &str,
) -> Result<MeetingFolderResolution, String> {
    let ctx = crate::context::current();
    let meeting = MeetingsRepository::get_meeting_metadata(pool, &ctx, meeting_id)
        .await
        .map_err(|e| format!("Failed to load meeting metadata: {}", e))?
        .ok_or_else(|| format!("Meeting not found: {}", meeting_id))?;

    let Some(folder_path) = meeting.folder_path.filter(|p| !p.trim().is_empty()) else {
        return Ok(MeetingFolderResolution::NoFolder);
    };

    Ok(MeetingFolderResolution::Folder(PathBuf::from(folder_path)))
}

/// Gets summary status and data (Native SQLx implementation)
///
/// Returns summary status (pending/processing/completed/failed) and parsed result data
#[tauri::command]
pub async fn api_get_summary<R: Runtime>(
    _app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
    meeting_id: String,
    _auth_token: Option<String>,
) -> Result<SummaryResponse, String> {
    log_info!(
        "api_get_summary (native) called for meeting_id: {}",
        meeting_id
    );
    let pool = state.db_manager.pool();
    let ctx = crate::context::current();

    match SummaryProcessesRepository::get_summary_data_for_meeting(pool, &ctx, &meeting_id).await {
        Ok(Some(process)) => {
            let status = process.status.to_lowercase();
            let error = process.error;

            // Parse result data if it exists (regardless of status)
            // This allows displaying restored summaries after cancellation or failure
            let data = if let Some(result_str) = process.result {
                match serde_json::from_str::<serde_json::Value>(&result_str) {
                    Ok(parsed) => Some(parsed),
                    Err(e) => {
                        log_error!("Failed to parse summary result JSON: {}", e);
                        None
                    }
                }
            } else {
                None
            };

            // Fetch meeting title from database
            let meeting_name = match MeetingsRepository::get_meeting(pool, &ctx, &meeting_id).await
            {
                Ok(Some(meeting_details)) => {
                    log_info!("Fetched meeting title: {}", &meeting_details.title);
                    Some(meeting_details.title)
                }
                Ok(None) => {
                    log_warn!("Meeting not found for meeting_id: {}", meeting_id);
                    None
                }
                Err(e) => {
                    log_error!("Failed to fetch meeting title: {}", e);
                    None
                }
            };

            let response = SummaryResponse {
                status: status.clone(),
                meeting_name,
                meeting_id: meeting_id.clone(),
                start: process.start_time.map(|t| t.to_rfc3339()),
                end: process.end_time.map(|t| t.to_rfc3339()),
                data,
                error,
            };

            log_info!(
                "Summary status for {}: {}, has_data: {}, meeting_name: {:?}",
                meeting_id,
                status,
                response.data.is_some(),
                response.meeting_name
            );
            Ok(response)
        }
        Ok(None) => {
            log_info!("No summary process found for meeting_id: {}", meeting_id);

            // Still fetch meeting title for idle state
            let meeting_name = match MeetingsRepository::get_meeting(pool, &ctx, &meeting_id).await
            {
                Ok(Some(meeting_details)) => Some(meeting_details.title),
                _ => None,
            };

            Ok(SummaryResponse {
                status: "idle".to_string(),
                meeting_name,
                meeting_id,
                start: None,
                end: None,
                data: None,
                error: None,
            })
        }
        Err(e) => {
            log_error!("Error retrieving summary for {}: {}", meeting_id, e);
            Err(format!("Failed to retrieve summary: {}", e))
        }
    }
}

/// Processes transcript and generates summary (Native SQLx implementation)
///
/// Spawns a background task and returns immediately with process_id
///
/// `structured` (BACKLOG C1.4, serde-default `None` → false): when `true`,
/// the background task first runs the structured, source-linked draft branch
/// (ADR-0019), degrading to the legacy markdown path on failure. Absent or
/// `false` leaves the legacy path byte-identical.
///
/// TS binding note: `invoke("api_process_transcript", { text, model,
/// modelName, meetingId?, customPrompt?, templateId?, summaryLanguage?,
/// structured?: boolean })` — the new key is optional; omitting it preserves
/// today's behavior.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn api_process_transcript<R: Runtime>(
    app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
    text: String,
    model: String,
    model_name: String,
    meeting_id: Option<String>,
    _chunk_size: Option<i32>,
    _overlap: Option<i32>,
    custom_prompt: Option<String>,
    template_id: Option<String>,
    summary_language: Option<String>,
    structured: Option<bool>,
    _auth_token: Option<String>,
) -> Result<ProcessTranscriptResponse, String> {
    use uuid::Uuid;

    let m_id = meeting_id.unwrap_or_else(|| format!("meeting-{}", Uuid::new_v4()));
    log_info!(
        "api_process_transcript (native) called for meeting_id: {}, model: {}",
        &m_id,
        &model
    );

    let pool = state.db_manager.pool().clone();
    let final_prompt = custom_prompt.unwrap_or_default();
    let final_template_id = template_id.unwrap_or_else(|| "daily_standup".to_string());
    // C1.4 flag: absent/None → false (legacy path untouched).
    let structured = structured.unwrap_or(false);

    // Normalise empty / whitespace-only to None so "" and null behave identically
    let summary_language = summary_language.and_then(|s| {
        let t = s.trim();
        if t.is_empty() {
            None
        } else {
            Some(t.to_string())
        }
    });

    let ctx = crate::context::current();

    // Opt-in PII/keyword redaction BEFORE the raw transcript is persisted at rest
    // (BACKLOG C6). `text` here is the verbatim in-memory transcript from the
    // frontend — NOT re-read from the already-redacted `transcripts` rows — and the
    // `save_transcript_data` write below lands it in `transcript_chunks` (durable,
    // full-text, and on the B4 sync allowlist). Redact ONCE here and rebind `text`
    // so the SAME redacted value flows to BOTH the `transcript_chunks` write and the
    // spawned summary task; the task's own redaction in service.rs then becomes an
    // idempotent no-op (kept for defense-in-depth). Disabled (the default) leaves
    // `text` unchanged. Fail safe: if the policy cannot be read, abort BEFORE the
    // write rather than persist raw text. Never logs the text or the term list.
    let text =
        match crate::database::repositories::setting::SettingsRepository::get_redaction_config(
            &pool, &ctx,
        )
        .await
        {
            Ok(cfg) if cfg.is_active() => {
                let redacted = crate::redaction::redact(&text, &cfg);
                log_info!(
                    "Applied redaction to transcript text before persisting transcript_chunks"
                );
                redacted
            }
            Ok(_) => text, // disabled/no-op
            Err(e) => {
                return Err(format!("Failed to load redaction configuration: {}", e));
            }
        };

    // Create or reset the process entry in the database
    SummaryProcessesRepository::create_or_reset_process(&pool, &ctx, &m_id)
        .await
        .map_err(|e| format!("Failed to initialize process: {}", e))?;

    log_info!("✓ Summary process initialized for meeting_id: {}", &m_id);

    // Save transcript chunks data (matching Python backend behavior)
    let chunk_size = _chunk_size.unwrap_or(40000);
    let overlap = _overlap.unwrap_or(1000);

    TranscriptChunksRepository::save_transcript_data(
        &pool,
        &ctx,
        &m_id,
        crate::database::repositories::transcript_chunk::TranscriptChunkData {
            text: &text,
            model: &model,
            model_name: &model_name,
            chunk_size,
            overlap,
        },
    )
    .await
    .map_err(|e| format!("Failed to save transcript data: {}", e))?;

    log_info!("✓ Transcript chunks saved for meeting_id: {}", &m_id);

    // Spawn background task for actual processing
    // Phase 2: when context::current() becomes request-scoped, capture the AuthContext
    // HERE (at command time) and pass it into the task — do not re-resolve inside (ADR-0010).
    let meeting_id_clone = m_id.clone();
    tauri::async_runtime::spawn(async move {
        SummaryService::process_transcript_background(
            app,
            pool,
            meeting_id_clone.clone(),
            text,
            model,
            model_name,
            final_prompt,
            final_template_id,
            summary_language,
            structured,
        )
        .await;
    });

    log_info!("🚀 Background task spawned for meeting_id: {}", &m_id);

    Ok(ProcessTranscriptResponse {
        message: "Summary generation started".to_string(),
        process_id: m_id,
    })
}

/// Cancels an ongoing summary generation process
///
/// This command triggers the cancellation token for the specified meeting,
/// stopping the summary generation gracefully.
#[tauri::command]
pub async fn api_cancel_summary<R: Runtime>(
    _app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
    meeting_id: String,
) -> Result<serde_json::Value, String> {
    log_info!("api_cancel_summary called for meeting_id: {}", meeting_id);

    // Trigger cancellation via the service
    let cancelled = SummaryService::cancel_summary(&meeting_id);

    if cancelled {
        // Update database status to cancelled
        let pool = state.db_manager.pool();
        let ctx = crate::context::current();
        if let Err(e) =
            SummaryProcessesRepository::update_process_cancelled(pool, &ctx, &meeting_id).await
        {
            log_error!(
                "Failed to update DB status to cancelled for {}: {}",
                meeting_id,
                e
            );
            return Err(format!("Failed to update cancellation status: {}", e));
        }

        log_info!(
            "Successfully cancelled summary generation for meeting_id: {}",
            meeting_id
        );
        Ok(serde_json::json!({
            "message": "Summary generation cancelled successfully",
            "meeting_id": meeting_id,
        }))
    } else {
        log_warn!(
            "No active summary generation found for meeting_id: {}",
            meeting_id
        );
        Ok(serde_json::json!({
            "message": "No active summary generation to cancel",
            "meeting_id": meeting_id,
        }))
    }
}

// ---------------------------------------------------------------------------
// C1.5 — HITL commands for source-linked summary drafts
// (BACKLOG C1.5; docs/CONTRACTS.md §4 approval rule; ADR-0019).
//
// Thin Tauri wrappers over the C1.3 tenant-scoped repositories
// (`SummariesRepository` / `ActionItemsRepository`). Every command:
//   - resolves identity ONLY via `crate::context::current()` — the frontend
//     never supplies `workspace_id`/`user_id` (ADR-0010, docs/MULTITENANCY.md
//     rule 2; `AuthContext` deliberately is not `Deserialize`);
//   - is ALWAYS active — HITL/approval enforcement is never behind the C1.4
//     `structured` generation flag (that flag only gates draft *generation*);
//   - maps `SummaryDraftError` to a CONTENT-FREE `String` via
//     [`summary_draft_err`] (the typed error itself already carries ids/counts
//     only — never block or transcript text — CLAUDE.md §0.6); and
//   - returns `Ok(false)` verbatim from the repository for an illegal
//     transition / not-found / cross-workspace no-op (no error, no leak).
//
// The repositories own ALL business rules (the §4 status machine,
// approve-time source re-resolution, "any block mutation de-approves the
// summary", soft-delete/scoping); these wrappers add NO logic beyond identity
// resolution and error mapping.
// ---------------------------------------------------------------------------

/// Content-free mapping of a repository [`SummaryDraftError`] to the `String`
/// error surfaced to the frontend. The `Display` impl of every variant is
/// ids/counts/status-tokens only (see `summary_draft.rs`), so this never leaks
/// meeting content into the Tauri error channel or the logs.
fn summary_draft_err(
    err: crate::database::repositories::summary_draft::SummaryDraftError,
) -> String {
    err.to_string()
}

/// The read-side payload for one meeting's structured, source-linked summary
/// draft plus its extracted action items (C1.5).
///
/// `draft` is `None` when the meeting has no (live) `summaries` row — a meeting
/// that was never summarized, or whose summary was soft-deleted. In that case
/// the HITL lifecycle fields are all `None`/`Draft` and `action_items` may
/// still be non-empty (action items live in their own table).
#[derive(Debug, Serialize)]
pub struct SummaryDraftResponse {
    /// The §4 [`MeetingNotesDraft`] hydrated from storage, or `None` when there
    /// is no summary row for this meeting in this workspace.
    pub draft: Option<MeetingNotesDraft>,
    /// Summary-level HITL status (`draft` when there is no row).
    pub status: SummaryStatus,
    /// Provider/model that generated the draft, if recorded.
    pub model: Option<String>,
    /// Summary template used, if recorded.
    pub template_id: Option<String>,
    /// When the draft was (re)generated (RFC 3339, as stored).
    pub generated_at: Option<String>,
    /// When a human approved the summary (RFC 3339, as stored).
    pub approved_at: Option<String>,
    /// Who approved the summary (`AuthContext::user_id` at approve time).
    pub approved_by: Option<String>,
    /// The meeting's live action-item drafts, in display order (§4 shape).
    pub action_items: Vec<ActionItemDraft>,
}

/// Reads the meeting's structured summary draft (§4) and its live action items.
///
/// Identity is resolved via `context::current()`; both reads are
/// workspace-scoped and exclude soft-deleted rows. A meeting with no summary
/// row yields `draft: None` (not an error). Repository errors map to a
/// content-free `String`.
///
/// TS binding: `invoke("api_get_summary_draft", { meetingId })
///   -> SummaryDraftResponse`.
#[tauri::command]
pub async fn api_get_summary_draft(
    state: tauri::State<'_, AppState>,
    meeting_id: String,
) -> Result<SummaryDraftResponse, String> {
    log_info!(
        "api_get_summary_draft called for meeting_id: {}",
        meeting_id
    );
    let pool = state.db_manager.pool();
    let ctx = crate::context::current();

    let summary = SummariesRepository::get_by_meeting(pool, &ctx, &meeting_id)
        .await
        .map_err(summary_draft_err)?;

    // Action items are their own table: present even when there is no summary.
    let action_items = ActionItemsRepository::list_by_meeting(pool, &ctx, &meeting_id)
        .await
        .map_err(summary_draft_err)?
        .into_iter()
        .map(|row| ActionItemDraft {
            id: row.id,
            text: row.text,
            assignee: row.assignee,
            due: row.due,
            status: row.status,
            source_chunk_id: row.source_chunk_id,
        })
        .collect();

    Ok(match summary {
        Some(row) => SummaryDraftResponse {
            draft: Some(row.draft),
            status: row.status,
            model: row.model,
            template_id: row.template_id,
            generated_at: row.generated_at,
            approved_at: row.approved_at,
            approved_by: row.approved_by,
            action_items,
        },
        None => SummaryDraftResponse {
            draft: None,
            status: SummaryStatus::Draft,
            model: None,
            template_id: None,
            generated_at: None,
            approved_at: None,
            approved_by: None,
            action_items,
        },
    })
}

/// Human APPROVE of one summary block. The repository re-validates that the
/// block's `source_chunk_id` still resolves NOW (retranscription may have
/// removed the cited segment — ADR-0019). `Ok(false)` = illegal transition,
/// unknown block, no summary in this workspace, or stale evidence.
///
/// TS binding: `invoke("api_approve_summary_block", { meetingId, blockId })
///   -> boolean`.
#[tauri::command]
pub async fn api_approve_summary_block(
    state: tauri::State<'_, AppState>,
    meeting_id: String,
    block_id: String,
) -> Result<bool, String> {
    log_info!(
        "api_approve_summary_block called for meeting_id: {}, block_id: {}",
        meeting_id,
        block_id
    );
    let pool = state.db_manager.pool();
    let ctx = crate::context::current();
    SummariesRepository::set_block_status(pool, &ctx, &meeting_id, &block_id, BlockStatus::Approved)
        .await
        .map_err(summary_draft_err)
}

/// Human REJECT of one summary block. `Ok(false)` = illegal transition,
/// unknown block, or no summary in this workspace.
///
/// TS binding: `invoke("api_reject_summary_block", { meetingId, blockId })
///   -> boolean`.
#[tauri::command]
pub async fn api_reject_summary_block(
    state: tauri::State<'_, AppState>,
    meeting_id: String,
    block_id: String,
) -> Result<bool, String> {
    log_info!(
        "api_reject_summary_block called for meeting_id: {}, block_id: {}",
        meeting_id,
        block_id
    );
    let pool = state.db_manager.pool();
    let ctx = crate::context::current();
    SummariesRepository::set_block_status(pool, &ctx, &meeting_id, &block_id, BlockStatus::Rejected)
        .await
        .map_err(summary_draft_err)
}

/// Human EDIT of one summary block's content. NEVER touches `source_chunk_id`
/// (the evidence anchor is immutable — §4); the first edit preserves the
/// generated text and the summary drops back to draft (repository invariant).
/// `Ok(false)` = unknown block, no summary in this workspace, or the block is
/// rejected (restore to draft first).
///
/// TS binding: `invoke("api_edit_summary_block", { meetingId, blockId, content })
///   -> boolean`.
#[tauri::command]
pub async fn api_edit_summary_block(
    state: tauri::State<'_, AppState>,
    meeting_id: String,
    block_id: String,
    content: String,
) -> Result<bool, String> {
    log_info!(
        "api_edit_summary_block called for meeting_id: {}, block_id: {}",
        meeting_id,
        block_id
    );
    let pool = state.db_manager.pool();
    let ctx = crate::context::current();
    SummariesRepository::edit_block(pool, &ctx, &meeting_id, &block_id, &content)
        .await
        .map_err(summary_draft_err)
}

/// Human RESTORE of one rejected summary block back to draft (`rejected ->
/// draft`, the only legal arc out of rejected). `Ok(false)` = illegal
/// transition, unknown block, or no summary in this workspace.
///
/// TS binding: `invoke("api_restore_summary_block", { meetingId, blockId })
///   -> boolean`.
#[tauri::command]
pub async fn api_restore_summary_block(
    state: tauri::State<'_, AppState>,
    meeting_id: String,
    block_id: String,
) -> Result<bool, String> {
    log_info!(
        "api_restore_summary_block called for meeting_id: {}, block_id: {}",
        meeting_id,
        block_id
    );
    let pool = state.db_manager.pool();
    let ctx = crate::context::current();
    SummariesRepository::set_block_status(pool, &ctx, &meeting_id, &block_id, BlockStatus::Draft)
        .await
        .map_err(summary_draft_err)
}

/// The explicit HUMAN approval of a whole summary (docs/CONTRACTS.md §4). The
/// repository enforces the gate AT THIS MOMENT: at least one non-rejected
/// block, every non-rejected block Approved, and every such block's
/// `source_chunk_id` still resolves (ADR-0019). `Ok(false)` on any gate
/// failure; on success the row is stamped `approved_at`/`approved_by`.
///
/// TS binding: `invoke("api_approve_summary", { meetingId }) -> boolean`.
#[tauri::command]
pub async fn api_approve_summary(
    state: tauri::State<'_, AppState>,
    meeting_id: String,
) -> Result<bool, String> {
    log_info!("api_approve_summary called for meeting_id: {}", meeting_id);
    let pool = state.db_manager.pool();
    let ctx = crate::context::current();
    SummariesRepository::approve_summary(pool, &ctx, &meeting_id)
        .await
        .map_err(summary_draft_err)
}

/// Human APPROVE of one action item. The repository re-validates the item's
/// `source_chunk_id` NOW (ADR-0019). `Ok(false)` = illegal transition, unknown
/// item in this workspace, or stale evidence.
///
/// TS binding: `invoke("api_approve_action_item", { itemId }) -> boolean`.
#[tauri::command]
pub async fn api_approve_action_item(
    state: tauri::State<'_, AppState>,
    item_id: String,
) -> Result<bool, String> {
    log_info!("api_approve_action_item called for item_id: {}", item_id);
    let pool = state.db_manager.pool();
    let ctx = crate::context::current();
    ActionItemsRepository::set_status(pool, &ctx, &item_id, BlockStatus::Approved)
        .await
        .map_err(summary_draft_err)
}

/// Cross-meeting open action items for the Home dashboard (Phase C). Returns
/// every non-rejected, non-deleted item in the current workspace (newest
/// meetings first, capped), each with its meeting id/title so the UI can link
/// back to the report. Read-only; tenant-scoped via the ambient context.
///
/// TS binding: `invoke("api_get_open_action_items", { limit? }) -> OpenActionItem[]`.
#[derive(serde::Serialize)]
pub struct OpenActionItem {
    pub id: String,
    pub meeting_id: String,
    pub meeting_title: String,
    pub text: String,
    pub assignee: Option<String>,
    pub due: Option<String>,
    pub status: BlockStatus,
    pub source_chunk_id: String,
}

#[tauri::command]
pub async fn api_get_open_action_items(
    state: tauri::State<'_, AppState>,
    limit: Option<i64>,
) -> Result<Vec<OpenActionItem>, String> {
    let limit = limit.unwrap_or(20).clamp(1, 100);
    log_info!("api_get_open_action_items called (limit {})", limit);
    let pool = state.db_manager.pool();
    let ctx = crate::context::current();
    let rows = ActionItemsRepository::list_open(pool, &ctx, limit)
        .await
        .map_err(summary_draft_err)?;
    Ok(rows
        .into_iter()
        .map(|(item, meeting_title)| OpenActionItem {
            id: item.id,
            meeting_id: item.meeting_id,
            meeting_title,
            text: item.text,
            assignee: item.assignee,
            due: item.due,
            status: item.status,
            source_chunk_id: item.source_chunk_id,
        })
        .collect())
}

/// Human REJECT of one action item. `Ok(false)` = illegal transition or
/// unknown item in this workspace.
///
/// TS binding: `invoke("api_reject_action_item", { itemId }) -> boolean`.
#[tauri::command]
pub async fn api_reject_action_item(
    state: tauri::State<'_, AppState>,
    item_id: String,
) -> Result<bool, String> {
    log_info!("api_reject_action_item called for item_id: {}", item_id);
    let pool = state.db_manager.pool();
    let ctx = crate::context::current();
    ActionItemsRepository::set_status(pool, &ctx, &item_id, BlockStatus::Rejected)
        .await
        .map_err(summary_draft_err)
}

/// Human RESTORE of one rejected action item back to draft (`rejected ->
/// draft`). `Ok(false)` = illegal transition or unknown item in this workspace.
///
/// TS binding: `invoke("api_restore_action_item", { itemId }) -> boolean`.
#[tauri::command]
pub async fn api_restore_action_item(
    state: tauri::State<'_, AppState>,
    item_id: String,
) -> Result<bool, String> {
    log_info!("api_restore_action_item called for item_id: {}", item_id);
    let pool = state.db_manager.pool();
    let ctx = crate::context::current();
    ActionItemsRepository::set_status(pool, &ctx, &item_id, BlockStatus::Draft)
        .await
        .map_err(summary_draft_err)
}

/// A tri-state patch for one nullable action-item field over the Tauri wire.
///
/// DEVIATION (documented): the C1.3 `ActionItemsRepository::edit` distinguishes
/// three intents per nullable field — leave unchanged / clear / set — via
/// `Option<Option<&str>>`. That double-`Option` does NOT survive JSON: serde
/// collapses an absent key and an explicit `null` to the same `None`, losing
/// the "clear" vs "leave unchanged" distinction across the boundary. Rather
/// than change the repository signature, the command layer models each patch as
/// this explicit, unambiguous enum and lowers it back to `Option<Option<&str>>`
/// for the repository. Wire form (serde `snake_case`, internally tagged):
///   `{ "op": "keep" }` — leave unchanged (the default when the whole field is
///     omitted from `FieldPatch`),
///   `{ "op": "clear" }` — set to NULL,
///   `{ "op": "set", "value": "..." }` — set to the given string.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum FieldPatch {
    /// Leave the stored value unchanged (repository `None`).
    Keep,
    /// Clear the stored value to NULL (repository `Some(None)`).
    Clear,
    /// Replace the stored value (repository `Some(Some(value))`).
    Set {
        /// The new value.
        value: String,
    },
}

impl FieldPatch {
    /// Lower this wire patch to the repository's `Option<Option<&str>>` intent.
    fn as_repo_patch(&self) -> Option<Option<&str>> {
        match self {
            FieldPatch::Keep => None,
            FieldPatch::Clear => Some(None),
            FieldPatch::Set { value } => Some(Some(value.as_str())),
        }
    }
}

/// The typed request for [`api_edit_action_item`]. Absent fields default to
/// [`FieldPatch::Keep`] / no-op, so a caller sends only what it wants to
/// change.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditActionItemRequest {
    /// New action text; `None`/absent leaves it unchanged.
    #[serde(default)]
    pub text: Option<String>,
    /// Assignee patch (keep / clear / set); absent = keep.
    #[serde(default = "field_patch_keep")]
    pub assignee: FieldPatch,
    /// Due-date patch (keep / clear / set); absent = keep.
    #[serde(default = "field_patch_keep")]
    pub due: FieldPatch,
}

/// serde default for an omitted [`FieldPatch`] (a `Default` derive is avoided so
/// the wire vocabulary stays the single source of truth for the variants).
fn field_patch_keep() -> FieldPatch {
    FieldPatch::Keep
}

/// Human EDIT of one action item (patch semantics). NEVER touches
/// `source_chunk_id` (§4); the first text edit preserves the generated text and
/// flips status to `edited` (repository invariant). `Ok(false)` = nothing to
/// change, unknown item in this workspace, or the item is rejected (restore to
/// draft first).
///
/// TS binding: `invoke("api_edit_action_item", { itemId, req: { text?,
///   assignee?: { op:'keep'|'clear'|'set', value? },
///   due?: { op:'keep'|'clear'|'set', value? } } }) -> boolean`. The `req`
/// object mirrors the C1.3 patch intents; omit a field to leave it unchanged.
#[tauri::command]
pub async fn api_edit_action_item(
    state: tauri::State<'_, AppState>,
    item_id: String,
    req: EditActionItemRequest,
) -> Result<bool, String> {
    log_info!("api_edit_action_item called for item_id: {}", item_id);
    let pool = state.db_manager.pool();
    let ctx = crate::context::current();
    ActionItemsRepository::edit(
        pool,
        &ctx,
        &item_id,
        req.text.as_deref(),
        req.assignee.as_repo_patch(),
        req.due.as_repo_patch(),
    )
    .await
    .map_err(summary_draft_err)
}

#[cfg(test)]
mod hitl_command_types_tests {
    use super::*;

    /// The wire vocabulary for [`FieldPatch`] lowers to the exact repository
    /// `Option<Option<&str>>` intents — the whole reason this enum exists is to
    /// keep "clear" distinct from "keep" across JSON (which double-`Option`
    /// cannot).
    #[test]
    fn field_patch_lowers_to_repo_intents() {
        assert_eq!(FieldPatch::Keep.as_repo_patch(), None);
        assert_eq!(FieldPatch::Clear.as_repo_patch(), Some(None));
        assert_eq!(
            FieldPatch::Set {
                value: "ayse".to_string()
            }
            .as_repo_patch(),
            Some(Some("ayse"))
        );
    }

    /// An absent key, an explicit `keep`, `clear`, and `set` all deserialize to
    /// the intended patch — and, critically, `clear` and an omitted field are
    /// DIFFERENT (the bug the typed request struct exists to prevent).
    #[test]
    fn edit_request_distinguishes_clear_from_omitted() {
        let omitted: EditActionItemRequest =
            serde_json::from_str(r#"{ "text": "new text" }"#).expect("omitted patches parse");
        assert_eq!(omitted.text.as_deref(), Some("new text"));
        assert_eq!(omitted.assignee.as_repo_patch(), None, "omitted = keep");
        assert_eq!(omitted.due.as_repo_patch(), None, "omitted = keep");

        let explicit: EditActionItemRequest = serde_json::from_str(
            r#"{ "assignee": { "op": "clear" }, "due": { "op": "set", "value": "Friday" } }"#,
        )
        .expect("explicit patches parse");
        assert_eq!(explicit.text, None);
        assert_eq!(
            explicit.assignee.as_repo_patch(),
            Some(None),
            "explicit clear must be Some(None), NOT None"
        );
        assert_eq!(explicit.due.as_repo_patch(), Some(Some("Friday")));
    }
}
