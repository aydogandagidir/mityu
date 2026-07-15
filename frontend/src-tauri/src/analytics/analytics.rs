use chrono::{DateTime, Utc};
use posthog_rs::{Client, Event};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

const PROFILE_PROPERTY_KEYS: &[&str] = &["platform", "architecture"];

fn is_valid_installation_id(value: &str) -> bool {
    value.len() <= 96
        && value.strip_prefix("user_").is_some_and(|suffix| {
            suffix.len() >= 6
                && suffix.chars().all(|character| {
                    character.is_ascii_alphanumeric() || matches!(character, '-' | '_')
                })
        })
}

fn is_allowed_event(event_name: &str) -> bool {
    matches!(
        event_name,
        "analytics_disabled"
            | "analytics_enabled"
            | "analytics_transparency_viewed"
            | "app_started"
            | "auto_save_recording_toggled"
            | "backend_connection"
            | "beta_feature_toggled"
            | "button_click"
            | "custom_prompt_used"
            | "daily_active_user"
            | "default_devices_changed"
            | "enhance_transcript_completed"
            | "enhance_transcript_started"
            | "error"
            | "feature_used"
            | "import_audio_completed"
            | "import_audio_started"
            | "language_selected"
            | "meeting_completed"
            | "meeting_deleted"
            | "meeting_ended"
            | "meeting_started"
            | "microphone_selected"
            | "model_changed"
            | "notification_settings_changed"
            | "page_view"
            | "preferences_viewed"
            | "recording_notification_preference_changed"
            | "recording_started"
            | "recording_stopped"
            | "session_ended"
            | "session_started"
            | "settings_changed"
            | "storage_folder_opened"
            | "summary_copied"
            | "summary_exported"
            | "summary_generation_completed"
            | "summary_generation_started"
            | "summary_regenerated"
            | "system_audio_selected"
            | "transcript_copied"
            | "transcription_error"
            | "transcription_success"
            | "user_activated"
            | "user_first_launch"
            | "user_id_copied"
    )
}

