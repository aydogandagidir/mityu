//! The installed set of modes: the built-ins, the user's custom ones, and which
//! is active (BACKLOG **I4b**).
//!
//! Pure, like the rest of `modes` except [`super::store`]. Every rule about
//! *which mode answers* is decided here over plain data, so it can be tested
//! without a running app — and so the Tauri command layer above stays a
//! translation of arguments rather than a place where policy hides.
//!
//! ## Why a custom mode can never shadow a built-in
//!
//! [`super::validator::validate_mode`] refuses an id or a name that collides
//! with anything already installed, and the built-ins are always in that list.
//! So [`all_modes`] can concatenate without a precedence rule: there is nothing
//! to break a tie between. That is deliberate — "your custom `general`
//! silently replaced the shipped one" is the kind of surprise that makes a
//! mode's behaviour unexplainable.

use serde::{Deserialize, Serialize};

use super::defaults::{builtin_modes, BUILTIN_MODE_IDS};
use super::types::Mode;
use super::DEFAULT_MODE_ID;

/// The whole of a workspace's mode state, as stored.
///
/// Carries its `workspace_id` (CLAUDE.md §0.3) even though local-first has
/// exactly one. That is the point: the field exists from the first commit, and
/// [`ModesState::belongs_to`] is the one place that decides whether a stored
/// blob may be used by the caller — so a Phase-2 tenant switch cannot quietly
/// hand one workspace another's modes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModesState {
    pub workspace_id: String,
    /// The mode used for the next insight. May name a mode that no longer
    /// exists — [`resolve_active`] is what makes that harmless.
    pub active_mode_id: String,
    /// Modes the user installed. Never contains a built-in id or name.
    pub custom: Vec<Mode>,
}

impl ModesState {
    /// A fresh workspace: the default mode, no custom modes.
    pub fn fresh(workspace_id: &str) -> Self {
        Self {
            workspace_id: workspace_id.to_string(),
            active_mode_id: DEFAULT_MODE_ID.to_string(),
            custom: Vec::new(),
        }
    }

    pub fn belongs_to(&self, workspace_id: &str) -> bool {
        self.workspace_id == workspace_id
    }
}

/// Everything installed: the built-ins first, then the custom modes in the
/// order they were added.
pub fn all_modes(state: &ModesState) -> Vec<Mode> {
    let mut modes = builtin_modes();
    modes.extend(state.custom.iter().cloned());
    modes
}

/// The ids and names already taken, which is what the validator needs to refuse
/// a collision. Built-ins included, always.
pub fn taken(state: &ModesState) -> (Vec<String>, Vec<String>) {
    let modes = all_modes(state);
    (
        modes.iter().map(|m| m.id.clone()).collect(),
        modes.into_iter().map(|m| m.name).collect(),
    )
}

/// The mode that answers.
///
/// Total by construction. A stored `active_mode_id` can name a custom mode the
/// user has since deleted, or one written by a build that had a mode this one
/// does not — and the copilot must still answer, so an unknown id falls back to
/// the default rather than failing a request the user just made.
pub fn resolve_active(state: &ModesState) -> Mode {
    all_modes(state)
        .into_iter()
        .find(|m| m.id == state.active_mode_id)
        .unwrap_or_else(super::default_mode)
}

/// Is this id one of the shipped modes? Custom modes are editable and
/// removable; built-ins are neither.
pub fn is_builtin(id: &str) -> bool {
    BUILTIN_MODE_IDS.contains(&id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modes::types::{AllowedSource, EvidencePolicy, LiveAction, LivePolicy};

    fn custom(id: &str, name: &str) -> Mode {
        Mode {
            id: id.to_string(),
            name: name.to_string(),
            purpose: "A conversation of some kind.".into(),
            user_role: "the user".into(),
            counterpart_role: "the other side".into(),
            voice: "Plain.".into(),
            summary_template_id: "standard_meeting".into(),
            live: LivePolicy {
                allowed_actions: vec![LiveAction::Recap],
                evidence_policy: EvidencePolicy::SourceFirst,
                cite_required: true,
            },
            allowed_sources: vec![AllowedSource::Transcript],
        }
    }

    fn with_custom(modes: Vec<Mode>) -> ModesState {
        ModesState {
            custom: modes,
            ..ModesState::fresh("local")
        }
    }

    #[test]
    fn a_fresh_workspace_is_on_the_default_mode() {
        assert_eq!(
            resolve_active(&ModesState::fresh("local")).id,
            DEFAULT_MODE_ID
        );
    }

    #[test]
    fn a_custom_mode_can_be_made_active() {
        let mut state = with_custom(vec![custom("site_walk", "Site walk")]);
        state.active_mode_id = "site_walk".into();
        assert_eq!(resolve_active(&state).name, "Site walk");
    }

    /// The copilot must answer even when the stored pointer is stale — the user
    /// deleting a mode, or opening a store written by a build that had one this
    /// one does not, must not turn into a failed request.
    #[test]
    fn an_active_mode_that_no_longer_exists_falls_back_to_the_default() {
        let mut state = ModesState::fresh("local");
        state.active_mode_id = "a_mode_that_was_deleted".into();
        assert_eq!(resolve_active(&state).id, DEFAULT_MODE_ID);
    }

    /// Every built-in id and name is offered to the validator, so a custom mode
    /// cannot take one and there is never a tie to break in `all_modes`.
    #[test]
    fn the_taken_names_include_every_builtin() {
        let (ids, names) = taken(&ModesState::fresh("local"));
        assert_eq!(ids.len(), builtin_modes().len());
        assert!(ids.contains(&DEFAULT_MODE_ID.to_string()));
        assert!(names.contains(&"General".to_string()));
    }

    #[test]
    fn taken_grows_with_an_installed_mode() {
        let state = with_custom(vec![custom("site_walk", "Site walk")]);
        let (ids, names) = taken(&state);
        assert!(ids.contains(&"site_walk".to_string()));
        assert!(names.contains(&"Site walk".to_string()));
    }

    #[test]
    fn a_builtin_is_recognised_and_a_custom_one_is_not() {
        assert!(is_builtin(DEFAULT_MODE_ID));
        assert!(!is_builtin("site_walk"));
    }

    /// `docs/MULTITENANCY.md` rule 2. The stored blob names its workspace, and
    /// a caller in another one must not be handed it.
    #[test]
    fn a_state_stored_for_another_workspace_is_not_ours() {
        let state = ModesState::fresh("some-other-workspace");
        assert!(!state.belongs_to("local"));
        assert!(state.belongs_to("some-other-workspace"));
    }
}
