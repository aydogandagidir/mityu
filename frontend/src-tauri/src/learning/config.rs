//! Per-workspace learning policy (ADR-0024 §7), stored as a small JSON blob in
//! `settings.learningConfig` — the `redactionConfig` pattern of `20260704000000`.
//!
//! Every field carries `#[serde(default)]`, so an older or partial blob parses
//! and fills the gaps with the product default rather than failing. That matters:
//! it keeps true parse failure rare, and rare is what lets
//! [`LearningConfig::disabled`] be as blunt as it is.

use serde::{Deserialize, Serialize};

/// How many corrections must back a mined rule before auto-activation will
/// consider it. Three is the smallest number that distinguishes a habit from a
/// coincidence — two identical corrections happen by accident.
pub const DEFAULT_AUTO_ACTIVATE_MIN_SUPPORT: i64 = 3;

fn default_true() -> bool {
    true
}

fn default_min_support() -> i64 {
    DEFAULT_AUTO_ACTIVATE_MIN_SUPPORT
}

/// The workspace's learning policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LearningConfig {
    /// Master switch: capture corrections, mine them, inject the results.
    /// Off means the app behaves exactly as it did before ADR-0024.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Whether a mined rule with enough support goes straight to `Active`
    /// instead of waiting in `Proposed` for a human.
    ///
    /// **On by default, and that is a deliberate, bounded decision** (ADR-0024
    /// §7). It is safe only because three other things are true, and it must be
    /// revisited if any of them stops being true: (1) the output HITL gate still
    /// requires a human to approve every block, so a rule can only change what a
    /// DRAFT looks like; (2) every generation snapshots the rules that shaped it
    /// (§5), so an auto-activated rule always leaves a trace; (3) every rule is
    /// visible, editable and deletable on the rules screen (§9).
    #[serde(default = "default_true")]
    pub auto_activate: bool,

    /// Support threshold for `auto_activate`.
    #[serde(default = "default_min_support")]
    pub auto_activate_min_support: i64,

    /// Whether the BYOK provider may be asked to mine patterns the deterministic
    /// miners cannot see (§8).
    ///
    /// **Off by default, unlike the rest** — not for privacy (the transcript
    /// already goes to that provider to be summarized, so this opens no new
    /// surface) but because it SPENDS THE USER'S OWN API BUDGET. Enabling a paid
    /// background process on someone's behalf is not a default anyone gets to
    /// choose for them.
    #[serde(default)]
    pub llm_miner_enabled: bool,
}

impl Default for LearningConfig {
    /// What a FRESH workspace gets: learning on, auto-activation on, the LLM
    /// miner off.
    fn default() -> Self {
        Self {
            enabled: true,
            auto_activate: true,
            auto_activate_min_support: DEFAULT_AUTO_ACTIVATE_MIN_SUPPORT,
            llm_miner_enabled: false,
        }
    }
}

impl LearningConfig {
    /// What a workspace gets when its STORED blob cannot be parsed.
    ///
    /// Deliberately NOT [`Self::default`], and this is the one place the two must
    /// not be confused. A corrupt blob means the user *had* preferences and we
    /// cannot read them — falling back to the fresh-install default would
    /// re-enable auto-activation for someone who had deliberately switched it
    /// off, silently applying a policy they refused. So: do nothing until they
    /// save a config we can read. This mirrors the redaction repository's rule
    /// that a bad blob must never *enable* redaction; the asymmetry is only that
    /// redaction's safe fallback happens to equal its default and learning's does
    /// not.
    ///
    /// Absent config is a different case entirely and still gets
    /// [`Self::default`]: no row means no preference was ever expressed, not that
    /// one was lost.
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            auto_activate: false,
            auto_activate_min_support: DEFAULT_AUTO_ACTIVATE_MIN_SUPPORT,
            llm_miner_enabled: false,
        }
    }

    /// Whether a mined rule with `support_count` behind it may skip the
    /// `Proposed` queue.
    pub fn may_auto_activate(&self, support_count: i64) -> bool {
        self.enabled && self.auto_activate && support_count >= self.auto_activate_min_support
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_workspace_learns_and_auto_activates_but_spends_nothing() {
        let config = LearningConfig::default();
        assert!(config.enabled);
        assert!(config.auto_activate);
        assert_eq!(config.auto_activate_min_support, 3);
        assert!(
            !config.llm_miner_enabled,
            "the LLM miner spends the user's own API budget; it cannot be a default",
        );
    }

    /// The distinction this whole module exists to preserve.
    #[test]
    fn the_corrupt_blob_fallback_is_not_the_fresh_install_default() {
        let corrupt = LearningConfig::disabled();
        assert!(!corrupt.auto_activate);
        assert!(!corrupt.enabled);
        assert_ne!(corrupt, LearningConfig::default());
    }

    #[test]
    fn auto_activation_needs_the_switch_the_master_and_the_support() {
        let config = LearningConfig::default();
        assert!(!config.may_auto_activate(2), "below threshold");
        assert!(config.may_auto_activate(3), "at threshold");
        assert!(config.may_auto_activate(9));

        let off = LearningConfig {
            auto_activate: false,
            ..LearningConfig::default()
        };
        assert!(!off.may_auto_activate(9));

        // The master switch overrides everything below it.
        let disabled = LearningConfig {
            enabled: false,
            ..LearningConfig::default()
        };
        assert!(!disabled.may_auto_activate(9));
    }

    /// Forward-compat: a blob written by an older build is missing fields, and
    /// must fill them from the product default rather than fail — otherwise every
    /// added field would tip working users into `disabled()`.
    #[test]
    fn a_partial_blob_fills_missing_fields_from_the_default() {
        let config: LearningConfig =
            serde_json::from_str(r#"{"autoActivate": false}"#).expect("partial blob parses");
        assert!(!config.auto_activate, "the stored preference wins");
        assert!(config.enabled, "the absent field takes the product default");
        assert_eq!(config.auto_activate_min_support, 3);
        assert!(!config.llm_miner_enabled);
    }

    #[test]
    fn the_wire_shape_is_camel_case_like_the_other_settings_blobs() {
        let json = serde_json::to_string(&LearningConfig::default()).unwrap();
        assert!(json.contains("\"autoActivate\""), "got: {json}");
        assert!(json.contains("\"autoActivateMinSupport\""), "got: {json}");
        assert!(json.contains("\"llmMinerEnabled\""), "got: {json}");
    }
}
