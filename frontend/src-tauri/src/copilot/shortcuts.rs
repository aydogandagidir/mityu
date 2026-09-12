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
//! 4. **What the OS actually did is remembered.** Registering can fail for a
//!    reason the user needs to know — Microsoft Teams holds
//!    `Ctrl+Shift+M` for in-call mute, which is exactly the combination and
//!    exactly the meeting this panel is for. Settings must not print "active"
//!    over a shortcut the OS refused, so the outcome is recorded here and read
//!    back by `commands::copilot_get_status`, which never calls [`apply`] and
//!    would otherwise have nothing but the stored config to go on.

use super::config::{CopilotConfig, KeybindAction};
use super::{keybind, store, window};
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use tauri::{AppHandle, Runtime};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

/// What the operating system did with a shortcut the last time it was asked.
///
/// Deliberately not a `bool`: "refused" carries the sentence the user needs, and
/// the absence of an entry ("never asked") is a third state that must not read
/// as either of the other two.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Registration {
    /// Accepted — the OS is holding the combination for this app.
    Held,
    /// Refused, with a reason fit to show in Settings.
    Refused(String),
}

/// Process-local record of the last outcome per action.
///
/// Not persisted: it describes what is true of *this run* on *this machine*, and
/// a stale copy read from disk at the next launch would be a confident lie. An
/// empty record means "not asked yet", which is the honest answer before
/// [`apply`] has run.
fn record() -> &'static Mutex<HashMap<KeybindAction, Registration>> {
    static RECORD: OnceLock<Mutex<HashMap<KeybindAction, Registration>>> = OnceLock::new();
    RECORD.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Remember what happened to one action. A poisoned lock is recovered rather
/// than propagated: losing this record must never take the app down.
pub fn note(action: KeybindAction, outcome: Registration) {
    let mut guard = record().lock().unwrap_or_else(|e| e.into_inner());
    guard.insert(action, outcome);
}

/// Forget every outcome — called whenever the shortcuts are cleared, so a
/// "held" entry can never outlive the registration it describes.
pub fn forget_all() {
    let mut guard = record().lock().unwrap_or_else(|e| e.into_inner());
    guard.clear();
}

/// What the OS last said about this action, or `None` if it was never asked.
pub fn registration(action: KeybindAction) -> Option<Registration> {
    let guard = record().lock().unwrap_or_else(|e| e.into_inner());
    guard.get(&action).cloned()
}

/// Is the plugin holding this combination *right now*?
///
/// The authority for the yes/no, with [`registration`] supplying the reason when
/// the answer is no. The record alone would go stale if anything ever cleared
/// the shortcuts without coming back through [`apply_config`]; this reads the
/// plugin's own map, which `unregister_all` empties, so it cannot.
///
/// `None` means the question could not be asked (an unparseable binding) — which
/// the caller must not read as "yes".
///
/// It does not — and no API can — detect the X11 case where `register()` reports
/// success after its event thread has died. The panel's platform wording owns
/// that caveat, not this function.
pub fn is_held<R: Runtime>(app: &AppHandle<R>, spec: &str) -> Option<bool> {
    let parsed = keybind::parse(spec).ok()?;
    let shortcut: Shortcut = parsed.canonical().parse().ok()?;
    Some(app.global_shortcut().is_registered(shortcut))
}

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

    // Clear first, unconditionally — rule 3. The record goes with it: an
    // outcome that outlived its registration would be the exact lie rule 4
    // exists to prevent.
    if let Err(e) = manager.unregister_all() {
        log::warn!("copilot: could not clear the previous shortcuts: {e}");
    }
    forget_all();

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
                note(action, Registration::Refused(e.to_string()));
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
                note(
                    action,
                    Registration::Refused(format!(
                        "'{}' is not a combination this system can register.",
                        parsed.canonical()
                    )),
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

        match registration {
            Ok(()) => note(action, Registration::Held),
            Err(e) => {
                log::warn!(
                    "copilot: '{}' could not be registered for {} ({e}). Another application may \
                     already be using it.",
                    parsed.canonical(),
                    action.wire_name()
                );
                note(
                    action,
                    Registration::Refused(format!(
                        "'{}' was refused by the system — another application may already be \
                         using it.",
                        parsed.canonical()
                    )),
                );
            }
        }
    }

    Ok(())
}

/// Remove every shortcut this app holds. Used when the copilot is switched off.
pub fn clear<R: Runtime>(app: &AppHandle<R>) {
    if let Err(e) = app.global_shortcut().unregister_all() {
        log::warn!("copilot: could not clear the shortcuts: {e}");
    }
    forget_all();
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

    /// The record is process-global, and cargo runs tests in parallel, so these
    /// three would race each other for it. Each takes this lock first —
    /// recovering it when a failing test poisons it, so one failure reports
    /// itself instead of cascading into two misleading ones.
    fn serialised() -> std::sync::MutexGuard<'static, ()> {
        static TEST_LOCK: Mutex<()> = Mutex::new(());
        TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The record must forget an outcome whenever the registration it describes
    /// is torn down; a surviving `Held` would be exactly the stale "active"
    /// label rule 4 exists to prevent.
    #[test]
    fn forgetting_clears_every_recorded_outcome() {
        let _guard = serialised();
        forget_all();
        note(KeybindAction::TogglePanel, Registration::Held);
        assert_eq!(
            registration(KeybindAction::TogglePanel),
            Some(Registration::Held)
        );
        forget_all();
        assert_eq!(registration(KeybindAction::TogglePanel), None);
    }

    #[test]
    fn a_refusal_keeps_the_reason_for_the_user() {
        let _guard = serialised();
        forget_all();
        note(
            KeybindAction::TogglePanel,
            Registration::Refused("already in use".into()),
        );
        match registration(KeybindAction::TogglePanel) {
            Some(Registration::Refused(reason)) => assert_eq!(reason, "already in use"),
            other => panic!("expected a refusal with its reason, got {other:?}"),
        }
        forget_all();
    }

    /// "Never asked" is a third state and must not read as either of the others.
    #[test]
    fn an_action_never_asked_about_has_no_outcome() {
        let _guard = serialised();
        forget_all();
        assert_eq!(registration(KeybindAction::Ask), None);
    }

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
