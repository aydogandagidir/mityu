use log::{info, warn};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::{AppHandle, Runtime};
use tauri_plugin_store::StoreExt;

use anyhow::Result;
#[cfg(target_os = "macos")]
use log::error;

use crate::audio::capture::AudioCaptureBackend;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RecordingPreferences {
    pub save_folder: PathBuf,
    pub auto_save: bool,
    pub file_format: String,
    #[serde(default)]
    pub preferred_mic_device: Option<String>,
    #[serde(default)]
    pub preferred_system_device: Option<String>,
    /// Persisted [`AudioCaptureBackend`] id. Present on every platform: off
    /// macOS there is only one backend today, but the Linux system-audio epic
    /// (ADR-0022) adds a second, and a preference that only exists on one
    /// platform is a seam that has to be re-cut later. `#[serde(default)]`
    /// keeps preference files written before this change loadable.
    #[serde(default)]
    pub system_audio_backend: Option<String>,
}

impl Default for RecordingPreferences {
    fn default() -> Self {
        Self {
            save_folder: get_default_recordings_folder(),
            auto_save: true,
            file_format: "mp4".to_string(),
            preferred_mic_device: None,
            preferred_system_device: None,
            system_audio_backend: Some(AudioCaptureBackend::default().to_string()),
        }
    }
}

/// Get the default recordings folder based on platform
pub fn get_default_recordings_folder() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        // Windows: %USERPROFILE%\Music\mityu-recordings
        if let Some(music_dir) = dirs::audio_dir() {
            music_dir.join("mityu-recordings")
        } else {
            // Fallback to Documents if Music folder is not available
            dirs::document_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("mityu-recordings")
        }
    }

    #[cfg(target_os = "macos")]
    {
        // macOS: ~/Movies/mityu-recordings
        if let Some(movies_dir) = dirs::video_dir() {
            movies_dir.join("mityu-recordings")
        } else {
            // Fallback to Documents if Movies folder is not available
            dirs::document_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("mityu-recordings")
        }
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        // Linux/Others: ~/Documents/mityu-recordings
        dirs::document_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("mityu-recordings")
    }
}

/// Ensure the recordings directory exists
pub fn ensure_recordings_directory(path: &PathBuf) -> Result<()> {
    if !path.exists() {
        std::fs::create_dir_all(path)?;
        info!("Created the Mityu recordings directory");
    }
    Ok(())
}

/// Generate a unique filename for a recording
pub fn generate_recording_filename(format: &str) -> String {
    let now = chrono::Utc::now();
    let timestamp = now.format("%Y%m%d_%H%M%S");
    format!("recording_{}.{}", timestamp, format)
}

/// Load recording preferences from store
pub async fn load_recording_preferences<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<RecordingPreferences> {
    // Try to load from Tauri store
    let store = match app.store("recording_preferences.json") {
        Ok(store) => store,
        Err(e) => {
            warn!("Failed to access store: {}, using defaults", e);
            return Ok(RecordingPreferences::default());
        }
    };

    // Try to get the preferences from store
    let mut prefs = if let Some(value) = store.get("preferences") {
        match serde_json::from_value::<RecordingPreferences>(value.clone()) {
            Ok(mut p) => {
                info!("Loaded recording preferences from store");
                // Report the backend actually in effect, not a stale stored id.
                let backend = crate::audio::capture::get_current_backend();
                p.system_audio_backend = Some(backend.to_string());
                p
            }
            Err(e) => {
                warn!("Failed to deserialize preferences: {}, using defaults", e);
                RecordingPreferences::default()
            }
        }
    } else {
        info!("No stored preferences found, using defaults");
        RecordingPreferences::default()
    };

    // v1.0.4 records only inside the platform-owned Mityu recordings root.
    // Never trust a renderer-controlled or legacy persisted deletion root.
    prefs.save_folder = get_default_recordings_folder();
    info!(
        "Loaded recording preferences: auto_save={}, format={}, mic_selected={}, system_selected={}",
        prefs.auto_save,
        prefs.file_format,
        prefs.preferred_mic_device.is_some(),
        prefs.preferred_system_device.is_some()
    );
    Ok(prefs)
}

/// Save recording preferences to store
pub async fn save_recording_preferences<R: Runtime>(
    app: &AppHandle<R>,
    preferences: &RecordingPreferences,
) -> Result<()> {
    let mut preferences = preferences.clone();
    preferences.save_folder = get_default_recordings_folder();
    info!(
        "Saving recording preferences: auto_save={}, format={}, mic_selected={}, system_selected={}",
        preferences.auto_save,
        preferences.file_format,
        preferences.preferred_mic_device.is_some(),
        preferences.preferred_system_device.is_some()
    );

    // Get or create store
    let store = app
        .store("recording_preferences.json")
        .map_err(|e| anyhow::anyhow!("Failed to access store: {}", e))?;

    // Serialize preferences to JSON value
    let prefs_value = serde_json::to_value(&preferences)
        .map_err(|e| anyhow::anyhow!("Failed to serialize preferences: {}", e))?;

    // Save to store
    store.set("preferences", prefs_value);

    // Persist to disk
    store
        .save()
        .map_err(|e| anyhow::anyhow!("Failed to save store to disk: {}", e))?;

    info!("Successfully persisted recording preferences to disk");

    // Save backend preference to global config
    if let Some(backend_str) = &preferences.system_audio_backend {
        if let Some(backend) = AudioCaptureBackend::from_string(backend_str) {
            info!("Setting audio capture backend to: {:?}", backend);
            crate::audio::capture::set_current_backend(backend);
        }
    }

    // Ensure the directory exists
    ensure_recordings_directory(&preferences.save_folder)?;

    Ok(())
}

