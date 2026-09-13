//! Meeting modes — what kind of conversation this is, and what the copilot may
//! do in it (BACKLOG **I4a**, ADR-0038).
//!
//! `summary/templates` already answers "how should the notes be written
//! afterwards". A mode answers the questions that only matter *during* the
//! conversation: who the user is in it, who the other side is, which of the
//! copilot's actions are appropriate, how strictly an answer must cite, and
//! what it may read. I3 builds its prompt from exactly that; I4b puts it in
//! Settings.
//!
//! ## Additive by construction
//!
//! Nothing in the existing summary path changes. A mode *points at* a template
//! by id and the loader resolves it the way it always did — so every meeting
//! that has no mode keeps working, and a mode is a layer above templates rather
//! than a replacement for them. This module registers no command, spawns no
//! task and is not called from `setup`; it is data and total functions until
//! something asks for a mode.
//!
//! ## Layout
//!
//! - [`types`] — [`Mode`] and the small enums it is made of.
//! - [`defaults`] — the eight built-ins, written in Rust so the compiler checks
//!   them, each resolving to a template compiled into the binary.
//! - [`validator`] — the **pure** check a custom mode passes before install:
//!   size cap, id and name collision, and ADR-0038's rejected category.

pub mod defaults;
pub mod types;
pub mod validator;

pub use defaults::{builtin_mode, builtin_modes, BUILTIN_MODE_IDS};
pub use types::{AllowedSource, EvidencePolicy, LiveAction, LivePolicy, Mode};
pub use validator::{preview_mode, ModeError};

/// The mode used when the user has not chosen one.
///
/// Every path that needs a mode resolves through here rather than reaching for
/// `builtin_modes()[0]`, so "what happens with no mode set" is one answer in
/// one place.
pub const DEFAULT_MODE_ID: &str = "general";

/// The default mode, or — if someone ever renames it out from under this
/// constant — the first built-in, which is better than no mode at all.
pub fn default_mode() -> Mode {
    builtin_mode(DEFAULT_MODE_ID).unwrap_or_else(|| {
        builtin_modes()
            .into_iter()
            .next()
            .expect("there is always at least one built-in mode")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_mode_exists_and_is_the_general_one() {
        assert_eq!(default_mode().id, DEFAULT_MODE_ID);
    }

    /// The default is what a user gets without choosing, so it must be the
    /// conservative one: it cites, and it reads only the conversation.
    #[test]
    fn the_default_mode_is_the_careful_one() {
        let mode = default_mode();
        assert_eq!(mode.live.evidence_policy, EvidencePolicy::SourceFirst);
        assert!(mode.live.cite_required);
        assert_eq!(mode.usable_sources(), vec![AllowedSource::Transcript]);
    }

    #[test]
    fn a_custom_mode_cannot_take_a_built_in_id() {
        let ids: Vec<String> = BUILTIN_MODE_IDS.iter().map(|s| s.to_string()).collect();
        let names: Vec<String> = builtin_modes().into_iter().map(|m| m.name).collect();
        let mut clash = default_mode();
        clash.name = "Something else".into();
        assert!(matches!(
            validator::validate_mode(&clash, &ids, &names),
            Err(ModeError::IdCollision(_))
        ));
    }
}
