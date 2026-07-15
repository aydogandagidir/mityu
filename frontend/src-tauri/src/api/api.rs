use log::{debug as log_debug, error as log_error, info as log_info, warn as log_warn};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Runtime};
use tauri_plugin_store::StoreExt;

use crate::{
    database::{
        models::MeetingModel,
        repositories::{
            meeting::MeetingsRepository, setting::SettingsRepository,
            transcript::TranscriptsRepository,
        },
    },
    state::AppState,
    summary::CustomOpenAIConfig,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Meeting {
    pub id: String,
    pub title: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SearchRequest {
    pub query: String,
}

/// Where within a meeting a search query matched. Serialized as the lowercase
/// string `"transcript" | "summary"` so the UI can label each hit (BACKLOG C3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SearchMatchField {
    Transcript,
    Summary,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TranscriptSearchResult {
    pub id: String,
    pub title: String,
    #[serde(rename = "matchContext")]
    pub match_context: String,
    pub timestamp: String,
    /// Which field the query matched in (`transcript` or `summary`). One row per
    /// meeting; when a meeting matches in both, the transcript hit wins (see
    /// `search_transcripts`).
    #[serde(rename = "matchedIn")]
    pub matched_in: SearchMatchField,
}

/// A ranked, source-resolvable local evidence hit (Product Intelligence v1).
///
/// Unlike the legacy meeting-level search result above, every hit is anchored
/// to one real transcript segment. SQLite FTS5 BM25 is internal ordering
/// metadata: its raw corpus-dependent score is deliberately not exposed as a
/// confidence value or cross-workspace side channel.
#[derive(Debug, Serialize, Deserialize)]
pub struct EvidenceSearchResult {
    /// Meeting id the evidence belongs to (kept as `id` for existing sidebar
    /// item conventions).
    pub id: String,
    pub title: String,
    #[serde(rename = "matchContext")]
    pub match_context: String,
    pub timestamp: String,
    #[serde(rename = "matchedIn")]
    pub matched_in: SearchMatchField,
    #[serde(rename = "sourceChunkId")]
    pub source_chunk_id: String,
    #[serde(rename = "audioStartTime")]
    pub audio_start_time: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ModelConfig {
    pub provider: String,
    pub model: String,
    #[serde(rename = "whisperModel")]
    pub whisper_model: String,
    #[serde(rename = "apiKey")]
    pub api_key: Option<String>,
    #[serde(rename = "hasApiKey")]
    pub has_api_key: bool,
    #[serde(rename = "ollamaEndpoint")]
    pub ollama_endpoint: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SaveModelConfigRequest {
    pub provider: String,
    pub model: String,
    #[serde(rename = "whisperModel")]
    pub whisper_model: String,
    #[serde(rename = "apiKey")]
    pub api_key: Option<String>,
    #[serde(rename = "ollamaEndpoint")]
    pub ollama_endpoint: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GetApiKeyRequest {
    pub provider: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TranscriptConfig {
    pub provider: String,
    pub model: String,
    #[serde(rename = "apiKey")]
    pub api_key: Option<String>,
    #[serde(rename = "hasApiKey")]
    pub has_api_key: bool,
}

#[derive(Debug, Serialize)]
pub struct CustomOpenAIConfigView {
    pub endpoint: String,
    #[serde(rename = "apiKey")]
    pub api_key: Option<String>,
    #[serde(rename = "hasApiKey")]
    pub has_api_key: bool,
    pub model: String,
    #[serde(rename = "maxTokens")]
    pub max_tokens: Option<i32>,
    pub temperature: Option<f32>,
    #[serde(rename = "topP")]
    pub top_p: Option<f32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SaveTranscriptConfigRequest {
    pub provider: String,
    pub model: String,
    #[serde(rename = "apiKey")]
    pub api_key: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeleteMeetingRequest {
    pub meeting_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MeetingDetails {
    pub id: String,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
    pub transcripts: Vec<MeetingTranscript>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MeetingTranscript {
    pub id: String,
    pub text: String,
    pub timestamp: String,
    // Recording-relative timestamps for audio-transcript synchronization
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio_start_time: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio_end_time: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<f64>,
}

/// Meeting metadata without transcripts (for pagination)
#[derive(Debug, Serialize, Deserialize)]
pub struct MeetingMetadata {
    pub id: String,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub folder_path: Option<String>,
}

/// Paginated transcripts response with total count
#[derive(Debug, Serialize, Deserialize)]
pub struct PaginatedTranscriptsResponse {
    pub transcripts: Vec<MeetingTranscript>,
    pub total_count: i64,
    pub has_more: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SaveMeetingTitleRequest {
    pub meeting_id: String,
    pub title: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SaveMeetingSummaryRequest {
    pub meeting_id: String,
    pub summary: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SaveTranscriptRequest {
    pub meeting_title: String,
    pub transcripts: Vec<TranscriptSegment>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TranscriptSegment {
    pub id: String,
    pub text: String,
    pub timestamp: String,
    // NEW: Recording-relative timestamps for playback synchronization
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio_start_time: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio_end_time: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<f64>,
}

// Helper function to get auth token from store (optional)
#[allow(dead_code)]
async fn get_auth_token<R: Runtime>(app: &AppHandle<R>) -> Option<String> {
    let store = match app.store("store.json") {
        Ok(store) => store,
        Err(_) => return None,
    };

    match store.get("authToken") {
        Some(token) => {
            if let Some(token_str) = token.as_str() {
                log_info!("Auth token loaded from the secure application store");
                Some(token_str.to_string())
            } else {
                log_warn!("Auth token is not a string");
                None
            }
        }
        None => {
            log_warn!("No auth token found in store");
            None
        }
    }
}

// API Commands for Tauri

#[tauri::command]
pub async fn api_get_meetings<R: Runtime>(
    _app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
    auth_token: Option<String>,
) -> Result<Vec<Meeting>, String> {
    log_info!(
        "api_get_meetings called with auth_token(native) : {}",
        auth_token.is_some()
    );
    let pool = state.db_manager.pool();
    let ctx = crate::context::current();
    let meetings: Result<Vec<MeetingModel>, sqlx::Error> =
        MeetingsRepository::get_meetings(pool, &ctx).await;

    match meetings {
        Ok(meeting_models) => {
            log_info!("Successfully got {} meetings", meeting_models.len());

            let result: Vec<Meeting> = meeting_models
                .into_iter()
                .map(|m| Meeting {
                    id: m.id,
                    title: m.title,
                })
                .collect();
            Ok(result)
        }
        Err(e) => {
            log_error!("Error getting meetings: {}", e);
            Err(e.to_string())
        }
    }
}

#[tauri::command]
pub async fn api_search_transcripts<R: Runtime>(
    _app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
    query: String,
    auth_token: Option<String>,
) -> Result<Vec<TranscriptSearchResult>, String> {
    // Search text is meeting content. Log only non-content metadata.
    log_info!(
        "api_search_transcripts called (query_chars={}, auth_token_present={})",
        query.chars().count(),
        auth_token.is_some()
    );

    let pool = state.db_manager.pool();
    let ctx = crate::context::current();

    match TranscriptsRepository::search_transcripts(pool, &ctx, &query).await {
        Ok(results) => {
            log_info!(
                "Search completed successfully with {} results.",
                results.len()
            );
            Ok(results)
        }
        Err(e) => {
            log_error!("Transcript search failed: {}", e);
            Err(format!("Failed to search transcripts: {}", e))
        }
    }
}

/// Ranked, transcript-segment evidence search used by the Product Intelligence
/// surface. Fully local/offline; no LLM or network provider participates.
#[tauri::command]
pub async fn api_search_evidence<R: Runtime>(
    _app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
    query: String,
) -> Result<Vec<EvidenceSearchResult>, String> {
    // Never log the query or snippets. They are meeting content.
    log_info!(
        "api_search_evidence called (query_chars={})",
        query.chars().count()
    );

    let pool = state.db_manager.pool();
    let ctx = crate::context::current();
    ctx.require(crate::context::Role::Viewer)
        .map_err(|_| "Not authorized to search meeting evidence".to_string())?;

    match TranscriptsRepository::search_evidence(pool, &ctx, &query).await {
        Ok(results) => {
            log_info!(
                "Evidence search completed successfully with {} results",
                results.len()
            );
            Ok(results)
        }
        Err(e) => {
            log_error!("Evidence search failed: {}", e);
            Err(format!("Failed to search meeting evidence: {}", e))
        }
    }
}

#[tauri::command]
pub async fn api_get_model_config<R: Runtime>(
    _app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
    _auth_token: Option<String>,
) -> Result<Option<ModelConfig>, String> {
    log_info!("api_get_model_config called (native)");
    let pool = state.db_manager.pool();
    let ctx = crate::context::current();

    match SettingsRepository::get_model_config(pool, &ctx).await {
        Ok(Some(config)) => {
            log_info!("Found model configuration in the local database");
            match SettingsRepository::get_api_key(pool, &ctx, &config.provider).await {
                Ok(api_key) => {
                    log_info!("Successfully retrieved model configuration");
                    Ok(Some(ModelConfig {
                        provider: config.provider,
                        model: config.model,
                        whisper_model: config.whisper_model,
                        has_api_key: api_key.is_some(),
                        api_key: None,
                        ollama_endpoint: config.ollama_endpoint,
                    }))
                }
                Err(e) => {
                    log_error!(
                        "Failed to get API key for provider {}: {}",
                        &config.provider,
                        e
                    );
                    Err(e.to_string())
                }
            }
        }
        Ok(None) => {
            log_warn!("⚠️ No model config found in database - database may be empty or settings table not initialized");
            Ok(None)
        }
        Err(e) => {
            log_error!("❌ Failed to get model config from database: {}", e);
            Err(e.to_string())
        }
    }
}

#[tauri::command]
pub async fn api_save_model_config<R: Runtime>(
    _app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
    provider: String,
    model: String,
    whisper_model: String,
    api_key: Option<String>,
    ollama_endpoint: Option<String>,
    _auth_token: Option<String>,
) -> Result<serde_json::Value, String> {
    log_info!("api_save_model_config called");
    let pool = state.db_manager.pool();
    let ctx = crate::context::current();
    let ollama_endpoint = if provider.eq_ignore_ascii_case("ollama") {
        ollama_endpoint
            .filter(|value| !value.trim().is_empty())
            .map(|value| crate::summary::validate_llm_endpoint(&value))
            .transpose()?
    } else {
        ollama_endpoint
    };

    if let Err(e) = SettingsRepository::save_model_config(
        pool,
        &ctx,
        &provider,
        &model,
        &whisper_model,
        ollama_endpoint.as_deref(),
    )
    .await
    {
        log_error!("❌ Failed to save model config to database: {}", e);
        return Err(e.to_string());
    }

    // Skip API key saving for custom-openai provider (it uses customOpenAIConfig JSON instead)
    if let Some(key) = api_key {
        if !key.is_empty() && provider != "custom-openai" {
            log_info!("🔑 API key provided, saving...");
            if let Err(e) = SettingsRepository::save_api_key(pool, &ctx, &provider, &key).await {
                log_error!("❌ Failed to save API key: {}", e);
                return Err(e.to_string());
            }
        }
    }

    // Trigger graceful shutdown of built-in AI sidecar if it's running
    // This ensures that if the user switched models/providers, the old one is cleaned up
    // The shutdown happens in the background, so it won't block the UI
    if let Err(e) = crate::summary::summary_engine::client::shutdown_sidecar_gracefully().await {
        log_warn!("Failed to initiate graceful sidecar shutdown: {}", e);
    }

    log_info!("✅ Successfully saved model configuration to database");
    Ok(
        serde_json::json!({ "status": "success", "message": "Model configuration saved successfully" }),
    )
}

#[tauri::command]
pub async fn api_has_api_key<R: Runtime>(
    _app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
    provider: String,
    _auth_token: Option<String>,
) -> Result<bool, String> {
    log_info!("api_has_api_key called");
    let ctx = crate::context::current();
    match SettingsRepository::get_api_key(state.db_manager.pool(), &ctx, &provider).await {
        Ok(key) => Ok(key.is_some()),
        Err(e) => {
            log_error!("Failed to inspect API key presence");
            Err(format!("Failed to inspect API key presence: {}", e))
        }
    }
}

#[tauri::command]
pub async fn api_get_transcript_config<R: Runtime>(
    _app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
    _auth_token: Option<String>,
) -> Result<Option<TranscriptConfig>, String> {
    log_info!("api_get_transcript_config called (native)");
    let pool = state.db_manager.pool();
    let ctx = crate::context::current();

    match SettingsRepository::get_transcript_config(pool, &ctx).await {
        Ok(Some(config)) => {
            log_info!("Found transcript configuration");
            match SettingsRepository::get_transcript_api_key(pool, &ctx, &config.provider).await {
                Ok(api_key) => {
                    log_info!("Successfully retrieved transcript configuration");
                    Ok(Some(TranscriptConfig {
                        provider: config.provider,
                        model: config.model,
                        has_api_key: api_key.is_some(),
                        api_key: None,
                    }))
                }
                Err(e) => {
                    log_error!(
                        "Failed to get transcript API key for provider {}: {}",
                        &config.provider,
                        e
                    );
                    Err(e.to_string())
                }
            }
        }
        Ok(None) => {
            log_info!("No transcript config found, returning default.");
            Ok(Some(TranscriptConfig {
                provider: "parakeet".to_string(),
                model: crate::config::DEFAULT_PARAKEET_MODEL.to_string(),
                api_key: None,
                has_api_key: false,
            }))
        }
        Err(e) => {
            log_error!("Failed to get transcript config: {}", e);
            Err(e.to_string())
        }
    }
}

#[tauri::command]
pub async fn api_save_transcript_config<R: Runtime>(
    _app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
    provider: String,
    model: String,
    api_key: Option<String>,
    _auth_token: Option<String>,
) -> Result<serde_json::Value, String> {
    log_info!(
        "api_save_transcript_config called (native) for provider '{}'",
        &provider
    );
    let pool = state.db_manager.pool();
    let ctx = crate::context::current();

    if let Err(e) = SettingsRepository::save_transcript_config(pool, &ctx, &provider, &model).await
    {
        log_error!("Failed to save transcript config: {}", e);
        return Err(e.to_string());
    }

    if let Some(key) = api_key {
        if !key.is_empty() {
            log_info!("API key provided, saving for transcript provider...");
            if let Err(e) =
                SettingsRepository::save_transcript_api_key(pool, &ctx, &provider, &key).await
            {
                log_error!("Failed to save transcript API key: {}", e);
                return Err(e.to_string());
            }
        }
    }

    log_info!("Successfully saved transcript configuration.");
    Ok(
        serde_json::json!({ "status": "success", "message": "Transcript configuration saved successfully" }),
    )
}

#[tauri::command]
pub async fn api_has_transcript_api_key<R: Runtime>(
    _app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
    provider: String,
    _auth_token: Option<String>,
) -> Result<bool, String> {
    log_info!("api_has_transcript_api_key called");
    let ctx = crate::context::current();
    match SettingsRepository::get_transcript_api_key(state.db_manager.pool(), &ctx, &provider).await
    {
        Ok(key) => Ok(key.is_some()),
        Err(e) => {
            log_error!("Failed to inspect transcript API key presence");
            Err(format!(
                "Failed to inspect transcript API key presence: {}",
                e
            ))
        }
    }
}

#[tauri::command]
pub async fn api_delete_api_key<R: Runtime>(
    _app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
    provider: String,
    _auth_token: Option<String>,
) -> Result<(), String> {
    log_info!(
        "log_api_delete_api_key called (native) for provider '{}'",
        &provider
    );
    let ctx = crate::context::current();
    match SettingsRepository::delete_api_key(state.db_manager.pool(), &ctx, &provider).await {
        Ok(_) => {
            log_info!("Successfully deleted API key for provider '{}'.", &provider);
            Ok(())
        }
        Err(e) => {
            log_error!(
                "Failed to delete API key for provider '{}': {}",
                &provider,
                e
            );
            Err(e.to_string())
        }
    }
}

#[tauri::command]
pub async fn api_delete_meeting<R: Runtime>(
    _app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
    meeting_id: String,
    auth_token: Option<String>,
) -> Result<serde_json::Value, String> {
    log_info!(
        "api_delete_meeting called (native), auth_token_present: {}",
        auth_token.is_some()
    );

    let ctx = crate::context::current();
    ctx.require(crate::context::Role::Member)
        .map_err(|_| "Not authorized to delete meetings".to_string())?;
    // The deletion trust root is compiled from the platform-owned default, never
    // from renderer-controlled or legacy persisted preferences.
    let allowed_recording_roots =
        vec![crate::audio::recording_preferences::get_default_recordings_folder()];

    match state
        .db_manager
        .delete_meeting_verified(&ctx, &meeting_id, allowed_recording_roots)
        .await
    {
        Ok(outcome) => {
            log_info!(
                "Verified local meeting deletion completed (already_absent={}, managed_files_removed={}, retained_user_entries={})",
                outcome.already_absent,
                outcome.artifacts.managed_files_removed,
                outcome.artifacts.retained_user_entries
            );
            Ok(serde_json::json!({
                "status": "success",
                "message": "Mityu-managed local meeting data deleted successfully",
                "already_absent": outcome.already_absent,
                "managed_files_removed": outcome.artifacts.managed_files_removed,
                "retained_user_entries": outcome.artifacts.retained_user_entries,
                "scope": "sqlite_fts_wal_and_mityu_managed_recording_artifacts",
                "storage_limitations": "SSD wear-leveling, copy-on-write snapshots, backups, exports, and other external copies are outside application-controlled deletion."
            }))
        }
        Err(_error) => {
            // The anyhow chain can contain a user-local recording path. Keep
            // diagnostic logs content-free because users may export them when
            // requesting support.
            log_error!("Verified local meeting deletion failed; no success was reported");
            Err(
                "Meeting deletion did not complete; no success was reported. Check that the recording folder is under the configured Mityu recording location, close any application using its files, and retry."
                    .to_string(),
            )
        }
    }
}

#[tauri::command]
pub async fn api_get_meeting<R: Runtime>(
    _app: AppHandle<R>,
    meeting_id: String,
    state: tauri::State<'_, AppState>,
    auth_token: Option<String>,
) -> Result<MeetingDetails, String> {
    log_info!(
        "api_get_meeting called (native), auth_token_present: {}",
        auth_token.is_some()
    );

    let pool = state.db_manager.pool();
    let ctx = crate::context::current();

    match MeetingsRepository::get_meeting(pool, &ctx, &meeting_id).await {
        Ok(Some(meeting)) => {
            log_info!("Successfully retrieved a workspace-scoped meeting");
            Ok(meeting)
        }
        Ok(None) => {
            log_warn!("Workspace-scoped meeting was not found");
            Err("Meeting not found".to_string())
        }
        Err(e) => {
            log_error!("Error retrieving workspace-scoped meeting: {}", e);
            Err(format!("Failed to retrieve meeting: {}", e))
        }
    }
}

/// Get meeting metadata without transcripts (for pagination)
#[tauri::command]
pub async fn api_get_meeting_metadata<R: Runtime>(
    _app: AppHandle<R>,
    meeting_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<MeetingMetadata, String> {
    log_info!("api_get_meeting_metadata called");

    let pool = state.db_manager.pool();
    let ctx = crate::context::current();

    match MeetingsRepository::get_meeting_metadata(pool, &ctx, &meeting_id).await {
        Ok(Some(meeting)) => {
            log_info!("Successfully retrieved workspace-scoped meeting metadata");
            Ok(MeetingMetadata {
                id: meeting.id,
                title: meeting.title,
                created_at: meeting.created_at.0.to_rfc3339(),
                updated_at: meeting.updated_at.0.to_rfc3339(),
                folder_path: meeting.folder_path,
            })
        }
        Ok(None) => {
            log_warn!("Workspace-scoped meeting metadata was not found");
            Err("Meeting not found".to_string())
        }
        Err(e) => {
            log_error!("Error retrieving workspace-scoped meeting metadata: {}", e);
            Err(format!("Failed to retrieve meeting metadata: {}", e))
        }
    }
}

/// Get paginated transcripts for a meeting
#[tauri::command]
pub async fn api_get_meeting_transcripts<R: Runtime>(
    _app: AppHandle<R>,
    meeting_id: String,
    limit: i64,
    offset: i64,
    state: tauri::State<'_, AppState>,
) -> Result<PaginatedTranscriptsResponse, String> {
    log_info!(
        "api_get_meeting_transcripts called (limit={}, offset={})",
        limit,
        offset
    );

    let pool = state.db_manager.pool();
    let ctx = crate::context::current();

    match MeetingsRepository::get_meeting_transcripts_paginated(
        pool,
        &ctx,
        &meeting_id,
        limit,
        offset,
    )
    .await
    {
        Ok((transcripts, total_count)) => {
            log_info!(
                "Successfully retrieved {} transcript segments (total={})",
                transcripts.len(),
                total_count
            );

            // Convert Transcript to MeetingTranscript
            let meeting_transcripts = transcripts
                .into_iter()
                .map(|t| MeetingTranscript {
                    id: t.id,
                    text: t.transcript,
                    timestamp: t.timestamp,
                    audio_start_time: t.audio_start_time,
                    audio_end_time: t.audio_end_time,
                    duration: t.duration,
                })
                .collect::<Vec<_>>();

            let has_more = (offset + meeting_transcripts.len() as i64) < total_count;

            Ok(PaginatedTranscriptsResponse {
                transcripts: meeting_transcripts,
                total_count,
                has_more,
            })
        }
        Err(e) => {
            log_error!("Error retrieving workspace-scoped transcripts: {}", e);
            Err(format!("Failed to retrieve transcripts: {}", e))
        }
    }
}

#[tauri::command]
pub async fn api_save_meeting_title<R: Runtime>(
    _app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
    meeting_id: String,
    title: String,
    auth_token: Option<String>,
) -> Result<serde_json::Value, String> {
    log_info!(
        "api_save_meeting_title called, auth_token_present: {}",
        auth_token.is_some()
    );
    let pool = state.db_manager.pool();
    let ctx = crate::context::current();
    match MeetingsRepository::update_meeting_title(pool, &ctx, &meeting_id, &title).await {
        Ok(true) => {
            log_info!("Successfully saved meeting title");
            Ok(serde_json::json!({"message": "Meeting title saved successfully"}))
        }
        Ok(false) => {
            log_error!("No workspace-scoped meeting found for title update");
            Err("Meeting not found".to_string())
        }
        Err(e) => {
            log_error!("Failed to update meeting {}", e);
            Err(format!("Failed to update meeting: {}", e))
        }
    }
}

#[tauri::command]
pub async fn api_save_transcript<R: Runtime>(
    _app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
    meeting_title: String,
    transcripts: Vec<serde_json::Value>,
    folder_path: Option<String>,
    completion_token: Option<String>,
    auth_token: Option<String>,
) -> Result<serde_json::Value, String> {
    log_info!(
        "api_save_transcript called (segments={}, folder_present={}, completion_token_present={}, auth_token_present={})",
        transcripts.len(),
        folder_path.is_some(),
        completion_token.is_some(),
        auth_token.is_some()
    );

    // Convert serde_json::Value to TranscriptSegment
    let transcripts_to_save: Vec<TranscriptSegment> = transcripts
        .into_iter()
        .map(serde_json::from_value)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| {
            log_error!("Failed to parse transcript segments: {}", e);
            format!(
                "Invalid transcript data format: {}. Please check the data structure.",
                e
            )
        })?;

    log_debug!("Parsed {} transcript segment(s)", transcripts_to_save.len());

    let pool = state.db_manager.pool();
    let ctx = crate::context::current();

    // A local IPC response can be interrupted after SQLite commits. Replaying
    // the same context-bound token returns the original meeting instead of
    // creating a duplicate row or stranding the native post-processing gate.
    if let Some(token) = completion_token.as_deref() {
        if let Some(meeting_id) =
            crate::audio::recording_commands::persisted_meeting_id_for_completion(&ctx, token)
        {
            log_info!("Returning the existing meeting for an idempotent save retry");
            return Ok(serde_json::json!({
                "status": "success",
                "message": "Transcript was already saved successfully",
                "meeting_id": meeting_id
            }));
        }
    }

    // Opt-in PII/keyword redaction BEFORE persistence (BACKLOG C6). Policy is loaded
    // per-workspace and applied at this call boundary — the repository stays a pure
    // writer. Disabled (the default) => `is_active()` is false and segments are
    // untouched, so existing local flows are unchanged. Only `text` is rewritten;
    // segment ids/timestamps (and thus source-linking) are preserved.
    let mut transcripts_to_save = transcripts_to_save;
    match SettingsRepository::get_redaction_config(pool, &ctx).await {
        Ok(cfg) if cfg.is_active() => {
            for seg in transcripts_to_save.iter_mut() {
                seg.text = crate::redaction::redact(&seg.text, &cfg);
            }
            log_info!(
                "Applied redaction to {} transcript segment(s) before persistence",
                transcripts_to_save.len()
            );
        }
        Ok(_) => {} // disabled/no-op: leave segments verbatim
        Err(e) => {
            // Fail safe: do not persist unredacted PII if the policy said to redact
            // but we could not read it. Surface a clear, content-free error.
            log_error!(
                "Failed to load redaction config before saving transcript: {}",
                e
            );
            return Err(format!("Failed to load redaction configuration: {}", e));
        }
    }

    // Bind the database row only to a folder created by the Rust recording
    // pipeline and selected with its opaque, one-time completion token. The
    // renderer-provided path is a legacy display/correlation field and is never
    // a filesystem authority. Recovery/manual saves carry no token and cannot
    // inspect, reserve, or consume the pending recording folder.
    let mut recording_folder_reservation =
        crate::audio::recording_commands::reserve_last_completed_recording_folder(
            &ctx,
            completion_token.as_deref(),
        );
    let trusted_folder_path = recording_folder_reservation
        .as_ref()
        .and_then(|reservation| reservation.folder_path().map(str::to_string));
    if completion_token.is_some() && recording_folder_reservation.is_none() {
        log_warn!(
            "Rejected an invalid, stale, duplicate, or cross-context recording completion token"
        );
        return Err(
            "The recording completion authorization is invalid or already used. The meeting was not saved; retry from the original post-recording flow."
                .to_string(),
        );
    }
    if folder_path.is_some() && completion_token.is_none() {
        log_warn!("Ignored an unverified or stale recording-folder claim");
    }

    // Now, call the repository with the correctly typed data.
    match TranscriptsRepository::save_transcript(
        pool,
        &ctx,
        &meeting_title,
        &transcripts_to_save,
        trusted_folder_path,
    )
    .await
    {
        Ok(meeting_id) => {
            if let Some(reservation) = recording_folder_reservation.take() {
                reservation.commit(&meeting_id);
            }
            log_info!("Successfully saved transcript and created meeting");
            Ok(serde_json::json!({
                "status": "success",
                "message": "Transcript saved successfully",
                "meeting_id": meeting_id
            }))
        }
        Err(e) => {
            log_error!("Error saving workspace-scoped transcript: {}", e);
            Err(format!("Failed to save transcript: {}", e))
        }
    }
}

#[tauri::command]
pub async fn api_acknowledge_recording_post_processing<R: Runtime>(
    app: AppHandle<R>,
    completion_token: String,
) -> Result<(), String> {
    let ctx = crate::context::current();
    ctx.require(crate::context::Role::Member)
        .map_err(|_| "Not authorized to finalize recording post-processing".to_string())?;

    let acknowledged =
        crate::audio::recording_commands::acknowledge_completed_recording_post_processing(
            &ctx,
            &completion_token,
        );
    let no_pending_completion =
        !crate::audio::recording_commands::has_pending_recording_post_processing_for_context(&ctx);

    if acknowledged || no_pending_completion {
        log_info!("Recording post-processing acknowledgement is complete");
        crate::tray::update_tray_menu(&app);
        Ok(())
    } else {
        log_warn!(
            "Rejected an invalid, premature, duplicate, or cross-context post-processing acknowledgement"
        );
        Err("Recording post-processing acknowledgement was rejected".to_string())
    }
}

#[tauri::command]
pub async fn api_get_pending_recording_post_processing() -> Result<Option<serde_json::Value>, String>
{
    let ctx = crate::context::current();
    ctx.require(crate::context::Role::Member)
        .map_err(|_| "Not authorized to inspect recording post-processing".to_string())?;
    crate::audio::recording_commands::pending_recording_post_processing_for_context(&ctx)
        .map(serde_json::to_value)
        .transpose()
        .map_err(|_| "Could not serialize pending recording state".to_string())
}

#[tauri::command]
pub async fn api_abandon_recording_post_processing<R: Runtime>(
    app: AppHandle<R>,
    completion_token: String,
) -> Result<(), String> {
    use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};

    let ctx = crate::context::current();
    ctx.require(crate::context::Role::Member)
        .map_err(|_| "Not authorized to unlock recording recovery".to_string())?;

    let dialog_app = app.clone();
    let confirmed = tauri::async_runtime::spawn_blocking(move || {
        dialog_app
            .dialog()
            .message(
                "The previous recording did not finish saving. Unlocking permits a new recording but abandons automatic audio-folder linking for the interrupted one. Any existing transcript recovery copy is kept; the recording folder may require manual review or cleanup. Continue?",
            )
            .title("Unlock interrupted recording")
            .kind(MessageDialogKind::Warning)
            .buttons(MessageDialogButtons::YesNo)
            .blocking_show()
    })
    .await
    .map_err(|_| "The recovery confirmation is temporarily unavailable".to_string())?;

    if !confirmed {
        return Err("Recording recovery unlock was cancelled".to_string());
    }

    if !crate::audio::recording_commands::abandon_recording_post_processing(&ctx, &completion_token)
    {
        return Err("The interrupted recording is no longer pending".to_string());
    }

    log_warn!("User confirmed abandonment of interrupted recording post-processing");
    crate::tray::update_tray_menu(&app);
    Ok(())
}

/// Opens the meeting's recording folder in the system file explorer
#[tauri::command]
pub async fn open_meeting_folder<R: Runtime>(
    _app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
    meeting_id: String,
) -> Result<(), String> {
    log_info!("open_meeting_folder called");

    let pool = state.db_manager.pool();
    let ctx = crate::context::current();

    // Get meeting with folder_path (workspace-scoped, via the repository)
    let meeting: Option<MeetingModel> =
        MeetingsRepository::get_meeting_metadata(pool, &ctx, &meeting_id)
            .await
            .map_err(|e| format!("Database error: {}", e))?;

    match meeting {
        Some(m) => {
            if let Some(folder_path) = m.folder_path {
                log_info!("Opening the configured meeting folder");

                let allowed_roots =
                    [crate::audio::recording_preferences::get_default_recordings_folder()];
                let canonical_folder =
                    crate::database::deletion::validate_managed_recording_folder(
                        std::path::Path::new(&folder_path),
                        &allowed_roots,
                    )
                    .map_err(|_| {
                        log_warn!("Rejected an untrusted meeting recording folder");
                        "Recording folder is unavailable or outside Mityu's managed storage"
                            .to_string()
                    })?;

                // Open folder based on OS
                #[cfg(target_os = "macos")]
                {
                    std::process::Command::new("open")
                        .arg(&canonical_folder)
                        .spawn()
                        .map_err(|e| format!("Failed to open folder: {}", e))?;
                }

                #[cfg(target_os = "windows")]
                {
                    std::process::Command::new("explorer")
                        .arg(&canonical_folder)
                        .spawn()
                        .map_err(|e| format!("Failed to open folder: {}", e))?;
                }

                #[cfg(target_os = "linux")]
                {
                    std::process::Command::new("xdg-open")
                        .arg(&canonical_folder)
                        .spawn()
                        .map_err(|e| format!("Failed to open folder: {}", e))?;
                }

                log_info!("Successfully opened the configured meeting folder");
                Ok(())
            } else {
                log_warn!("Meeting has no recording folder configured");
                Err("Recording folder path not available for this meeting".to_string())
            }
        }
        None => {
            log_warn!("Workspace-scoped meeting was not found");
            Err("Meeting not found".to_string())
        }
    }
}

fn validate_external_url(url: &str) -> Result<url::Url, String> {
    const ALLOWED_HOSTS: &[&str] = &[
        "bluedev.dev",
        "www.bluedev.dev",
        "buy.polar.sh",
        "ffmpeg.org",
        "www.ffmpeg.org",
        "github.com",
        "huggingface.co",
        "ai.google.dev",
        "ollama.com",
    ];

    if url
        .chars()
        .any(|character| matches!(character, '&' | '|' | ';' | '<' | '>' | '`'))
    {
        return Err("External link contains unsupported characters".to_string());
    }
    let parsed = url::Url::parse(url.trim())
        .map_err(|_| "External link must be a valid HTTPS URL".to_string())?;
    if parsed.scheme() != "https"
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.port().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return Err("External link is not permitted".to_string());
    }
    let host = parsed
        .host_str()
        .ok_or_else(|| "External link host is required".to_string())?;
    if !ALLOWED_HOSTS
        .iter()
        .any(|allowed| host.eq_ignore_ascii_case(allowed))
    {
        return Err("External link host is not permitted".to_string());
    }
    Ok(parsed)
}

#[tauri::command]
pub async fn open_external_url(url: String) -> Result<(), String> {
    use std::process::Command;
    let url = validate_external_url(&url)?;

    let result = if cfg!(target_os = "windows") {
        Command::new("explorer.exe").arg(url.as_str()).spawn()
    } else if cfg!(target_os = "macos") {
        Command::new("open").arg(url.as_str()).spawn()
    } else {
        // Linux and other Unix-like systems
        Command::new("xdg-open").arg(url.as_str()).spawn()
    };

    match result {
        Ok(_) => Ok(()),
        Err(_) => Err("Failed to open the external link".to_string()),
    }
}

#[cfg(test)]
mod external_url_tests {
    use super::validate_external_url;

    #[test]
    fn external_links_require_https_and_an_allowlisted_host() {
        assert!(validate_external_url("https://ollama.com/download").is_ok());
        assert!(validate_external_url("https://github.com/aydogandagidir/mityu").is_ok());
        assert!(validate_external_url("https://ai.google.dev/gemma/terms").is_ok());
        assert!(validate_external_url("http://ollama.com/download").is_err());
        assert!(validate_external_url("https://evil.example/download").is_err());
        assert!(validate_external_url("file:///etc/passwd").is_err());
    }

    #[test]
    fn external_links_reject_shell_and_secret_channels() {
        assert!(validate_external_url("https://ollama.com/download&calc.exe").is_err());
        assert!(validate_external_url("https://ollama.com/download|calc.exe").is_err());
        assert!(validate_external_url("https://user:pass@ollama.com/download").is_err());
        assert!(validate_external_url("https://ollama.com/download?token=secret").is_err());
        assert!(validate_external_url("custom:payload").is_err());
    }
}

// ===== CUSTOM OPENAI API COMMANDS =====

/// Saves the custom OpenAI configuration
/// Non-secret configuration is stored as JSON; apiKey lives only in the OS keychain.
#[tauri::command]
pub async fn api_save_custom_openai_config<R: Runtime>(
    _app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
    endpoint: String,
    api_key: Option<String>,
    model: String,
    max_tokens: Option<i32>,
    temperature: Option<f32>,
    top_p: Option<f32>,
) -> Result<serde_json::Value, String> {
    log_info!("api_save_custom_openai_config called");

    // Validate required fields
    if endpoint.trim().is_empty() {
        return Err("Endpoint URL is required".to_string());
    }
    if model.trim().is_empty() {
        return Err("Model name is required".to_string());
    }

    let endpoint = crate::summary::validate_custom_openai_endpoint(&endpoint)?;

    // Validate optional numeric parameters
    if let Some(temp) = temperature {
        if !(0.0..=2.0).contains(&temp) {
            return Err("Temperature must be between 0.0 and 2.0".to_string());
        }
    }
    if let Some(top) = top_p {
        if !(0.0..=1.0).contains(&top) {
            return Err("Top P must be between 0.0 and 1.0".to_string());
        }
    }
    if let Some(tokens) = max_tokens {
        if tokens < 1 {
            return Err("Max tokens must be at least 1".to_string());
        }
    }

    let config = CustomOpenAIConfig {
        endpoint,
        api_key: api_key.filter(|k| !k.trim().is_empty()),
        model: model.trim().to_string(),
        max_tokens,
        temperature,
        top_p,
    };

    let pool = state.db_manager.pool();
    let ctx = crate::context::current();

    match SettingsRepository::save_custom_openai_config(pool, &ctx, &config).await {
        Ok(()) => {
            log_info!("Successfully saved custom OpenAI config");
            Ok(serde_json::json!({
                "status": "success",
                "message": "Custom OpenAI configuration saved successfully"
            }))
        }
        Err(e) => {
            log_error!("❌ Failed to save custom OpenAI config: {}", e);
            Err(format!("Failed to save custom OpenAI configuration: {}", e))
        }
    }
}

/// Gets the custom OpenAI configuration
#[tauri::command]
pub async fn api_get_custom_openai_config<R: Runtime>(
    _app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
) -> Result<Option<CustomOpenAIConfigView>, String> {
    log_info!("api_get_custom_openai_config called");

    let pool = state.db_manager.pool();
    let ctx = crate::context::current();

    match SettingsRepository::get_custom_openai_config(pool, &ctx).await {
        Ok(config) => {
            if config.is_some() {
                log_info!("Found custom OpenAI config");
            } else {
                log_info!("No custom OpenAI config found");
            }
            Ok(config.map(|config| CustomOpenAIConfigView {
                endpoint: config.endpoint,
                has_api_key: config.api_key.is_some(),
                api_key: None,
                model: config.model,
                max_tokens: config.max_tokens,
                temperature: config.temperature,
                top_p: config.top_p,
            }))
        }
        Err(e) => {
            log_error!("❌ Failed to get custom OpenAI config: {}", e);
            Err(format!("Failed to get custom OpenAI configuration: {}", e))
        }
    }
}

/// Gets the current workspace's redaction policy (BACKLOG C6). Returns the
/// disabled default when none is stored, so the frontend can render the toggle in
/// its off state without special-casing "not configured". Redaction config is not
/// identity, so the config value legitimately round-trips through the frontend.
#[tauri::command]
pub async fn api_get_redaction_config<R: Runtime>(
    _app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
) -> Result<crate::redaction::RedactionConfig, String> {
    log_info!("api_get_redaction_config called");

    let pool = state.db_manager.pool();
    let ctx = crate::context::current();

    match SettingsRepository::get_redaction_config(pool, &ctx).await {
        Ok(cfg) => {
            // Log only non-sensitive shape (never the custom terms themselves).
            log_info!(
                "Loaded redaction config: enabled={}, default_patterns={}, custom_terms={}",
                cfg.enabled,
                cfg.use_default_patterns,
                cfg.custom_terms.len()
            );
            Ok(cfg)
        }
        Err(e) => {
            log_error!("Failed to get redaction config: {}", e);
            Err(format!("Failed to get redaction configuration: {}", e))
        }
    }
}

/// Persists the current workspace's redaction policy (BACKLOG C6). Enabling this is
/// opt-in; when enabled, transcript text is redacted before DB persistence and
/// before it reaches a summary LLM provider. The custom terms are never logged.
#[tauri::command]
pub async fn api_set_redaction_config<R: Runtime>(
    _app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
    config: crate::redaction::RedactionConfig,
) -> Result<serde_json::Value, String> {
    log_info!(
        "api_set_redaction_config called: enabled={}, default_patterns={}, custom_terms={}",
        config.enabled,
        config.use_default_patterns,
        config.custom_terms.len()
    );

    let pool = state.db_manager.pool();
    let ctx = crate::context::current();

    match SettingsRepository::set_redaction_config(pool, &ctx, &config).await {
        Ok(()) => Ok(serde_json::json!({
            "status": "success",
            "message": "Redaction configuration saved"
        })),
        Err(e) => {
            log_error!("Failed to save redaction config: {}", e);
            Err(format!("Failed to save redaction configuration: {}", e))
        }
    }
}

/// Tests the connection to a custom OpenAI-compatible endpoint
/// Makes a minimal request to verify the endpoint is reachable and responds correctly
#[tauri::command]
pub async fn api_test_custom_openai_connection<R: Runtime>(
    _app: AppHandle<R>,
    state: tauri::State<'_, AppState>,
    endpoint: String,
    api_key: Option<String>,
    model: String,
) -> Result<serde_json::Value, String> {
    log_info!("api_test_custom_openai_connection called");

    let endpoint = crate::summary::validate_custom_openai_endpoint(&endpoint)?;

    // Build the URL - append /chat/completions to the base endpoint
    let url = format!("{}/chat/completions", endpoint.trim_end_matches('/'));

    // Create a minimal test request
    let test_request = serde_json::json!({
        "model": model,
        "messages": [
            {
                "role": "user",
                "content": "Hi"
            }
        ],
        "max_tokens": 5
    });

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let mut request = client
        .post(&url)
        .header("Content-Type", "application/json")
        .json(&test_request);

    // A newly entered key may be tested once. Otherwise use the credential
    // store internally; a persisted secret is never returned to the WebView.
    let api_key = match api_key.filter(|key| !key.trim().is_empty()) {
        Some(key) => Some(key),
        None => SettingsRepository::get_api_key(
            state.db_manager.pool(),
            &crate::context::current(),
            "custom-openai",
        )
        .await
        .map_err(|_| "Could not access the stored custom endpoint credential".to_string())?,
    };
    if let Some(key) = api_key {
        request = request.header("Authorization", format!("Bearer {}", key));
    }

    match request.send().await {
        Ok(response) => {
            let status = response.status();
            let response_text = response.text().await.unwrap_or_default();

            if status.is_success() {
                // Parse response as JSON to verify it's a valid OpenAI-compatible response
                match serde_json::from_str::<serde_json::Value>(&response_text) {
                    Ok(json) => {
                        // Verify the response has the expected OpenAI structure
                        if let Some(choices) = json.get("choices") {
                            if let Some(choices_array) = choices.as_array() {
                                if !choices_array.is_empty() {
                                    // Verify the first choice has the required message structure
                                    if let Some(first_choice) = choices_array.get(0) {
                                        // Check if message.content field exists (can be empty string)
                                        let has_message_structure = first_choice
                                            .get("message")
                                            .and_then(|m| {
                                                m.get("content")
                                                    .or_else(|| m.get("reasoning_content"))
                                            })
                                            .is_some();

                                        if has_message_structure {
                                            log_info!("✅ Custom OpenAI connection test successful - response validated");
                                            return Ok(serde_json::json!({
                                                "status": "success",
                                                "message": "Connection successful and response validated",
                                                "http_status": status.as_u16()
                                            }));
                                        }
                                    }
                                }
                            }
                        }

                        // Response was 200 but doesn't match OpenAI format
                        log_warn!(
                            "Endpoint returned success but not the expected OpenAI response shape"
                        );
                        Err("Endpoint is reachable but doesn't appear to be OpenAI-compatible. Response is missing 'choices' array or 'message.content' / 'message.reasoning_content' field.".to_string())
                    }
                    Err(_error) => {
                        log_warn!("Endpoint returned success but invalid JSON");
                        Err("Endpoint is reachable but returned invalid JSON".to_string())
                    }
                }
            } else {
                log_warn!(
                    "Custom OpenAI connection test failed with status {}",
                    status
                );
                Err(format!("Connection failed with status {}", status))
            }
        }
        Err(e) => {
            log_error!("Custom OpenAI connection test failed");
            if e.is_timeout() {
                Err("Connection timed out. Please check the endpoint URL.".to_string())
            } else if e.is_connect() {
                Err("Could not connect to endpoint. Please verify the URL is correct and the server is running.".to_string())
            } else {
                Err("Connection failed".to_string())
            }
        }
    }
}