fn is_allowed_event_property(event_name: &str, key: &str) -> bool {
    if matches!(key, "app_version" | "session_duration") {
        return true;
    }

    match event_name {
        "auto_save_recording_toggled" | "recording_notification_preference_changed" => {
            key == "enabled"
        }
        "backend_connection" => key == "success",
        "beta_feature_toggled" => matches!(key, "feature" | "enabled"),
        "button_click" => matches!(key, "button" | "location"),
        "custom_prompt_used" => key == "prompt_length",
        "default_devices_changed" => {
            matches!(
                key,
                "has_preferred_microphone" | "has_preferred_system_audio"
            )
        }
        "enhance_transcript_completed" | "import_audio_completed" => {
            matches!(key, "success" | "duration_seconds" | "segments_count")
        }
        "enhance_transcript_started" => {
            matches!(key, "language" | "model_provider" | "model_family")
        }
        "error" => key == "error_type",
        "feature_used" => matches!(
            key,
            "feature_name" | "is_first_use" | "platform" | "architecture"
        ),
        "import_audio_started" => matches!(
            key,
            "file_size_bytes" | "duration_seconds" | "language" | "model_provider" | "model_family"
        ),
        "language_selected" => {
            matches!(
                key,
                "language_code" | "is_auto_detect" | "is_auto_translate"
            )
        }
        "meeting_completed" => matches!(
            key,
            "duration_seconds"
                | "transcript_segments"
                | "transcript_word_count"
                | "words_per_minute"
                | "meetings_today"
                | "day_of_week"
                | "hour_of_day"
                | "platform"
                | "architecture"
        ),
        "meeting_ended" => matches!(
            key,
            "transcription_provider"
                | "transcription_model_family"
                | "summary_provider"
                | "summary_model_family"
                | "total_duration_seconds"
                | "active_duration_seconds"
                | "pause_duration_seconds"
                | "microphone_device_type"
                | "system_audio_device_type"
                | "chunks_processed"
                | "transcript_segments_count"
                | "had_fatal_error"
        ),
        "microphone_selected" => {
            matches!(key, "device_category" | "is_bluetooth" | "has_system_audio")
        }
        "model_changed" => {
            matches!(
                key,
                "old_provider" | "old_model_family" | "new_provider" | "new_model_family"
            )
        }
        "notification_settings_changed" | "preferences_viewed" => key == "notifications_enabled",
        "page_view" => key == "page",
        "recording_stopped" => key == "duration_seconds",
        "session_ended" => matches!(
            key,
            "session_duration"
                | "session_duration_seconds"
                | "meetings_in_session"
                | "platform"
                | "architecture"
        ),
        "session_started" => matches!(
            key,
            "days_since_last_meeting" | "total_meetings" | "platform" | "architecture"
        ),
        "settings_changed" => key == "setting_type",
        "storage_folder_opened" => key == "folder_type",
        "summary_copied" | "transcript_copied" => matches!(
            key,
            "copy_type"
                | "copy_count_today"
                | "transcript_length"
                | "word_count"
                | "has_markdown"
                | "platform"
                | "architecture"
        ),
        "summary_exported" => matches!(key, "export_format" | "platform" | "architecture"),
        "summary_generation_completed" => matches!(
            key,
            "model_provider" | "model_family" | "success" | "duration_seconds"
        ),
        "summary_generation_started" => matches!(
            key,
            "model_provider"
                | "model_family"
                | "transcript_length"
                | "time_since_recording_minutes"
                | "platform"
                | "architecture"
        ),
        "summary_regenerated" => matches!(key, "model_provider" | "model_family"),
        "system_audio_selected" => {
            matches!(key, "device_category" | "is_bluetooth" | "has_microphone")
        }
        "transcription_success" => key == "duration",
        "user_activated" => matches!(key, "meetings_count" | "days_since_install"),
        _ => false,
    }
}

fn is_boolean_property(key: &str) -> bool {
    matches!(
        key,
        "enabled"
            | "had_fatal_error"
            | "has_markdown"
            | "has_microphone"
            | "has_preferred_microphone"
            | "has_preferred_system_audio"
            | "has_system_audio"
            | "is_auto_detect"
            | "is_auto_translate"
            | "is_bluetooth"
            | "is_first_use"
            | "notifications_enabled"
            | "success"
    )
}

fn is_numeric_property(key: &str) -> bool {
    matches!(
        key,
        "active_duration_seconds"
            | "chunks_processed"
            | "copy_count_today"
            | "day_of_week"
            | "days_since_install"
            | "days_since_last_meeting"
            | "duration"
            | "duration_seconds"
            | "file_size_bytes"
            | "hour_of_day"
            | "meetings_in_session"
            | "meetings_today"
            | "meetings_count"
            | "pause_duration_seconds"
            | "prompt_length"
            | "segments_count"
            | "session_duration"
            | "session_duration_seconds"
            | "time_since_recording_minutes"
            | "total_duration_seconds"
            | "total_meetings"
            | "transcript_length"
            | "transcript_segments"
            | "transcript_segments_count"
            | "transcript_word_count"
            | "word_count"
            | "words_per_minute"
    )
}

fn bucket_model_family(raw_value: &str) -> Option<String> {
    let value = raw_value.trim().to_ascii_lowercase();
    if value.is_empty() || value.len() > 256 || value.chars().any(char::is_control) {
        return None;
    }

    let family = if value.contains("parakeet") {
        "parakeet"
    } else if value.contains("whisper") {
        "whisper"
    } else if value.contains("llama") {
        "llama"
    } else if value.contains("qwen") {
        "qwen"
    } else if value.contains("gemma") {
        "gemma"
    } else if value.contains("mistral") || value.contains("mixtral") {
        "mistral"
    } else if value.contains("deepseek") {
        "deepseek"
    } else if value.contains("claude") {
        "claude"
    } else if value.contains("gpt")
        || value.starts_with("o1")
        || value.starts_with("o3")
        || value.starts_with("o4")
    {
        "openai"
    } else if value.contains("phi") {
        "phi"
    } else if value.contains("command-r") {
        "command-r"
    } else {
        "custom"
    };

    Some(family.to_string())
}

