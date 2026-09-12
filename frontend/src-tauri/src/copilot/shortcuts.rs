//! Registering the panel's global shortcut with the operating system.
//!
//! Three rules, each of which exists because breaking it is invisible to the
//! person who broke it:
//!
//! 1. **Only an action with an implementation behind it is registered.** The
//!    set comes from [`KeybindAction::is_registerable`] rather than a list
//!    written here, so `Ask` and `CaptureScreen` — declared for their Settings
//!    rows, implemented in I3 and I6 — cannot take a system-wide key
//!    combination away from every other app to then do nothing with it. A test
//!    pins that derivation.
//! 2. **Nothing is registered while the copilot is off.** With `enabled: false`
//!    this function unregisters and returns, so a disabled build holds no OS
//!    shortcut at all (ADR-0038 invariant 4).
//! 3. **Re-applying is idempotent.** Every call clears the previous
//!    registration first, so changing a keybind in Settings cannot leave the
//!    old combination registered — which would look like a shortcut that
//!    "still works" long after the user changed it.

use super::config::{CopilotConfig, KeybindAction};
use super::{keybind, store, window};
use tauri::{AppHandle, Runtime};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

/// Re-apply every shortcut from the stored settings.
///
/// Called at startup and after any settings change. Individual failures are
/// logged and skipped rather than aborting the rest: one unavailable
/// combination (already taken by another application, which the OS reports at
/// registration time) must not cost the user the others.
pub fn apply<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let config = store::load_config(app);
    apply_config(app, &config)
}

pub fn apply_config<R: Runtime>(app: &AppHandle<R>, config: &CopilotConfig) -> Result<(), String> {
    let manager = app.global_shortcut();

    // Clear first, unconditionally — rule 3.
    if let Err(e) = manager.unregister_all() {
        log::warn!("copilot: could not clear the previous shortcuts: {e}");
    }

    if !config.enabled || !config.shortcuts_enabled {
        return Ok(());
    }

    for action in registerable_actions() {
        let spec = config.keybinds.get(action);
        let parsed = match keybind::parse(spec) {
            Ok(parsed) => parsed,
            Err(e) => {
                log::warn!(
                    "copilot: the shortcut for {} is not valid ({e}); it was not registered",
                    action.wire_name()
                );
                continue;
            }
        };

        let shortcut: Shortcut = match parsed.canonical().parse() {
            Ok(shortcut) => shortcut,
            Err(e) => {
                log::warn!(
                    "copilot: the system could not read the shortcut for {} ({e}); it was not registered",
                    action.wire_name()
                );
                continue;
            }
        };

        let app_handle = app.clone();
        let registration = manager.on_shortcut(shortcut, move |_app, _shortcut, event| {
            // A key press delivers both Pressed and Released; acting on both
            // would toggle the panel twice and leave it exactly as it was.
            if event.state() != ShortcutState::Pressed {
                return;
            }
            handle(&app_handle, action);
        });

        if let Err(e) = registration {
            log::warn!(
                "copilot: '{}' could not be registered for {} ({e}). Another application may \
                 already be using it.",
                parsed.canonical(),
                action.wire_name()
            );
        }
    }

    Ok(())
}

/// Remove every shortcut this app holds. Used when the copilot is switched off.
pub fn clear<R: Runtime>(app: &AppHandle<R>) {
    if let Err(e) = app.global_shortcut().unregister_all() {
        log::warn!("copilot: could not clear the shortcuts: {e}");
    }
}

/// The actions that reach the operating system — derived, never hand-listed.
fn registerable_actions() -> impl Iterator<Item = KeybindAction> {
    KeybindAction::ALL
        .into_iter()
        .filter(|action| action.is_registerable())
}

fn handle<R: Runtime>(app: &AppHandle<R>, action: KeybindAction) {
    match action {
        KeybindAction::TogglePanel => {
            if let Err(e) = window::toggle(app) {
                log::warn!("copilot: the panel shortcut did nothing: {e}");
            }
        }
        // Unreachable while `is_registerable` gates registration; kept as an
        // explicit arm so adding an action forces a decision here rather than
        // compiling into a silent no-op.
        KeybindAction::Ask | KeybindAction::CaptureScreen => {
            log::debug!(
                "copilot: {} has no implementation yet; ignoring",
                action.wire_name()
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The set that reaches the OS is exactly the set with an implementation.
    /// If a future action is added and forgotten here, this fails rather than
    /// registering a dead key combination.
    #[test]
    fn only_implemented_actions_are_registered() {
        let registered: Vec<KeybindAction> = registerable_actions().collect();
        assert_eq!(registered, vec![KeybindAction::TogglePanel]);
        for action in KeybindAction::ALL {
            assert_eq!(
                registered.contains(&action),
                action.is_registerable(),
                "{} is registered without an implementation (or vice versa)",
                action.wire_name()
            );
        }
    }
}