/// Tauri commands for recording preferences
#[tauri::command]
pub async fn get_recording_preferences<R: Runtime>(
    app: AppHandle<R>,
) -> Result<RecordingPreferences, String> {
    load_recording_preferences(&app)
        .await
        .map_err(|e| format!("Failed to load recording preferences: {}", e))
}

#[tauri::command]
pub async fn set_recording_preferences<R: Runtime>(
    app: AppHandle<R>,
    preferences: RecordingPreferences,
) -> Result<(), String> {
    save_recording_preferences(&app, &preferences)
        .await
        .map_err(|e| format!("Failed to save recording preferences: {}", e))
}

#[tauri::command]
pub async fn get_default_recordings_folder_path() -> Result<String, String> {
    let path = get_default_recordings_folder();
    Ok(path.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn open_recordings_folder<R: Runtime>(app: AppHandle<R>) -> Result<(), String> {
    let preferences = load_recording_preferences(&app)
        .await
        .map_err(|e| format!("Failed to load preferences: {}", e))?;

    // Ensure directory exists before trying to open it
    ensure_recordings_directory(&preferences.save_folder)
        .map_err(|e| format!("Failed to create directory: {}", e))?;

    let folder_path = preferences.save_folder.to_string_lossy().to_string();

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(&folder_path)
            .spawn()
            .map_err(|e| format!("Failed to open folder: {}", e))?;
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&folder_path)
            .spawn()
            .map_err(|e| format!("Failed to open folder: {}", e))?;
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        std::process::Command::new("xdg-open")
            .arg(&folder_path)
            .spawn()
            .map_err(|e| format!("Failed to open folder: {}", e))?;
    }

    info!("Opened recordings folder: {}", folder_path);
    Ok(())
}

#[tauri::command]
pub async fn select_recording_folder<R: Runtime>(
    _app: AppHandle<R>,
) -> Result<Option<String>, String> {
    // Use Tauri's dialog to select folder
    // For now, return None - this would need to be implemented with tauri-plugin-dialog
    // when it's available in the Cargo.toml
    warn!("Folder selection not yet implemented - using dialog plugin");
    Ok(None)
}

// Backend selection commands
//
// These four commands used to branch on `#[cfg(target_os = "macos")]` and, off
// macOS, return the hardcoded literal "screencapturekit" — so Windows and Linux
// advertised an Apple framework as their only capture backend, and the settings
// UI rendered it disabled, leaving the list empty of anything choosable. They
// now go through `AudioCaptureBackend` on every platform, which is what makes
// adding a backend (the Linux PipeWire/PulseAudio epic, ADR-0022) a matter of
// adding one enum variant.

/// Get available audio capture backends for the current platform
#[tauri::command]
pub async fn get_available_audio_backends() -> Result<Vec<String>, String> {
    Ok(crate::audio::capture::get_available_backends()
        .iter()
        .map(|b| b.to_string())
        .collect())
}

/// Get current audio capture backend
#[tauri::command]
pub async fn get_current_audio_backend() -> Result<String, String> {
    Ok(crate::audio::capture::get_current_backend().to_string())
}

/// Set audio capture backend
#[tauri::command]
pub async fn set_audio_backend(backend: String) -> Result<(), String> {
    let backend_enum = AudioCaptureBackend::from_string(&backend)
        .ok_or_else(|| format!("Backend '{}' is not available on this platform", backend))?;

    // Core Audio is the one backend that needs an OS permission before it can
    // open a tap; the check stays macOS-only because the permission does.
    #[cfg(target_os = "macos")]
    if backend_enum == AudioCaptureBackend::CoreAudio {
        use crate::audio::permissions::{
            check_screen_recording_permission, request_screen_recording_permission,
        };

        info!("🔐 Core Audio backend requires Audio Capture permission (macOS 14.4+)");
        info!("📍 Permission dialog will appear automatically when recording starts");

        // Check if permission is already granted (this is informational only)
        if !check_screen_recording_permission() {
            warn!("⚠️  Audio Capture permission may not be granted");

            // Attempt to open System Settings (opens System Settings)
            if let Err(e) = request_screen_recording_permission() {
                error!("Failed to open System Settings: {}", e);
            }

            return Err(
                "Core Audio requires Audio Capture permission. \
                    The permission dialog will appear when you start recording. \
                    If already denied, enable it in System Settings → Privacy & Security → Audio Capture, \
                    then restart the app.".to_string()
            );
        }

        info!("✅ Core Audio backend selected - permission check will occur at recording start");
    }

    info!("Setting audio backend to: {:?}", backend_enum);
    crate::audio::capture::set_current_backend(backend_enum);
    Ok(())
}

/// Get backend information (name and description)
#[derive(Serialize)]
pub struct BackendInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    /// Whether the settings UI should let the user pick this backend. Owned
    /// here rather than inferred in the frontend from the id string.
    pub selectable: bool,
}

#[tauri::command]
pub async fn get_audio_backend_info() -> Result<Vec<BackendInfo>, String> {
    Ok(crate::audio::capture::get_available_backends()
        .into_iter()
        .map(|b| BackendInfo {
            id: b.to_string(),
            name: b.name().to_string(),
            description: b.description().to_string(),
            selectable: b.selectable(),
        })
        .collect())
}
