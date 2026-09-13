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

use super::config::{CopilotConfig, KeybindAction, MAX_LIVE_WINDOW_SECS, MIN_LIVE_WINDOW_SECS};
use super::policy::ProtectionVerdict;
use super::session::LiveContextStatus;
use super::{keybind, session, shortcuts, store, window};
use serde::Serialize;
use tauri::{AppHandle, Runtime};

/// One row of the Settings shortcut list: what the binding is, whether it is
/// actually registered, and — when it is not — why.
///
/// Serialize only — see [`ProtectionVerdict`] for why a struct holding
/// `&'static str` cannot derive `Deserialize`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShortcutInfo {
    /// Wire token of the action (`togglePanel`, `ask`, `captureScreen`).
    pub action: &'static str,
    pub label: &'static str,
    /// The user's binding, canonicalised.
    pub keybind: String,
    /// True only when the operating system is actually holding this combination
    /// for Mityu — never merely because the settings say it should.
    pub registered: bool,
    /// Why it is not registered, when it is not. Owned rather than `&'static`
    /// because the interesting cases come from the OS at runtime.
    pub unavailable_reason: Option<String>,
}

/// Shown while the copilot is on and shortcuts are enabled but the OS has not
/// been asked yet — the brief window between enabling and [`shortcuts::apply`].
const NOT_ASKED_YET: &str = "Not registered — Mityu has not asked the system yet.";

/// Decide one Settings row.
///
/// Pure, and separated from [`build_status`] for one reason: this is where the
/// UI previously lied. The row used to be derived from the stored config, so it
/// printed "· active" over a combination the OS had refused — and the refusal is
/// not hypothetical, Microsoft Teams holds `Ctrl+Shift+M` for in-call mute,
/// which is this feature's own default and the very meeting it is for.
///
/// Two sources, each for what it is actually good for:
///
/// - `held_by_os` — [`shortcuts::is_held`], the plugin's live map. **The
///   authority for the yes/no**, because it cannot go stale: anything that
///   clears the shortcuts empties that map too.
/// - `registration` — what [`shortcuts::apply_config`] recorded the OS saying.
///   **The authority for the reason**, which the boolean cannot carry.
///
/// `held_by_os: None` means the question could not be asked, and falls back to
/// the record rather than being read as a yes.
fn shortcut_row(
    action: KeybindAction,
    config: &CopilotConfig,
    registration: Option<shortcuts::Registration>,
    held_by_os: Option<bool>,
) -> ShortcutInfo {
    let keybind = config.keybinds.get(action).to_string();
    let (registered, unavailable_reason) = if !action.is_registerable() {
        // The most specific truth wins: an action with no implementation says so
        // whatever the switches are set to.
        (false, action.unavailable_reason().map(str::to_string))
    } else if !config.enabled {
        (false, Some("The copilot is off.".to_string()))
    } else if !config.shortcuts_enabled {
        (false, Some("Shortcuts are turned off.".to_string()))
    } else {
        let recorded_held = matches!(registration, Some(shortcuts::Registration::Held));
        // The live answer wins; the record only fills in when we could not ask.
        let held = held_by_os.unwrap_or(recorded_held);
        let reason = match registration {
            _ if held => None,
            Some(shortcuts::Registration::Refused(reason)) => Some(reason),
            // Recorded as held but the OS is not holding it: something cleared
            // the registration behind our back. Say that, rather than inventing
            // a collision that may not have happened.
            Some(shortcuts::Registration::Held) => {
                Some("Registered, then released by the system.".to_string())
            }
            None => Some(NOT_ASKED_YET.to_string()),
        };
        (held, reason)
    };

    ShortcutInfo {
        action: action.wire_name(),
        label: action.label(),
        keybind,
        registered,
        unavailable_reason,
    }
}

