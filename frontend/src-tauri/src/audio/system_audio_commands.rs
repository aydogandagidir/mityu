//! Commands for *detecting* system-audio activity by other applications.
//!
//! Nothing here captures audio. Three capture-flavoured commands used to live
//! in this file — `start_system_audio_capture_command`,
//! `list_system_audio_devices_command` and
//! `check_system_audio_permissions_command`. They wrapped the dead
//! `audio/capture/system.rs` surface (see that module's replacement doc in
//! `audio/capture/mod.rs`), the first was never exposed to renderer IPC at all,
//! and the other two were registered but invoked by no frontend code. The live
//! permission commands are `audio::permissions::*`.

use crate::audio::{new_system_audio_callback, SystemAudioDetector, SystemAudioEvent};
use anyhow::Result;
use std::sync::{Arc, Mutex};
use tauri::{command, AppHandle, Emitter, State};

// Global state for system audio detector
type SystemAudioDetectorState = Arc<Mutex<Option<SystemAudioDetector>>>;

/// Start monitoring system audio usage by other applications
#[command]
pub async fn start_system_audio_monitoring(
    app_handle: AppHandle,
    detector_state: State<'_, SystemAudioDetectorState>,
) -> Result<(), String> {
    let mut detector_guard = detector_state
        .lock()
        .map_err(|e| format!("Failed to acquire detector lock: {}", e))?;

    if detector_guard.is_some() {
        return Err("System audio monitoring is already active".to_string());
    }

    let mut detector = SystemAudioDetector::new();

    // Create callback that emits events to the frontend
    let callback = new_system_audio_callback(move |event| match event {
        SystemAudioEvent::SystemAudioStarted(apps) => {
            tracing::info!("System audio started by apps: {:?}", apps);
            let _ = app_handle.emit("system-audio-started", apps);
        }
        SystemAudioEvent::SystemAudioStopped => {
            let _ = app_handle.emit("system-audio-stopped", ());
            tracing::info!("System audio stopped");
        }
    });

    detector.start(callback);
    *detector_guard = Some(detector);

    Ok(())
}

/// Stop monitoring system audio usage
#[command]
pub async fn stop_system_audio_monitoring(
    detector_state: State<'_, SystemAudioDetectorState>,
) -> Result<(), String> {
    let mut detector_guard = detector_state
        .lock()
        .map_err(|e| format!("Failed to acquire detector lock: {}", e))?;

    if let Some(mut detector) = detector_guard.take() {
        detector.stop();
        Ok(())
    } else {
        Err("System audio monitoring is not active".to_string())
    }
}

/// Get the current status of system audio monitoring
#[command]
pub async fn get_system_audio_monitoring_status(
    detector_state: State<'_, SystemAudioDetectorState>,
) -> Result<bool, String> {
    let detector_guard = detector_state
        .lock()
        .map_err(|e| format!("Failed to acquire detector lock: {}", e))?;

    Ok(detector_guard.is_some())
}

/// Initialize the system audio detector state in Tauri app
pub fn init_system_audio_state() -> SystemAudioDetectorState {
    Arc::new(Mutex::new(None))
}

// Event payload types for frontend
#[derive(serde::Serialize, Clone)]
pub struct SystemAudioStartedPayload {
    pub apps: Vec<String>,
}

#[derive(serde::Serialize, Clone)]
pub struct SystemAudioStoppedPayload;
