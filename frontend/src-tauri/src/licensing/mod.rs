//! Licensing & trial (ADR-0023): Polar.sh license keys + a client-side 14-day
//! trial, with local-first guardrails.
//!
//! Single source of truth for the app's licensing state. The UI consumes exactly
//! three Tauri commands ([`commands::get_licensing_status`],
//! [`commands::activate_license`], [`commands::deactivate_license`]); gated
//! capture commands call [`state::check_capture_allowed`].
//!
//! ## Layout
//! - [`store`] — the per-workspace, non-secret `licensingState` JSON blob
//!   (trial anchor copy, clock high-water, license display cache).
//! - [`trial`] — trial mechanics: dual-store anchor (earliest wins, each side
//!   heals the other), clock-rollback guard, ceil-semantics `days_left`.
//! - [`polar`] — the Polar customer-portal client behind the [`polar::LicenseApi`]
//!   trait (public endpoints only; no API secret in the binary).
//! - [`state`] — the state machine ([`LicensingStatus`] evaluation), the lazy
//!   ≤1/7-day background validation policy, and the capture gate.
//! - [`commands`] — the Tauri command surface (registered in `lib.rs`).
//!
//! ## Invariants (ADR-0023, constitution)
//! - **Local-first / fail-open:** status evaluation NEVER touches the network;
//!   validation is fire-and-forget and transport failures keep the user
//!   `Licensed`. The trial runs fully offline.
//! - **Secrets:** the license key and activation id live in the OS keychain only
//!   (`secrets::licensing`) — never SQLite, never logs (§7).
//! - **Data is never hostage (§5):** `TrialExpired`/`Revoked` gate exactly two
//!   entry points — starting a new recording and importing audio. Everything
//!   else (browse, search, playback, export, summaries of existing content,
//!   settings) stays available forever.

pub mod commands;
pub mod polar;
pub mod state;
pub mod store;
pub mod trial;

use serde::{Deserialize, Serialize};

/// Trial length in days (ADR-0023 §4: 14 days, full-featured).
pub const TRIAL_DAYS: i64 = 14;

/// Discriminant of [`LicensingStatus::state`]. Serialized snake_case, matching
/// the FROZEN frontend contract: `"trial" | "trial_expired" | "licensed" |
/// "revoked"`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LicensingStateKind {
    Trial,
    TrialExpired,
    Licensed,
    Revoked,
}

/// The status object the UI consumes (FROZEN CONTRACT — the frontend is built
/// against this exact camelCase shape; do not rename fields):
///
/// ```json
/// { "state": "trial" | "trial_expired" | "licensed" | "revoked",
///   "daysLeft": 14,            // number | null — only for trial
///   "plan": "pro",             // string | null
///   "expiresAt": "2027-...",   // ISO8601 | null — from Polar expires_at
///   "displayKey": "MITYU-****-****-1234",  // string | null — masked, never the raw key
///   "reason": "...",           // string | null — human sentence for revoked
///   "configured": true }       // false when MITYU_POLAR_ORG_ID is absent
/// ```
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LicensingStatus {
    pub state: LicensingStateKind,
    /// Whole days of trial remaining (ceil semantics: day 0 = 14 left). `None`
    /// outside the trial state.
    pub days_left: Option<i64>,
    /// Plan label, e.g. `"pro"`. Polar's validate payload carries benefit ids,
    /// not names, so this defaults to `"pro"` while licensed (ADR-0023 §10).
    pub plan: Option<String>,
    /// License expiry (ISO 8601) as last reported by Polar; `None` = perpetual.
    pub expires_at: Option<String>,
    /// Masked key for the UI (see [`state::mask_key`]). The raw key never
    /// leaves the keychain.
    pub display_key: Option<String>,
    /// Human sentence explaining a `revoked` state.
    pub reason: Option<String>,
    /// `false` when the build carries no `MITYU_POLAR_ORG_ID` (activation is
    /// unavailable; trial mechanics still fully work).
    pub configured: bool,
}

impl LicensingStatus {
    /// A pure-trial status with `days_left` remaining days.
    pub(crate) fn trial(days_left: i64) -> Self {
        Self {
            state: LicensingStateKind::Trial,
            days_left: Some(days_left),
            plan: None,
            expires_at: None,
            display_key: None,
            reason: None,
            configured: polar::is_configured(),
        }
    }

    /// The expired-trial status.
    pub(crate) fn trial_expired() -> Self {
        Self {
            state: LicensingStateKind::TrialExpired,
            days_left: None,
            plan: None,
            expires_at: None,
            display_key: None,
            reason: None,
            configured: polar::is_configured(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The FROZEN wire shape: exact key set, camelCase names, snake_case state
    /// tokens. The frontend is built against this in parallel — a failure here
    /// means the contract drifted.
    #[test]
    fn licensing_status_serializes_to_frozen_contract() {
        let status = LicensingStatus {
            state: LicensingStateKind::TrialExpired,
            days_left: None,
            plan: None,
            expires_at: None,
            display_key: None,
            reason: None,
            configured: false,
        };
        // The exact wire string (struct declaration order): field NAMES are the
        // contract; a rename or a dropped field fails here.
        assert_eq!(
            serde_json::to_string(&status).expect("serialize"),
            r#"{"state":"trial_expired","daysLeft":null,"plan":null,"expiresAt":null,"displayKey":null,"reason":null,"configured":false}"#
        );

        let json = serde_json::to_value(&status).expect("serialize");
        let obj = json.as_object().expect("object");
        let mut keys: Vec<&str> = obj.keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            vec![
                "configured",
                "daysLeft",
                "displayKey",
                "expiresAt",
                "plan",
                "reason",
                "state"
            ],
            "key set must match the frozen contract"
        );
        assert_eq!(json["state"], "trial_expired");
        assert_eq!(json["daysLeft"], serde_json::Value::Null);
        assert_eq!(json["configured"], false);
    }

    #[test]
    fn state_tokens_are_snake_case() {
        for (kind, token) in [
            (LicensingStateKind::Trial, "trial"),
            (LicensingStateKind::TrialExpired, "trial_expired"),
            (LicensingStateKind::Licensed, "licensed"),
            (LicensingStateKind::Revoked, "revoked"),
        ] {
            assert_eq!(serde_json::to_value(kind).unwrap(), token);
        }
    }

    #[test]
    fn trial_status_carries_days_left() {
        let status = LicensingStatus::trial(14);
        assert_eq!(status.state, LicensingStateKind::Trial);
        assert_eq!(status.days_left, Some(14));
        assert_eq!(status.plan, None);
    }
}
