//! Persistence for the installed modes (BACKLOG **I4b**).
//!
//! A `tauri-plugin-store` JSON file, for the same reason `copilot::store` is
//! one: the active mode is read when an insight is requested, and the copilot's
//! whole surface must keep working with no database — a mode is configuration,
//! not meeting content, and nothing here ever holds a transcript.
//!
//! ## Failing towards the shipped modes
//!
//! Every read degrades to [`ModesState::fresh`]: an unreadable store, absent
//! JSON, a blob written by a different workspace, or a file a user hand-edited
//! into nonsense all resolve to "the eight built-ins, `general` active". That
//! is the conservative direction — a corrupt file can lose a *custom* mode, but
//! it can never put the copilot into one the user did not write, and it can
//! never leave the copilot with no mode at all.

use super::registry::ModesState;
use tauri::{AppHandle, Runtime};
use tauri_plugin_store::StoreExt;

/// Store file name, alongside `copilot.json` in the app data dir.
const STORE_FILE: &str = "modes.json";
const STATE_KEY: &str = "state";

/// Read this workspace's modes.
///
/// **A blob stored for another workspace is discarded, not migrated**
/// (`docs/MULTITENANCY.md` rule 2). Rewriting its `workspace_id` would be the
/// tempting one-liner and is exactly the bug the rule exists to prevent: it
/// would hand one tenant another's custom modes, purposes and role wording.
pub fn load<R: Runtime>(app: &AppHandle<R>, workspace_id: &str) -> ModesState {
    let Ok(store) = app.store(STORE_FILE) else {
        log::warn!("modes: store unavailable; using the built-ins");
        return ModesState::fresh(workspace_id);
    };
    let Some(value) = store.get(STATE_KEY) else {
        return ModesState::fresh(workspace_id);
    };
    match serde_json::from_value::<ModesState>(value.clone()) {
        Ok(state) if state.belongs_to(workspace_id) => state,
        Ok(_) => {
            log::info!("modes: stored state belongs to another workspace; using the built-ins");
            ModesState::fresh(workspace_id)
        }
        Err(e) => {
            log::warn!("modes: stored state could not be read ({e}); using the built-ins");
            ModesState::fresh(workspace_id)
        }
    }
}

/// Write this workspace's modes. Errors are returned, never swallowed: this
/// runs from a command the user triggered, and a silently lost mode that
/// reappears missing after a restart is worse than a visible failure.
pub fn save<R: Runtime>(app: &AppHandle<R>, state: &ModesState) -> Result<(), String> {
    let store = app
        .store(STORE_FILE)
        .map_err(|e| format!("Could not open the modes file: {e}"))?;
    let value =
        serde_json::to_value(state).map_err(|e| format!("Could not encode the modes: {e}"))?;
    store.set(STATE_KEY, value);
    store
        .save()
        .map_err(|e| format!("Could not save the modes: {e}"))
}