/// Everything the panel and the Settings tab need in one read.
///
/// Serialize only, because it contains [`ProtectionVerdict`] and
/// [`ShortcutInfo`]. The config *inside* it stays round-trippable: the UI sends
/// a `CopilotConfig` back to `copilot_set_config`, and that type owns its
/// strings.
#[derive(Clone, Debug, Serialize)]
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
    /// The I2 live-context service: subscribed or not, and how much it holds.
    /// Counts only — the payload never carries transcript text.
    pub live_context: LiveContextStatus,
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
    // Same rule for the live window: reject, do not silently clamp — a user
    // who typed 5 should learn the minimum, not find 30 saved.
    if !config.live_window_is_valid() {
        return Err(format!(
            "Live context window — must be between {MIN_LIVE_WINDOW_SECS} and {MAX_LIVE_WINDOW_SECS} seconds."
        ));
    }

    store::save_config(&app, &config)?;

    if config.enabled {
        shortcuts::apply_config(&app, &config)?;
        window::refresh_content_protection(&app, config.content_protection);
        session::apply_config(&app, &config);
    } else {
        // Off means off: no shortcut held, no panel on screen, no listener.
        shortcuts::clear(&app);
        session::stop(&app);
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

    let shortcuts = KeybindAction::ALL
        .into_iter()
        .map(|action| {
            shortcut_row(
                action,
                &config,
                shortcuts::registration(action),
                shortcuts::is_held(app, config.keybinds.get(action)),
            )
        })
        .collect();

    CopilotStatus {
        panel_open: window::is_open(app),
        recording: crate::audio::recording_commands::is_recording().await,
        protection,
        shortcuts,
        live_context: session::status(),
        config,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::copilot::shortcuts::Registration;

    fn enabled_config() -> CopilotConfig {
        CopilotConfig {
            enabled: true,
            ..CopilotConfig::default()
        }
    }

    /// The defect this function was extracted to fix: Settings printed "active"
    /// over a shortcut the OS had refused.
    #[test]
    fn a_refused_shortcut_is_not_reported_as_registered() {
        let row = shortcut_row(
            KeybindAction::TogglePanel,
            &enabled_config(),
            Some(Registration::Refused(
                "'CommandOrControl+Shift+M' was refused by the system.".into(),
            )),
            Some(false),
        );
        assert!(!row.registered);
        assert!(row
            .unavailable_reason
            .expect("a refusal explains itself")
            .contains("refused"));
    }

    #[test]
    fn a_held_shortcut_is_reported_as_registered() {
        let row = shortcut_row(
            KeybindAction::TogglePanel,
            &enabled_config(),
            Some(Registration::Held),
            Some(true),
        );
        assert!(row.registered);
        assert_eq!(row.unavailable_reason, None);
    }

    /// The live plugin map is the authority. A record saying "held" cannot
    /// outvote an OS that is no longer holding it — that staleness is the whole
    /// reason the live query exists.
    #[test]
    fn the_live_answer_outranks_a_stale_record() {
        let row = shortcut_row(
            KeybindAction::TogglePanel,
            &enabled_config(),
            Some(Registration::Held),
            Some(false),
        );
        assert!(!row.registered);
        assert_eq!(
            row.unavailable_reason.as_deref(),
            Some("Registered, then released by the system.")
        );
    }

    /// …and in the other direction: the OS holding it settles the question even
    /// if the record never recorded anything.
    #[test]
    fn the_live_answer_also_outranks_a_missing_record() {
        let row = shortcut_row(
            KeybindAction::TogglePanel,
            &enabled_config(),
            None,
            Some(true),
        );
        assert!(row.registered);
        assert_eq!(row.unavailable_reason, None);
    }

    /// When the question could not be asked, fall back to the record — never to
    /// an optimistic "yes".
    #[test]
    fn an_unanswerable_query_falls_back_to_the_record() {
        let refused = shortcut_row(
            KeybindAction::TogglePanel,
            &enabled_config(),
            Some(Registration::Refused("nope".into())),
            None,
        );
        assert!(!refused.registered);

        let held = shortcut_row(
            KeybindAction::TogglePanel,
            &enabled_config(),
            Some(Registration::Held),
            None,
        );
        assert!(held.registered);
    }

    /// Before `apply` has run there is no outcome, and "no outcome" must not
    /// read as success.
    #[test]
    fn an_unasked_shortcut_is_not_reported_as_registered() {
        let row = shortcut_row(KeybindAction::TogglePanel, &enabled_config(), None, None);
        assert!(!row.registered);
        assert_eq!(row.unavailable_reason.as_deref(), Some(NOT_ASKED_YET));
    }

    /// A stale `Held` must never survive the copilot being switched off — and
    /// even if one did, the row refuses it.
    #[test]
    fn a_disabled_copilot_registers_nothing_whatever_the_record_says() {
        let row = shortcut_row(
            KeybindAction::TogglePanel,
            &CopilotConfig::default(),
            Some(Registration::Held),
            Some(true),
        );
        assert!(!row.registered);
        assert_eq!(
            row.unavailable_reason.as_deref(),
            Some("The copilot is off.")
        );
    }

    #[test]
    fn turning_shortcuts_off_is_reported_as_its_own_reason() {
        let config = CopilotConfig {
            enabled: true,
            shortcuts_enabled: false,
            ..CopilotConfig::default()
        };
        let row = shortcut_row(
            KeybindAction::TogglePanel,
            &config,
            Some(Registration::Held),
            Some(true),
        );
        assert!(!row.registered);
        assert_eq!(
            row.unavailable_reason.as_deref(),
            Some("Shortcuts are turned off.")
        );
    }

    /// An action with no implementation says so, whatever the switches and
    /// whatever the OS might be holding.
    #[test]
    fn an_unimplemented_action_says_so_first() {
        for action in [KeybindAction::Ask, KeybindAction::CaptureScreen] {
            let row = shortcut_row(
                action,
                &enabled_config(),
                Some(Registration::Held),
                Some(true),
            );
            assert!(!row.registered, "{} claimed to be registered", row.action);
            assert_eq!(
                row.unavailable_reason.as_deref(),
                action.unavailable_reason()
            );
        }
    }
}
