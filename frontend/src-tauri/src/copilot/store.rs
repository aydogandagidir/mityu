//! Persistence for the copilot's two pieces of state: the user's settings and
//! where they left the panel.
//!
//! Deliberately a `tauri-plugin-store` JSON file rather than SQLite. The
//! settings must be readable at startup, before `AppState`/the database exists
//! and before any window has been created, because that is when the decision
//! "register a global shortcut or not" is made. It follows the same shape as
//! `onboarding.rs` and `audio::recording_preferences`, which have the same
//! requirement.
//!
//! **Nothing in this file touches meeting content.** A copilot insight is never
//! persisted (ADR-0038), so the store holds a few booleans, three shortcut
//! strings and four window coordinates — and a corrupt or missing file degrades
//! to [`CopilotConfig::default`], which is the *off* state.

use super::config::{CopilotConfig, PanelGeometry};
use tauri::{AppHandle, Runtime};
use tauri_plugin_store::StoreExt;

/// Store file name, alongside `onboarding-status.json` in the app data dir.
const STORE_FILE: &str = "copilot.json";
const CONFIG_KEY: &str = "config";
const GEOMETRY_KEY: &str = "geometry";

/// Read the settings. Infallible by construction: an unreadable store, a
/// missing key or unparseable JSON all resolve to the default, and the default
/// is `enabled: false`. A corrupt file can therefore never *switch the copilot
/// on* — the failure direction matters more than the failure itself.
pub fn load_config<R: Runtime>(app: &AppHandle<R>) -> CopilotConfig {
    let Ok(store) = app.store(STORE_FILE) else {
        log::warn!("copilot: settings store unavailable; using defaults (copilot stays off)");
        return CopilotConfig::default();
    };
    let Some(value) = store.get(CONFIG_KEY) else {
        return CopilotConfig::default();
    };
    match serde_json::from_value::<CopilotConfig>(value.clone()) {
        Ok(config) => config,
        Err(e) => {
            log::warn!("copilot: settings could not be read ({e}); using defaults");
            CopilotConfig::default()
        }
    }
}

/// Write the settings. Errors are returned rather than swallowed: this runs
/// from a command the user triggered, so a silent failure would show a switch
/// that flips back on the next launch with no explanation.
pub fn save_config<R: Runtime>(app: &AppHandle<R>, config: &CopilotConfig) -> Result<(), String> {
    let store = app
        .store(STORE_FILE)
        .map_err(|e| format!("Could not open the copilot settings file: {e}"))?;
    let value = serde_json::to_value(config)
        .map_err(|e| format!("Could not encode the copilot settings: {e}"))?;
    store.set(CONFIG_KEY, value);
    store
        .save()
        .map_err(|e| format!("Could not save the copilot settings: {e}"))
}

/// Where the panel was last left, or `None` when there is nothing usable.
/// A stored geometry that would place the panel off-screen is discarded here
/// rather than handed to the window builder — see
/// [`PanelGeometry::is_plausible`].
pub fn load_geometry<R: Runtime>(app: &AppHandle<R>) -> Option<PanelGeometry> {
    let store = app.store(STORE_FILE).ok()?;
    let value = store.get(GEOMETRY_KEY)?;
    let geometry = serde_json::from_value::<PanelGeometry>(value.clone()).ok()?;
    geometry.is_plausible().then_some(geometry)
}

/// Remember where the panel is. Best effort: this runs while the panel is
/// closing, and losing a window position is not worth failing a close over, so
/// the failure is logged and swallowed.
pub fn save_geometry<R: Runtime>(app: &AppHandle<R>, geometry: PanelGeometry) {
    if !geometry.is_plausible() {
        return;
    }
    let Ok(store) = app.store(STORE_FILE) else {
        return;
    };
    match serde_json::to_value(geometry) {
        Ok(value) => {
            store.set(GEOMETRY_KEY, value);
            if let Err(e) = store.save() {
                log::debug!("copilot: could not persist the panel position: {e}");
            }
        }
        Err(e) => log::debug!("copilot: could not encode the panel position: {e}"),
    }
}