fn bucket_language(raw_value: &str) -> Option<String> {
    let value = raw_value.trim();
    if !value.is_ascii() {
        return None;
    }
    match value {
        "auto" | "auto-translate" | "specified" => Some(value.to_string()),
        _ if (value.len() == 2
            && value
                .chars()
                .all(|character| character.is_ascii_lowercase()))
            || (value.len() == 5
                && value.as_bytes()[2] == b'-'
                && value[..2]
                    .chars()
                    .all(|character| character.is_ascii_lowercase())
                && value[3..]
                    .chars()
                    .all(|character| character.is_ascii_alphabetic())) =>
        {
            Some("specified".to_string())
        }
        _ => None,
    }
}

fn sanitize_categorical_value(key: &str, value: &str) -> Option<String> {
    let allowed = match key {
        "app_version" => {
            let safe = value == "unknown"
                || (value.len() <= 32
                    && value.starts_with(|character: char| character.is_ascii_digit())
                    && value.chars().all(|character| {
                        character.is_ascii_digit() || matches!(character, '.' | '-' | '+')
                    }));
            return safe.then(|| value.to_string());
        }
        "platform" => &["Windows", "macOS", "Linux", "tauri", "unknown"][..],
        "architecture" => &["x86", "x86_64", "aarch64", "unknown"][..],
        "device_category" => &["default", "airpods", "bluetooth", "wired", "unknown"][..],
        "microphone_device_type" | "system_audio_device_type" => {
            &["Bluetooth", "Wired", "Unknown"][..]
        }
        "copy_type" => &["transcript", "summary"][..],
        "export_format" => &["markdown", "docx", "pdf"][..],
        "folder_type" => &["database", "models", "recordings"][..],
        "setting_type" => &["model_config", "transcript_config"][..],
        "model_provider"
        | "old_provider"
        | "new_provider"
        | "transcription_provider"
        | "summary_provider" => &[
            "ollama",
            "groq",
            "claude",
            "anthropic",
            "openai",
            "openrouter",
            "builtin-ai",
            "custom-openai",
            "parakeet",
            "whisper",
            "local",
            "none",
            "unknown",
        ][..],
        "model_family"
        | "old_model_family"
        | "new_model_family"
        | "transcription_model_family"
        | "summary_model_family" => &[
            "parakeet",
            "whisper",
            "llama",
            "qwen",
            "gemma",
            "mistral",
            "deepseek",
            "claude",
            "openai",
            "phi",
            "command-r",
            "custom",
        ][..],
        "feature" => &["importAndRetranscribe", "structuredSummaries"][..],
        "feature_name" => &["template_selected"][..],
        "page" => &["home", "meeting_details"][..],
        "button" => &[
            "copy_summary",
            "copy_transcript",
            "edit_meeting_title",
            "enhance_transcript",
            "find_in_summary",
            "generate_summary",
            "open_recording_folder",
            "pause_recording",
            "recording_notification_acknowledged",
            "regenerate_summary",
            "replay_product_tour",
            "resume_recording",
            "save_changes",
            "start_recording",
            "start_recording_blocked_downloading",
            "start_recording_blocked_license",
            "start_recording_blocked_missing",
            "start_recording_error",
            "stop_recording",
            "stop_summary_generation",
            "view_meeting_from_toast",
        ][..],
        "location" => &[
            "home_page",
            "meeting_details",
            "recording_complete",
            "recording_controls",
            "settings",
            "sidebar",
            "sidebar_auto",
            "sidebar_direct",
            "toast",
        ][..],
        "error_type" => &["import_audio_failed", "enhance_transcript_failed"][..],
        "language" | "language_code" => return bucket_language(value),
        _ => return None,
    };

    allowed.contains(&value).then(|| value.to_string())
}

