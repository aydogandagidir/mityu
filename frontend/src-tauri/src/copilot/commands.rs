//! The copilot's Tauri command surface. Registered in `lib.rs`.
//!
//! TS bindings live in `frontend/src/services/copilotService.ts` — the only
//! place these names appear on the renderer side.
//!
//! ```ts
//! const status = await invoke<CopilotStatus>('copilot_get_status');
//! const status = await invoke<CopilotStatus>('copilot_set_config', { config });
//! await invoke('copilot_toggle_panel');
//! await invoke('copilot_close_panel');
//! await invoke('copilot_focus_main_window');
//! ```
//!
//! Every command here is local, synchronous work over a settings file and a
//! window handle: no network, no database, no model, no transcript.

use super::config::{CopilotConfig, KeybindAction};
use super::policy::ProtectionVerdict;
use super::{keybind, shortcuts, store, window};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Runtime};

/// One row of the Settings shortcut list: what the binding is, whether it is
/// actually registered, and — when it is not — why.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShortcutInfo {
    /// Wire token of the action (`togglePanel`, `ask`, `captureScreen`).
    pub action: &'static str,
    pub label: &'static str,
    /// The user's binding, canonicalised.
    pub keybind: String,
    /// True only when the action has an implementation *and* shortcuts are on
    /// *and* the copilot is enabled — i.e. when the OS is actually holding it.
    pub registered: bool,
    /// Why it is not registered, when it is not.
    pub unavailable_reason: Option<&'static str>,
}

/// Everything the panel and the Settings tab need in one read.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CopilotStatus {
    pub config: CopilotConfig,
    /// What the OS will do about screen capture here, with the exact wording.
    pub protection: ProtectionVerdict,
    /// Is the panel window open right now?
    pub panel_open: bool,
    /// Is a recording session active? The panel shows live content only while
    /// this is true — the copilot has no capture of its own, it follows a
    /// session the user consented to (ADR-0038 invariant 2).
    pub recording: bool,
    pub shortcuts: Vec<ShortcutInfo>,
}

/// Read the copilot's state. Local-only and cheap enough to poll.
#[tauri::command]
pub async fn copilot_get_status<R: Runtime>(app: AppHandle<R>) -> CopilotStatus {
    let config = store::load_config(&app);
    build_status(&app, config).await
}

/// Save settings, then make the running app match them: shortcuts are
/// re-applied, the panel's content protection is refreshed, and switching the
/// copilot off closes the panel and releases every shortcut.
///
/// Returns the same shape as `copilot_get_status`, so the UI never has to guess
/// what the backend did with what it sent.
#[tauri::command]
pub async fn copilot_set_config<R: Runtime>(
    app: AppHandle<R>,
    config: CopilotConfig,
) -> Result<CopilotStatus, String> {
    // Validate and canonicalise every binding before anything is stored: a
    // rejected string must not reach the store, or the next startup would try
    // to register it and fail silently.
    let mut config = config;
    for action in KeybindAction::ALL {
        let spec = config.keybinds.get(action);
        let parsed = keybind::parse(spec).map_err(|e| format!("{} — {e}", action.label()))?;
        config.keybinds.set(action, parsed.canonical());
    }

    store::save_config(&app, &config)?;

    if config.enabled {
        shortcuts::apply_config(&app, &config)?;
        window::refresh_content_protection(&app, config.content_protection);
    } else {
        // Off means off: no shortcut held, no panel on screen.
        shortcuts::clear(&app);
        window::close(&app)?;
    }

    Ok(build_status(&app, config).await)
}

/// Show or hide the panel — the same action as the global shortcut and the tray
/// entry.
#[tauri::command]
pub async fn copilot_toggle_panel<R: Runtime>(app: AppHandle<R>) -> Result<(), String> {
    window::toggle(&app)
}

/// Close the panel (used by the panel's own close button).
#[tauri::command]
pub async fn copilot_close_panel<R: Runtime>(app: AppHandle<R>) -> Result<(), String> {
    window::close(&app)
}

/// Bring the main window forward.
///
/// The panel deliberately does not duplicate stopping a recording — that path
/// needs a save location plus the post-processing chain the main window owns,
/// and `stop_recording` already returns a boolean meaning "another caller owns
/// shutdown". Rather than become a second owner of it, the panel sends the user
/// to the window that is one.
#[tauri::command]
pub async fn copilot_focus_main_window<R: Runtime>(app: AppHandle<R>) {
    crate::tray::focus_main_window(&app);
}

async fn build_status<R: Runtime>(app: &AppHandle<R>, config: CopilotConfig) -> CopilotStatus {
    let protection = super::policy::current_verdict();
    let shortcuts_live = config.enabled && config.shortcuts_enabled;

    let shortcuts = KeybindAction::ALL
        .into_iter()
        .map(|action| ShortcutInfo {
            action: action.wire_name(),
            label: action.label(),
            keybind: config.keybinds.get(action).to_string(),
            registered: shortcuts_live && action.is_registerable(),
            unavailable_reason: action.unavailable_reason(),
        })
        .collect();

    CopilotStatus {
        panel_open: window::is_open(app),
        recording: crate::audio::recording_commands::is_recording().await,
        protection,
        shortcuts,
        config,
    }
}
