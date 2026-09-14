//! The modes Tauri surface (BACKLOG **I4b**). Registered in `lib.rs`.
//!
//! TS bindings live in `frontend/src/services/modesService.ts` — the only place
//! these names appear on the renderer side (`docs/CONVENTIONS.md`: no raw
//! `invoke` in a component).
//!
//! Thin on purpose. Every rule lives below it: the collision and rejected-
//! category checks in [`super::validator`], "which mode answers" in
//! [`super::registry`], and the workspace boundary in [`super::store`]. This
//! file resolves identity, reads, applies one pure function and writes.
//!
//! **The renderer never names the workspace.** Identity comes from
//! `context::current()` on every call (`docs/CONTRACTS.md`), so a panel cannot
//! ask for another tenant's modes by passing an id.

use serde::Serialize;
use tauri::{AppHandle, Runtime};

use super::registry::{all_modes, is_builtin, resolve_active, taken, ModesState};
use super::types::{LiveAction, Mode};
use super::validator::{preview_mode, validate_mode, ModeError};
use super::{store, DEFAULT_MODE_ID};

/// One row of the modes list.
///
/// The whole [`Mode`] plus the two things the UI cannot derive: whether it may
/// be edited, and whether it is the one answering right now.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModeRow {
    #[serde(flatten)]
    pub mode: Mode,
    /// Built-ins are neither editable nor removable — they are the shipped
    /// vocabulary, and a user who could rewrite `general` could make the
    /// copilot's default behaviour unexplainable to the next person.
    pub builtin: bool,
    pub active: bool,
}

/// What every command here returns: the full list and the active id, so the UI
/// never has to reason about what a mutation did — it re-renders from the
/// backend's own answer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModesView {
    pub modes: Vec<ModeRow>,
    pub active_mode_id: String,
}

fn view(state: &ModesState) -> ModesView {
    let active = resolve_active(state);
    ModesView {
        modes: all_modes(state)
            .into_iter()
            .map(|mode| ModeRow {
                builtin: is_builtin(&mode.id),
                active: mode.id == active.id,
                mode,
            })
            .collect(),
        // The RESOLVED id, not the stored one. A stale pointer must not make
        // the UI show nothing selected while Rust is happily answering with the
        // default.
        active_mode_id: active.id,
    }
}

fn load<R: Runtime>(app: &AppHandle<R>) -> ModesState {
    store::load(app, crate::context::current().tenant_id.as_str())
}

/// List the installed modes and say which one answers.
#[tauri::command]
pub async fn modes_list<R: Runtime>(app: AppHandle<R>) -> ModesView {
    view(&load(&app))
}

/// Choose the mode the copilot uses for the next insight.
#[tauri::command]
pub async fn modes_set_active<R: Runtime>(
    app: AppHandle<R>,
    mode_id: String,
) -> Result<ModesView, String> {
    let mut state = load(&app);
    // Checked against what is actually installed rather than trusted: an id
    // from a stale UI would otherwise be stored and silently resolve to the
    // default, so the user would see their choice fail to stick with no reason
    // given.
    if !all_modes(&state).iter().any(|m| m.id == mode_id) {
        return Err(format!("There is no mode with the id \"{mode_id}\"."));
    }
    state.active_mode_id = mode_id;
    store::save(&app, &state)?;
    Ok(view(&state))
}

/// Check a mode file and show what it would install — **without installing it**.
///
/// The import flow's first half. Every refusal the installer can produce is
/// produced here, with the validator's own sentence, so the user reads the
/// problem before anything is written rather than after.
#[tauri::command]
pub async fn modes_preview<R: Runtime>(app: AppHandle<R>, raw: String) -> Result<Mode, String> {
    let state = load(&app);
    let (ids, names) = taken(&state);
    preview_mode(&raw, &ids, &names).map_err(|e: ModeError| e.to_string())
}

/// Install a custom mode from a file's contents.
///
/// Re-validates rather than trusting a preview the renderer may be holding from
/// before another mode was installed. The check is pure and cheap; the bug it
/// prevents — two modes sharing a name after a race between two Settings
/// windows — is not.
#[tauri::command]
pub async fn modes_install<R: Runtime>(
    app: AppHandle<R>,
    raw: String,
) -> Result<ModesView, String> {
    let mut state = load(&app);
    let (ids, names) = taken(&state);
    let mode = preview_mode(&raw, &ids, &names).map_err(|e: ModeError| e.to_string())?;
    state.custom.push(mode);
    store::save(&app, &state)?;
    Ok(view(&state))
}

/// Edit a custom mode's wording.
///
/// Only the three fields the Settings editor exposes, and deliberately **not**
/// the id, the allowed actions or the allowed sources: those are what a mode
/// *permits*, and widening a permission is an install, not an edit. Changing
/// them through a text field would let a mode quietly gain an action its author
/// never wrote (CLAUDE.md §0.5's spirit — permissions are authored, not
/// migrated).
#[tauri::command]
pub async fn modes_update<R: Runtime>(
    app: AppHandle<R>,
    mode_id: String,
    name: String,
    purpose: String,
    voice: String,
) -> Result<ModesView, String> {
    let mut state = load(&app);
    if is_builtin(&mode_id) {
        return Err(
            "A built-in mode cannot be edited. Import a copy under a new name instead.".into(),
        );
    }
    let Some(position) = state.custom.iter().position(|m| m.id == mode_id) else {
        return Err(format!(
            "There is no custom mode with the id \"{mode_id}\"."
        ));
    };

    let mut edited = state.custom[position].clone();
    edited.name = name;
    edited.purpose = purpose;
    edited.voice = voice;

    // Validated against everything EXCEPT itself, or renaming a mode to the
    // name it already has would be refused as a collision with itself.
    let mut others = state.clone();
    others.custom.remove(position);
    let (ids, names) = taken(&others);
    validate_mode(&edited, &ids, &names).map_err(|e: ModeError| e.to_string())?;

    state.custom[position] = edited;
    store::save(&app, &state)?;
    Ok(view(&state))
}

/// Remove a custom mode.
///
/// If it was the active one the copilot falls back to the default, and that is
/// done **here rather than left to `resolve_active`**: the fallback is already
/// total, but leaving a dangling pointer in the store would mean the Settings
/// list and the stored state disagreed about what is selected.
#[tauri::command]
pub async fn modes_remove<R: Runtime>(
    app: AppHandle<R>,
    mode_id: String,
) -> Result<ModesView, String> {
    let mut state = load(&app);
    if is_builtin(&mode_id) {
        return Err("A built-in mode cannot be removed.".into());
    }
    let before = state.custom.len();
    state.custom.retain(|m| m.id != mode_id);
    if state.custom.len() == before {
        return Err(format!(
            "There is no custom mode with the id \"{mode_id}\"."
        ));
    }
    if state.active_mode_id == mode_id {
        state.active_mode_id = DEFAULT_MODE_ID.to_string();
    }
    store::save(&app, &state)?;
    Ok(view(&state))
}

/// The actions the active mode offers — what `copilot_get_status` reports so
/// the panel renders only the buttons the mode allows.
pub fn active_actions<R: Runtime>(app: &AppHandle<R>) -> Vec<LiveAction> {
    resolve_active(&load(app)).live.allowed_actions
}

/// The mode an insight is built from.
pub fn active_mode<R: Runtime>(app: &AppHandle<R>) -> Mode {
    resolve_active(&load(app))
}