fn sanitize_property_value(key: &str, raw_value: String) -> Option<String> {
    let value = raw_value.trim();
    if value.is_empty() || value.len() > 96 || value.chars().any(char::is_control) {
        return None;
    }

    if is_boolean_property(key) {
        return matches!(value, "true" | "false").then(|| value.to_string());
    }

    if is_numeric_property(key) {
        if matches!(key, "days_since_install" | "days_since_last_meeting") && value == "null" {
            return Some(value.to_string());
        }
        return value
            .parse::<f64>()
            .ok()
            .filter(|number| number.is_finite())
            .map(|_| value.to_string());
    }

    if matches!(
        key,
        "model_family"
            | "old_model_family"
            | "new_model_family"
            | "transcription_model_family"
            | "summary_model_family"
    ) {
        let bucketed = bucket_model_family(value)?;
        return sanitize_categorical_value(key, &bucketed);
    }

    sanitize_categorical_value(key, value)
}

fn sanitize_event_properties(
    event_name: &str,
    properties: HashMap<String, String>,
) -> Result<HashMap<String, String>, String> {
    if !is_allowed_event(event_name) {
        return Err("Analytics event is not permitted".to_string());
    }

    Ok(properties
        .into_iter()
        .filter(|(key, _)| is_allowed_event_property(event_name, key))
        .filter_map(|(key, value)| sanitize_property_value(&key, value).map(|value| (key, value)))
        .collect())
}

fn sanitize_profile_properties(properties: HashMap<String, String>) -> HashMap<String, String> {
    properties
        .into_iter()
        .filter(|(key, _)| PROFILE_PROPERTY_KEYS.contains(&key.as_str()))
        .filter_map(|(key, value)| sanitize_property_value(&key, value).map(|value| (key, value)))
        .collect()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyticsConfig {
    pub api_key: String,
    pub host: Option<String>,
    pub enabled: bool,
}

impl Default for AnalyticsConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            host: Some("https://us.i.posthog.com".to_string()),
            enabled: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSession {
    pub session_id: String,
    pub user_id: String,
    pub start_time: DateTime<Utc>,
    pub is_active: bool,
}

impl UserSession {
    pub fn new(user_id: String) -> Self {
        let now = Utc::now();
        Self {
            session_id: format!("session_{}", Uuid::new_v4()),
            user_id,
            start_time: now,
            is_active: true,
        }
    }

    pub fn duration_seconds(&self) -> i64 {
        (Utc::now() - self.start_time).num_seconds()
    }
}

pub struct AnalyticsClient {
    client: Option<Arc<Client>>,
    config: AnalyticsConfig,
    user_id: Arc<Mutex<Option<String>>>,
    current_session: Arc<Mutex<Option<UserSession>>>,
}

impl AnalyticsClient {
    pub async fn new(config: AnalyticsConfig) -> Self {
        let client = if config.enabled && !config.api_key.is_empty() {
            Some(Arc::new(posthog_rs::client(config.api_key.as_str()).await))
        } else {
            None
        };

        Self {
            client,
            config,
            user_id: Arc::new(Mutex::new(None)),
            current_session: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn identify(
        &self,
        user_id: String,
        properties: Option<HashMap<String, String>>,
    ) -> Result<(), String> {
        if !is_valid_installation_id(&user_id) {
            return Err("Analytics installation identifier is not permitted".to_string());
        }

        let mut properties = sanitize_profile_properties(properties.unwrap_or_default());
        properties.insert(
            "app_version".to_string(),
            env!("CARGO_PKG_VERSION").to_string(),
        );

        let client = match &self.client {
            Some(client) => Arc::clone(client),
            None => return Ok(()),
        };

        // Store user ID for future events
        *self.user_id.lock().await = Some(user_id.clone());

        let mut event = Event::new("$identify", &user_id);
        let _ = event.insert_prop("$geoip_disable", true);

        // Add user properties
        for (key, value) in properties {
            if let Err(e) = event.insert_prop(&key, value) {
                eprintln!("Failed to add property {}: {}", key, e);
            }
        }

        client.capture(event);

        Ok(())
    }

    pub async fn track_event(
        &self,
        event_name: &str,
        properties: Option<HashMap<String, String>>,
    ) -> Result<(), String> {
        let mut properties = sanitize_event_properties(event_name, properties.unwrap_or_default())?;

        let client = match &self.client {
            Some(client) => Arc::clone(client),
            None => return Ok(()),
        };

        let user_id = match self.user_id.lock().await.clone() {
            Some(id) => id,
            None => {
                // Don't create anonymous users, wait for proper identification
                log::warn!(
                    "Attempted to track event '{}' before user identification",
                    event_name
                );
                return Ok(());
            }
        };

        let event_name = event_name.to_string();

        // Add app version to all events
        properties.insert(
            "app_version".to_string(),
            env!("CARGO_PKG_VERSION").to_string(),
        );

        // Include only a coarse session duration. Session IDs remain local.
        if let Some(session) = self.current_session.lock().await.as_ref() {
            properties.insert(
                "session_duration".to_string(),
                session.duration_seconds().to_string(),
            );
        }

        let mut event = Event::new(&event_name, &user_id);
        let _ = event.insert_prop("$geoip_disable", true);

        // Add event properties
        for (key, value) in properties {
            if let Err(e) = event.insert_prop(&key, value) {
                log::warn!("Failed to add property {}: {}", key, e);
            }
        }

        client.capture(event);

        Ok(())
    }

    // Enhanced user tracking methods
    pub async fn start_session(&self, user_id: String) -> Result<String, String> {
        let session = UserSession::new(user_id.clone());
        let session_id = session.session_id.clone();

        *self.current_session.lock().await = Some(session);

        self.track_event("session_started", None).await?;

        Ok(session_id)
    }

    pub async fn end_session(&self) -> Result<(), String> {
        let session = self.current_session.lock().await.take();

        if let Some(session) = session {
            let mut properties = HashMap::new();
            properties.insert(
                "session_duration".to_string(),
                session.duration_seconds().to_string(),
            );

            self.track_event("session_ended", Some(properties)).await?;
        }

        Ok(())
    }

    pub async fn track_daily_active_user(&self) -> Result<(), String> {
        self.track_event("daily_active_user", None).await
    }

    pub async fn track_user_first_launch(&self) -> Result<(), String> {
        self.track_event("user_first_launch", None).await
    }

    pub async fn get_current_session(&self) -> Option<UserSession> {
        self.current_session.lock().await.clone()
    }

    pub async fn is_session_active(&self) -> bool {
        self.current_session.lock().await.is_some()
    }

    // Meeting-specific event tracking methods
    pub async fn track_meeting_started(&self) -> Result<(), String> {
        self.track_event("meeting_started", None).await
    }

    pub async fn track_recording_started(&self) -> Result<(), String> {
        self.track_event("recording_started", None).await
    }

    pub async fn track_recording_stopped(
        &self,
        duration_seconds: Option<u64>,
    ) -> Result<(), String> {
        let mut properties = HashMap::new();

        if let Some(duration) = duration_seconds {
            properties.insert("duration_seconds".to_string(), duration.to_string());
        }

        self.track_event("recording_stopped", Some(properties))
            .await
    }

    pub async fn track_meeting_deleted(&self) -> Result<(), String> {
        self.track_event("meeting_deleted", None).await
    }

    pub async fn track_settings_changed(&self, setting_type: &str) -> Result<(), String> {
        let mut properties = HashMap::new();
        properties.insert("setting_type".to_string(), setting_type.to_string());

        self.track_event("settings_changed", Some(properties)).await
    }

    pub async fn track_app_started(&self, version: &str) -> Result<(), String> {
        let mut properties = HashMap::new();
        properties.insert("app_version".to_string(), version.to_string());

        self.track_event("app_started", Some(properties)).await
    }

    pub async fn track_feature_used(&self, feature_name: &str) -> Result<(), String> {
        let mut properties = HashMap::new();
        properties.insert("feature_name".to_string(), feature_name.to_string());

        self.track_event("feature_used", Some(properties)).await
    }

    // Summary generation analytics
    pub async fn track_summary_generation_started(
        &self,
        model_provider: &str,
        model_name: &str,
        transcript_length: usize,
    ) -> Result<(), String> {
        let mut properties = HashMap::new();
        properties.insert("model_provider".to_string(), model_provider.to_string());
        if let Some(model_family) = bucket_model_family(model_name) {
            properties.insert("model_family".to_string(), model_family);
        }
        properties.insert(
            "transcript_length".to_string(),
            transcript_length.to_string(),
        );

        self.track_event("summary_generation_started", Some(properties))
            .await
    }

    pub async fn track_summary_generation_completed(
        &self,
        model_provider: &str,
        model_name: &str,
        success: bool,
        duration_seconds: Option<u64>,
    ) -> Result<(), String> {
        let mut properties = HashMap::new();
        properties.insert("model_provider".to_string(), model_provider.to_string());
        if let Some(model_family) = bucket_model_family(model_name) {
            properties.insert("model_family".to_string(), model_family);
        }
        properties.insert("success".to_string(), success.to_string());

        if let Some(duration) = duration_seconds {
            properties.insert("duration_seconds".to_string(), duration.to_string());
        }

        self.track_event("summary_generation_completed", Some(properties))
            .await
    }

    pub async fn track_summary_regenerated(
        &self,
        model_provider: &str,
        model_name: &str,
    ) -> Result<(), String> {
        let mut properties = HashMap::new();
        properties.insert("model_provider".to_string(), model_provider.to_string());
        if let Some(model_family) = bucket_model_family(model_name) {
            properties.insert("model_family".to_string(), model_family);
        }

        self.track_event("summary_regenerated", Some(properties))
            .await
    }

    pub async fn track_model_changed(
        &self,
        old_provider: &str,
        old_model: &str,
        new_provider: &str,
        new_model: &str,
    ) -> Result<(), String> {
        let mut properties = HashMap::new();
        properties.insert("old_provider".to_string(), old_provider.to_string());
        properties.insert("new_provider".to_string(), new_provider.to_string());
        if let Some(model_family) = bucket_model_family(old_model) {
            properties.insert("old_model_family".to_string(), model_family);
        }
        if let Some(model_family) = bucket_model_family(new_model) {
            properties.insert("new_model_family".to_string(), model_family);
        }

        self.track_event("model_changed", Some(properties)).await
    }

    pub async fn track_custom_prompt_used(&self, prompt_length: usize) -> Result<(), String> {
        let mut properties = HashMap::new();
        properties.insert("prompt_length".to_string(), prompt_length.to_string());

        self.track_event("custom_prompt_used", Some(properties))
            .await
    }

    pub async fn track_meeting_ended(
        &self,
        transcription_provider: &str,
        transcription_model: &str,
        summary_provider: &str,
        summary_model: &str,
        total_duration_seconds: Option<f64>,
        active_duration_seconds: f64,
        pause_duration_seconds: f64,
        microphone_device_type: &str,
        system_audio_device_type: &str,
        chunks_processed: u64,
        transcript_segments_count: u64,
        had_fatal_error: bool,
    ) -> Result<(), String> {
        let mut properties = HashMap::new();

        // Model information
        properties.insert(
            "transcription_provider".to_string(),
            transcription_provider.to_string(),
        );
        properties.insert("summary_provider".to_string(), summary_provider.to_string());
        if let Some(model_family) = bucket_model_family(transcription_model) {
            properties.insert("transcription_model_family".to_string(), model_family);
        }
        if let Some(model_family) = bucket_model_family(summary_model) {
            properties.insert("summary_model_family".to_string(), model_family);
        }

        // Duration metrics
        if let Some(duration) = total_duration_seconds {
            properties.insert("total_duration_seconds".to_string(), duration.to_string());
        }
        properties.insert(
            "active_duration_seconds".to_string(),
            active_duration_seconds.to_string(),
        );
        properties.insert(
            "pause_duration_seconds".to_string(),
            pause_duration_seconds.to_string(),
        );

        // Privacy-safe device types
        properties.insert(
            "microphone_device_type".to_string(),
            microphone_device_type.to_string(),
        );
        properties.insert(
            "system_audio_device_type".to_string(),
            system_audio_device_type.to_string(),
        );

        // Processing stats
        properties.insert("chunks_processed".to_string(), chunks_processed.to_string());
        properties.insert(
            "transcript_segments_count".to_string(),
            transcript_segments_count.to_string(),
        );
        properties.insert("had_fatal_error".to_string(), had_fatal_error.to_string());

        self.track_event("meeting_ended", Some(properties)).await
    }

    // Analytics consent tracking
    pub async fn track_analytics_enabled(&self) -> Result<(), String> {
        self.track_event("analytics_enabled", None).await
    }

    pub async fn track_analytics_disabled(&self) -> Result<(), String> {
        self.track_event("analytics_disabled", None).await
    }

    pub async fn track_analytics_transparency_viewed(&self) -> Result<(), String> {
        self.track_event("analytics_transparency_viewed", None)
            .await
    }

    pub fn is_enabled(&self) -> bool {
        self.config.enabled && self.client.is_some()
    }

    pub async fn set_user_properties(
        &self,
        properties: HashMap<String, String>,
    ) -> Result<(), String> {
        let client = match &self.client {
            Some(client) => Arc::clone(client),
            None => return Ok(()),
        };

        let user_id = match self.user_id.lock().await.clone() {
            Some(id) => id,
            None => {
                eprintln!("Warning: Attempted to set user properties before user identification");
                return Ok(());
            }
        };

        let properties = sanitize_profile_properties(properties);
        let mut event = Event::new("$set", &user_id);
        let _ = event.insert_prop("$geoip_disable", true);

        // Add user properties
        for (key, value) in properties {
            if let Err(e) = event.insert_prop(&key, value) {
                eprintln!("Failed to add property {}: {}", key, e);
            }
        }

        client.capture(event);

        Ok(())
    }
}

// Helper function to create analytics client from config
pub async fn create_analytics_client(config: AnalyticsConfig) -> AnalyticsClient {
    AnalyticsClient::new(config).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_schema_drops_identifiers_content_paths_and_raw_errors() {
        let mut properties = HashMap::new();
        properties.insert("meeting_title".to_string(), "Board Strategy".to_string());
        properties.insert("transcript".to_string(), "confidential words".to_string());
        properties.insert(
            "file_path".to_string(),
            "C:\\meetings\\secret.wav".to_string(),
        );
        properties.insert("device_name".to_string(), "Jane's AirPods".to_string());
        properties.insert("meeting_id".to_string(), "meeting-123".to_string());
        properties.insert("user_id".to_string(), "user-123".to_string());
        properties.insert("error".to_string(), "C:\\private\\failure".to_string());
        properties.insert(
            "error_message".to_string(),
            "raw provider error".to_string(),
        );
        properties.insert("new_value".to_string(), "secret setting".to_string());
        properties.insert("duration_seconds".to_string(), "125".to_string());

        let sanitized = sanitize_event_properties("meeting_completed", properties).unwrap();

        assert_eq!(sanitized.len(), 1);
        assert_eq!(sanitized.get("duration_seconds"), Some(&"125".to_string()));
    }

    #[test]
    fn event_schema_rejects_unknown_events_and_buckets_path_shaped_model_values() {
        assert!(sanitize_event_properties("meeting_title_changed", HashMap::new()).is_err());

        let mut properties = HashMap::new();
        properties.insert("model_provider".to_string(), "ollama".to_string());
        properties.insert(
            "model_family".to_string(),
            "C:\\Users\\private\\model.gguf".to_string(),
        );
        let sanitized =
            sanitize_event_properties("summary_generation_started", properties).unwrap();

        assert_eq!(sanitized.get("model_provider"), Some(&"ollama".to_string()));
        assert_eq!(sanitized.get("model_family"), Some(&"custom".to_string()));
    }

    #[test]
    fn event_schema_accepts_content_free_model_identifiers() {
        let mut properties = HashMap::new();
        properties.insert("model_provider".to_string(), "openrouter".to_string());
        properties.insert(
            "model_family".to_string(),
            bucket_model_family("meta-llama/llama-3.2:latest").unwrap(),
        );

        let sanitized =
            sanitize_event_properties("summary_generation_started", properties).unwrap();
        assert_eq!(sanitized.len(), 2);
        assert_eq!(sanitized.get("model_family"), Some(&"llama".to_string()));
        assert_eq!(
            bucket_model_family("private-customer-model"),
            Some("custom".to_string())
        );
    }

    #[test]
    fn categorical_values_cannot_smuggle_renderer_content() {
        for (event_name, key) in [
            ("feature_used", "feature_name"),
            ("button_click", "button"),
            ("button_click", "location"),
            ("page_view", "page"),
            ("summary_generation_started", "model_provider"),
            ("error", "error_type"),
        ] {
            let mut properties = HashMap::new();
            properties.insert(key.to_string(), "JohnSmithSSN123456".to_string());
            let sanitized = sanitize_event_properties(event_name, properties).unwrap();
            assert!(
                !sanitized.contains_key(key),
                "unapproved categorical value survived for {event_name}.{key}"
            );
        }
    }

    #[test]
    fn profile_schema_is_strict_and_drops_fingerprinting_text() {
        let mut properties = HashMap::new();
        properties.insert("app_version".to_string(), "1.0.4".to_string());
        properties.insert("platform".to_string(), "Windows".to_string());
        properties.insert("architecture".to_string(), "x86_64".to_string());
        properties.insert("user_id".to_string(), "user-123".to_string());
        properties.insert("os_version".to_string(), "Mozilla/5.0 private".to_string());
        properties.insert("first_seen".to_string(), Utc::now().to_rfc3339());

        let sanitized = sanitize_profile_properties(properties);

        assert_eq!(sanitized.len(), 2);
        assert!(!sanitized.contains_key("app_version"));
        assert!(!sanitized.contains_key("user_id"));
        assert!(!sanitized.contains_key("os_version"));
        assert!(!sanitized.contains_key("first_seen"));
    }

    #[test]
    fn installation_identifier_rejects_accounts_and_addresses() {
        assert!(is_valid_installation_id("user_1720990000000_a1b2c3d4e"));
        assert!(!is_valid_installation_id("person@example.com"));
        assert!(!is_valid_installation_id("meeting_123456"));
        assert!(!is_valid_installation_id("user_C:\\private"));
    }

    #[tokio::test]
    async fn client_without_api_key_stays_disabled_and_noops() {
        // Even with enabled=true, an empty key must yield a no-op client —
        // the guard init_analytics relies on when no build-time key exists.
        let config = AnalyticsConfig {
            api_key: String::new(),
            enabled: true,
            ..AnalyticsConfig::default()
        };
        let client = AnalyticsClient::new(config).await;

        assert!(!client.is_enabled());
        assert!(client.track_event("app_started", None).await.is_ok());
        assert!(client.track_event("unapproved_event", None).await.is_err());
        assert!(client
            .identify("person@example.com".to_string(), None)
            .await
            .is_err());
        assert!(client
            .identify("user_1720990000000_a1b2c3d4e".to_string(), None)
            .await
            .is_ok());
    }
}
