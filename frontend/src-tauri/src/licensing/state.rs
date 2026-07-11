//! The licensing state machine (ADR-0023 §9): evaluates
//! `Trial / TrialExpired / Licensed / Revoked` from the two local stores, applies
//! the lazy validation policy, and hosts the capture gate.
//!
//! **Evaluation never touches the network.** A status read is pure local IO
//! (settings blob + keychain); Polar validation runs as a separate
//! fire-and-forget task the command layer spawns at most once per 7 days
//! (ADR-0023 §7). Transport failures keep the user `Licensed` (local-first,
//! fail-open); only explicit negatives act:
//!
//! | validate outcome            | effect                                        |
//! |-----------------------------|-----------------------------------------------|
//! | transport error / timeout   | stay `Licensed`, stamp the attempt            |
//! | `granted`                   | refresh `expires_at`, clear any revoked flag  |
//! | `revoked` / `disabled`      | `Revoked{reason}` (re-activation path in UI)  |
//! | past `expires_at` (cached)  | `Revoked{expired}` — checked offline each read|
//! | 404 (seat freed via portal) | clear keychain license, fall back to trial    |

use super::polar::{ActivateOutcome, DeactivateOutcome, LicenseApi, ValidateOutcome};
use super::store::{self, LicensingBlob};
use super::{trial, LicensingStateKind, LicensingStatus};
use crate::context::AuthContext;
use chrono::{DateTime, Duration, Utc};
use sqlx::SqlitePool;

/// Machine-readable prefix for the capture-gate error. The UI maps it to the
/// paywall dialog. FROZEN — the frontend matches on it.
pub const LICENSE_REQUIRED_PREFIX: &str = "LICENSE_REQUIRED:";

/// Machine-readable prefix returned by `activate_license` when the build
/// carries no `MITYU_POLAR_ORG_ID`. FROZEN — the frontend matches on it.
pub const NOT_CONFIGURED_PREFIX: &str = "NOT_CONFIGURED:";

/// Default plan label while Polar's payload carries only benefit ids
/// (ADR-0023 §10).
const DEFAULT_PLAN: &str = "pro";

/// Re-validate at most once per this many days (ADR-0023 §7).
const VALIDATE_EVERY_DAYS: i64 = 7;

/// A status evaluation plus the hint the command layer uses to decide whether
/// to spawn the fire-and-forget background validation.
#[derive(Clone, Debug)]
pub struct Evaluation {
    pub status: LicensingStatus,
    /// A license is stored locally and its last validation attempt is older
    /// than the 7-day policy (or never happened). The caller must still check
    /// `polar::is_configured()` before spawning.
    pub validation_due: bool,
}

/// What the keychain says about a stored license key.
enum KeychainLicense {
    Present(String),
    Absent,
    /// The OS store itself is unreachable (locked keychain). Not the same as
    /// absent: we then fall back to the blob's display cache as evidence, so a
    /// locked keychain never demotes a licensed user (fail-open).
    Unavailable,
}

fn read_keychain_license() -> KeychainLicense {
    match crate::secrets::licensing::get(crate::secrets::licensing::KEY_ENTRY) {
        Ok(Some(key)) => KeychainLicense::Present(key),
        Ok(None) => KeychainLicense::Absent,
        Err(e) => {
            tracing::warn!(
                error = %format!("{e:#}"),
                "licensing: OS credential store unreachable while reading the license key"
            );
            KeychainLicense::Unavailable
        }
    }
}

/// Mask a license key for display: keep the first key segment and the last 4
/// characters, hide everything else (e.g. `MITYU-AAAA-BBBB-1234` →
/// `MITYU-****-****-1234`). Never returns enough of the key to reconstruct it.
pub fn mask_key(raw: &str) -> String {
    let key: Vec<char> = raw.trim().chars().collect();
    let n = key.len();
    if n <= 8 {
        return "****".to_string();
    }
    let last4: String = key[n - 4..].iter().collect();
    if n < 14 {
        return format!("****-{last4}");
    }
    // Prefix = the first '-'-separated segment when it is short, else 5 chars.
    let dash = key.iter().position(|&c| c == '-');
    let prefix_len = match dash {
        Some(i) if (1..=8).contains(&i) => i,
        _ => 5,
    };
    let prefix: String = key[..prefix_len].iter().collect();
    format!("{prefix}-****-****-{last4}")
}

/// Human sentence for a locally-detected expiry.
fn expired_reason(expires_at: &str) -> String {
    match trial::parse_ts(expires_at) {
        Some(ts) => format!("This license expired on {}.", ts.format("%Y-%m-%d")),
        None => "This license has expired.".to_string(),
    }
}

/// Evaluate the licensing status from local stores only (no network, ADR-0023
/// §7). Infallible by design: storage failures degrade to a keychain-only view
/// rather than erroring a status read.
pub async fn evaluate(pool: &SqlitePool, ctx: &AuthContext, now: DateTime<Utc>) -> Evaluation {
    let mut blob = match store::get_blob(pool, ctx).await {
        Ok(blob) => blob,
        Err(e) => {
            tracing::warn!(
                workspace_id = %ctx.tenant_id,
                error = %e,
                "licensing: could not read the licensingState blob; using keychain-only view"
            );
            return evaluate_without_db(now);
        }
    };
    let mut dirty = false;

    // Clock-rollback guard: effective time never runs backwards; persist the
    // high-water at most ~once/hour (ADR-0023 §4).
    let high_water = blob.last_seen_at.as_deref().and_then(trial::parse_ts);
    let guard = trial::clock_guard(high_water, now);
    if guard.persist_high_water {
        blob.last_seen_at = Some(guard.effective_now.to_rfc3339());
        dirty = true;
    }
    let effective_now = guard.effective_now;

    let license = match read_keychain_license() {
        KeychainLicense::Present(key) => Some(Some(key)),
        // Locked keychain + cached license evidence ⇒ stay licensed (fail-open).
        KeychainLicense::Unavailable if blob.display_key.is_some() => Some(None),
        KeychainLicense::Unavailable | KeychainLicense::Absent => None,
    };

    let status = match license {
        Some(raw_key) => {
            // Heal the display cache from the raw key if it was lost.
            if blob.display_key.is_none() {
                if let Some(key) = raw_key.as_deref() {
                    blob.display_key = Some(mask_key(key));
                    dirty = true;
                }
            }
            licensed_status(&blob, effective_now)
        }
        None => {
            // Trial state machine: dual-store anchor, earliest wins, heal both.
            let keychain_anchor = trial::read_keychain_anchor();
            let blob_anchor = blob.trial_anchor.as_deref().and_then(trial::parse_ts);
            let resolution = trial::resolve_anchor(keychain_anchor, blob_anchor, effective_now);
            if resolution.write_keychain {
                trial::write_keychain_anchor(resolution.anchor);
            }
            if resolution.write_blob {
                blob.trial_anchor = Some(resolution.anchor.to_rfc3339());
                dirty = true;
            }
            let days_left = trial::days_left(resolution.anchor, effective_now);
            if days_left > 0 {
                LicensingStatus::trial(days_left)
            } else {
                LicensingStatus::trial_expired()
            }
        }
    };

    let validation_due = matches!(
        status.state,
        LicensingStateKind::Licensed | LicensingStateKind::Revoked
    ) && should_validate(&blob, now);

    if dirty {
        if let Err(e) = store::set_blob(pool, ctx, &blob).await {
            tracing::warn!(
                workspace_id = %ctx.tenant_id,
                error = %e,
                "licensing: could not persist the licensingState blob (status still served)"
            );
        }
    }

    Evaluation {
        status,
        validation_due,
    }
}

/// Build the licensed/revoked status from the blob's display cache. The cached
/// `expires_at` is honored offline (ADR-0023 §10: expiry works from day one).
fn licensed_status(blob: &LicensingBlob, effective_now: DateTime<Utc>) -> LicensingStatus {
    let expired = blob
        .expires_at
        .as_deref()
        .and_then(trial::parse_ts)
        .is_some_and(|ts| ts < effective_now);

    let (state, reason) = if let Some(reason) = blob.revoked_reason.clone() {
        (LicensingStateKind::Revoked, Some(reason))
    } else if expired {
        (
            LicensingStateKind::Revoked,
            Some(expired_reason(
                blob.expires_at.as_deref().unwrap_or_default(),
            )),
        )
    } else {
        (LicensingStateKind::Licensed, None)
    };

    LicensingStatus {
        state,
        days_left: None,
        plan: Some(
            blob.plan
                .clone()
                .unwrap_or_else(|| DEFAULT_PLAN.to_string()),
        ),
        expires_at: blob.expires_at.clone(),
        display_key: blob.display_key.clone(),
        reason,
        configured: super::polar::is_configured(),
    }
}

/// Degraded evaluation when the DB is not available yet (first launch before
/// database initialization) or the blob read failed: keychain only, no healing
/// into the blob, no rollback guard, and no background validation
/// (`validation_due` = false — without the blob there is no 7-day throttle to
/// consult, and an unthrottled phone-home would violate ADR-0023 §7).
pub fn evaluate_without_db(now: DateTime<Utc>) -> Evaluation {
    let status = match read_keychain_license() {
        KeychainLicense::Present(key) => LicensingStatus {
            state: LicensingStateKind::Licensed,
            days_left: None,
            plan: Some(DEFAULT_PLAN.to_string()),
            expires_at: None,
            display_key: Some(mask_key(&key)),
            reason: None,
            configured: super::polar::is_configured(),
        },
        KeychainLicense::Absent | KeychainLicense::Unavailable => {
            let anchor = match trial::read_keychain_anchor() {
                Some(anchor) => anchor,
                None => {
                    // True first launch (pre-DB): mint the anchor now; the blob
                    // copy heals in on the first evaluated read after DB init.
                    trial::write_keychain_anchor(now);
                    now
                }
            };
            let days_left = trial::days_left(anchor, now);
            if days_left > 0 {
                LicensingStatus::trial(days_left)
            } else {
                LicensingStatus::trial_expired()
            }
        }
    };
    Evaluation {
        status,
        validation_due: false,
    }
}

/// The ≤ once-per-7-days validation throttle (ADR-0023 §7). Counts *attempts*
/// (`last_validated_at` is stamped on transport failures too), so an offline
/// week costs one failed attempt, not one per status read.
pub fn should_validate(blob: &LicensingBlob, now: DateTime<Utc>) -> bool {
    match blob.last_validated_at.as_deref().and_then(trial::parse_ts) {
        Some(last) => now - last > Duration::days(VALIDATE_EVERY_DAYS),
        None => true,
    }
}

/// The capture gate (ADR-0023 §5): `TrialExpired`/`Revoked` block exactly the
/// two new-capture entry points; `Trial`/`Licensed` pass. Internal evaluation
/// problems fail OPEN (never block capture on a licensing bug — data first).
pub async fn check_capture_allowed(
    pool: &SqlitePool,
    ctx: &AuthContext,
    now: DateTime<Utc>,
) -> Result<(), String> {
    let status = evaluate(pool, ctx, now).await.status;
    match status.state {
        LicensingStateKind::Trial | LicensingStateKind::Licensed => Ok(()),
        LicensingStateKind::TrialExpired => Err(format!(
            "{LICENSE_REQUIRED_PREFIX} Your 14-day free trial has ended. Enter a license key to \
             start new recordings or imports — every existing meeting stays fully accessible."
        )),
        LicensingStateKind::Revoked => {
            let reason = status
                .reason
                .unwrap_or_else(|| "Your license is no longer active.".to_string());
            Err(format!(
                "{LICENSE_REQUIRED_PREFIX} {reason} Enter a valid license key to start new \
                 recordings or imports."
            ))
        }
    }
}

/// The `activate_license` core (command-testable; the Tauri wrapper adds the
/// org-id/configured check and passes the real [`super::polar::PolarApi`]).
pub async fn activate(
    pool: &SqlitePool,
    ctx: &AuthContext,
    api: &dyn LicenseApi,
    org_id: &str,
    raw_key: &str,
    label: &str,
    now: DateTime<Utc>,
) -> Result<LicensingStatus, String> {
    let key = raw_key.trim();
    if key.is_empty() {
        return Err("Please enter a license key.".to_string());
    }

    let outcome = match api.activate(key, org_id, label).await {
        Ok(outcome) => outcome,
        Err(e) => {
            tracing::warn!(error = %format!("{e:#}"), "licensing: activate request failed (transport)");
            return Err(
                "Could not reach the licensing service. Check your internet connection and try \
                 again."
                    .to_string(),
            );
        }
    };

    let (activation_id, expires_at) = match outcome {
        ActivateOutcome::Activated {
            activation_id,
            expires_at,
        } => (activation_id, expires_at),
        ActivateOutcome::LimitReached => {
            return Err(
                "This license key has reached its device activation limit. Deactivate it on \
                 another device (or in the Polar customer portal) and try again."
                    .to_string(),
            )
        }
        ActivateOutcome::KeyNotFound => {
            return Err("License key not found. Check the key for typos and try again.".to_string())
        }
        ActivateOutcome::Invalid => {
            return Err(
                "That license key looks invalid. Paste it exactly as it appears in your purchase \
                 email."
                    .to_string(),
            )
        }
    };

    // Persist key + activation id in the KEYCHAIN ONLY (constitution §7). If
    // the keychain rejects the write, free the seat again (best-effort) so the
    // failed attempt does not burn one of the user's activations.
    let store_result = crate::secrets::licensing::set(crate::secrets::licensing::KEY_ENTRY, key)
        .and_then(|()| {
            crate::secrets::licensing::set(
                crate::secrets::licensing::ACTIVATION_ID_ENTRY,
                &activation_id,
            )
        });
    if let Err(e) = store_result {
        tracing::error!(error = %format!("{e:#}"), "licensing: could not store the license in the OS credential store");
        let _ = crate::secrets::licensing::delete(crate::secrets::licensing::KEY_ENTRY);
        if let Err(rollback) = api.deactivate(key, org_id, &activation_id).await {
            tracing::warn!(error = %format!("{rollback:#}"), "licensing: could not roll back the activation after a keychain failure");
        }
        return Err(
            "Could not store the license in the system keychain. Unlock your keychain and try \
             again."
                .to_string(),
        );
    }

    // Non-secret display cache in the settings blob (best-effort — the license
    // itself is already safely in the keychain).
    match store::get_blob(pool, ctx).await {
        Ok(mut blob) => {
            blob.display_key = Some(mask_key(key));
            blob.plan = Some(DEFAULT_PLAN.to_string());
            blob.expires_at = expires_at;
            blob.last_validated_at = Some(now.to_rfc3339());
            blob.revoked_reason = None;
            if let Err(e) = store::set_blob(pool, ctx, &blob).await {
                tracing::warn!(error = %e, "licensing: could not cache license display state");
            }
        }
        Err(e) => {
            tracing::warn!(error = %e, "licensing: could not read licensingState blob after activation");
        }
    }

    tracing::info!("licensing: license activated on this device");
    Ok(evaluate(pool, ctx, now).await.status)
}

/// The `deactivate_license` core: best-effort remote deactivate (a network
/// failure is logged, never blocks — the seat can also be freed via Polar's
/// portal), then the keychain entries are cleared and the trial state machine
/// takes over again.
pub async fn deactivate(
    pool: &SqlitePool,
    ctx: &AuthContext,
    api: &dyn LicenseApi,
    org_id: Option<&str>,
    now: DateTime<Utc>,
) -> Result<LicensingStatus, String> {
    let key = crate::secrets::licensing::get(crate::secrets::licensing::KEY_ENTRY)
        .map_err(|e| keychain_unavailable(&e))?;
    let activation_id =
        crate::secrets::licensing::get(crate::secrets::licensing::ACTIVATION_ID_ENTRY)
            .map_err(|e| keychain_unavailable(&e))?;

    if let (Some(org), Some(key), Some(activation_id)) =
        (org_id, key.as_deref(), activation_id.as_deref())
    {
        match api.deactivate(key, org, activation_id).await {
            Ok(DeactivateOutcome::Done) => {
                tracing::info!("licensing: seat deactivated at Polar");
            }
            Ok(DeactivateOutcome::NotFound) => {
                tracing::info!("licensing: seat was already gone at Polar");
            }
            Ok(DeactivateOutcome::Invalid) => {
                tracing::warn!("licensing: Polar rejected the deactivate request (422); clearing locally anyway");
            }
            Err(e) => {
                tracing::warn!(
                    error = %format!("{e:#}"),
                    "licensing: network deactivate failed; clearing the local license anyway \
                     (the seat can be freed later via the Polar customer portal)"
                );
            }
        }
    }

    // Clearing the local entries must succeed — otherwise the device still
    // *has* a license and reporting trial state would lie.
    crate::secrets::licensing::delete(crate::secrets::licensing::KEY_ENTRY)
        .map_err(|e| keychain_unavailable(&e))?;
    crate::secrets::licensing::delete(crate::secrets::licensing::ACTIVATION_ID_ENTRY)
        .map_err(|e| keychain_unavailable(&e))?;

    clear_license_cache(pool, ctx).await;
    Ok(evaluate(pool, ctx, now).await.status)
}

fn keychain_unavailable(e: &anyhow::Error) -> String {
    tracing::error!(error = %format!("{e:#}"), "licensing: OS credential store unavailable");
    "The system keychain is unavailable, so the stored license could not be changed. Unlock your \
     keychain and try again."
        .to_string()
}

/// Drop the non-secret license display cache from the blob, preserving the
/// trial anchor and the clock high-water.
async fn clear_license_cache(pool: &SqlitePool, ctx: &AuthContext) {
    match store::get_blob(pool, ctx).await {
        Ok(mut blob) => {
            blob.display_key = None;
            blob.plan = None;
            blob.expires_at = None;
            blob.last_validated_at = None;
            blob.revoked_reason = None;
            if let Err(e) = store::set_blob(pool, ctx, &blob).await {
                tracing::warn!(error = %e, "licensing: could not clear the license display cache");
            }
        }
        Err(e) => {
            tracing::warn!(error = %e, "licensing: could not read licensingState blob while clearing");
        }
    }
}

/// The background validation pass (ADR-0023 §7). Fire-and-forget: the caller
/// spawns it AFTER answering the status read — it must never block a read or
/// startup on the network. All outcomes are applied to the blob; the next
/// status read reflects them.
pub async fn run_validation(
    pool: &SqlitePool,
    ctx: &AuthContext,
    api: &dyn LicenseApi,
    org_id: &str,
    now: DateTime<Utc>,
) {
    let key = match crate::secrets::licensing::get(crate::secrets::licensing::KEY_ENTRY) {
        Ok(Some(key)) => key,
        Ok(None) => return, // nothing to validate
        Err(e) => {
            tracing::warn!(error = %format!("{e:#}"), "licensing: skipping validation (keychain unavailable)");
            return;
        }
    };
    let activation_id = match crate::secrets::licensing::get(
        crate::secrets::licensing::ACTIVATION_ID_ENTRY,
    ) {
        Ok(Some(id)) => id,
        Ok(None) => {
            tracing::warn!("licensing: stored key has no activation id; skipping validation");
            return;
        }
        Err(e) => {
            tracing::warn!(error = %format!("{e:#}"), "licensing: skipping validation (keychain unavailable)");
            return;
        }
    };

    let mut blob = match store::get_blob(pool, ctx).await {
        Ok(blob) => blob,
        Err(e) => {
            tracing::warn!(error = %e, "licensing: skipping validation (blob unreadable)");
            return;
        }
    };

    // Every path below stamps the attempt — that is what the 7-day throttle
    // rate-limits (ids-only phone-home, ADR-0023 §7).
    blob.last_validated_at = Some(now.to_rfc3339());

    match api.validate(&key, org_id, &activation_id).await {
        Err(e) => {
            // Transport error/timeout ⇒ remain Licensed (local-first fail-open).
            tracing::warn!(
                error = %format!("{e:#}"),
                "licensing: validation transport error; keeping the local license state (fail-open)"
            );
        }
        Ok(ValidateOutcome::Granted { expires_at }) => {
            tracing::info!("licensing: validation granted");
            blob.expires_at = expires_at;
            blob.revoked_reason = None;
            if blob.plan.is_none() {
                blob.plan = Some(DEFAULT_PLAN.to_string());
            }
            if blob.display_key.is_none() {
                blob.display_key = Some(mask_key(&key));
            }
        }
        Ok(ValidateOutcome::Revoked) => {
            tracing::warn!("licensing: validation says the key is revoked");
            blob.revoked_reason = Some("This license key was revoked by the seller.".to_string());
        }
        Ok(ValidateOutcome::Disabled) => {
            tracing::warn!("licensing: validation says the key is disabled");
            blob.revoked_reason = Some("This license key was disabled.".to_string());
        }
        Ok(ValidateOutcome::NotFound) => {
            // Our activation was removed via the Polar portal: clear the local
            // license and fall back to the trial state machine (which may be
            // TrialExpired). Trial anchor and high-water are preserved.
            tracing::warn!(
                "licensing: activation no longer exists at Polar (freed via portal); \
                 clearing the local license and falling back to the trial"
            );
            if let Err(e) = crate::secrets::licensing::delete(crate::secrets::licensing::KEY_ENTRY)
            {
                tracing::warn!(error = %format!("{e:#}"), "licensing: could not clear the stored key");
            }
            if let Err(e) =
                crate::secrets::licensing::delete(crate::secrets::licensing::ACTIVATION_ID_ENTRY)
            {
                tracing::warn!(error = %format!("{e:#}"), "licensing: could not clear the stored activation id");
            }
            blob.display_key = None;
            blob.plan = None;
            blob.expires_at = None;
            blob.revoked_reason = None;
        }
        Ok(ValidateOutcome::Invalid) => {
            // 422 on ids we stored verbatim — a schema drift, not a verdict on
            // the license. Fail open.
            tracing::warn!("licensing: validation request rejected (422); keeping local state");
        }
        Ok(ValidateOutcome::Unknown(status)) => {
            tracing::warn!(
                status,
                "licensing: unknown validation status; keeping local state"
            );
        }
    }

    if let Err(e) = store::set_blob(pool, ctx, &blob).await {
        tracing::warn!(error = %e, "licensing: could not persist the validation outcome");
    }
}

#[cfg(test)]
mod tests {
    use super::super::store::test_support::{keychain_guard, open_migrated_pool};
    use super::*;
    use crate::context::AuthContext;
    use crate::secrets::licensing as kc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    const ORG: &str = "org-test";

    fn ctx() -> AuthContext {
        AuthContext::local()
    }

    fn ts(raw: &str) -> DateTime<Utc> {
        trial::parse_ts(raw).expect("test timestamp")
    }

    /// Programmable [`LicenseApi`] fake. `None` outcome = transport failure.
    #[derive(Default)]
    struct FakeApi {
        activate_outcome: Option<ActivateOutcome>,
        validate_outcome: Option<ValidateOutcome>,
        deactivate_outcome: Option<DeactivateOutcome>,
        validate_calls: AtomicUsize,
        deactivate_calls: AtomicUsize,
    }

    #[async_trait::async_trait]
    impl LicenseApi for FakeApi {
        async fn activate(
            &self,
            _key: &str,
            _org: &str,
            _label: &str,
        ) -> anyhow::Result<ActivateOutcome> {
            self.activate_outcome
                .clone()
                .ok_or_else(|| anyhow::anyhow!("fake transport failure"))
        }
        async fn validate(
            &self,
            _key: &str,
            _org: &str,
            _activation_id: &str,
        ) -> anyhow::Result<ValidateOutcome> {
            self.validate_calls.fetch_add(1, Ordering::SeqCst);
            self.validate_outcome
                .clone()
                .ok_or_else(|| anyhow::anyhow!("fake transport failure"))
        }
        async fn deactivate(
            &self,
            _key: &str,
            _org: &str,
            _activation_id: &str,
        ) -> anyhow::Result<DeactivateOutcome> {
            self.deactivate_calls.fetch_add(1, Ordering::SeqCst);
            self.deactivate_outcome
                .clone()
                .ok_or_else(|| anyhow::anyhow!("fake transport failure"))
        }
    }

    fn install_license(key: &str, activation_id: &str) {
        kc::set(kc::KEY_ENTRY, key).expect("store key");
        kc::set(kc::ACTIVATION_ID_ENTRY, activation_id).expect("store activation id");
    }

    // ===== trial lifecycle =====

    #[tokio::test]
    async fn trial_counts_down_and_expires_at_day_14() {
        let _guard = keychain_guard().await;
        let (pool, _dir) = open_migrated_pool().await;
        let ctx = ctx();
        let t0 = ts("2026-07-01T10:00:00Z");

        // First read mints the anchor: full 14 days.
        let eval = evaluate(&pool, &ctx, t0).await;
        assert_eq!(eval.status.state, LicensingStateKind::Trial);
        assert_eq!(eval.status.days_left, Some(14));
        assert!(!eval.validation_due, "trial never validates");

        // Day 13 (13 full days elapsed) ⇒ 1 day left, still trial.
        let eval = evaluate(&pool, &ctx, t0 + Duration::days(13)).await;
        assert_eq!(eval.status.state, LicensingStateKind::Trial);
        assert_eq!(eval.status.days_left, Some(1));

        // Day 14 ⇒ expired.
        let eval = evaluate(&pool, &ctx, t0 + Duration::days(14)).await;
        assert_eq!(eval.status.state, LicensingStateKind::TrialExpired);
        assert_eq!(eval.status.days_left, None);
    }

    #[tokio::test]
    async fn trial_anchor_earliest_wins_and_heals_both_stores() {
        let _guard = keychain_guard().await;
        let (pool, _dir) = open_migrated_pool().await;
        let ctx = ctx();
        let early = ts("2026-07-01T00:00:00Z");
        let late = ts("2026-07-06T00:00:00Z");

        // Keychain carries the EARLIER anchor; the blob a later (reset?) one.
        trial::write_keychain_anchor(early);
        let blob = LicensingBlob {
            trial_anchor: Some(late.to_rfc3339()),
            ..Default::default()
        };
        store::set_blob(&pool, &ctx, &blob)
            .await
            .expect("seed blob");

        // Day count follows the EARLIEST anchor (10 elapsed ⇒ 4 left)…
        let now = ts("2026-07-11T00:00:00Z");
        let eval = evaluate(&pool, &ctx, now).await;
        assert_eq!(eval.status.days_left, Some(4));

        // …and the blob healed down to it.
        let healed = store::get_blob(&pool, &ctx).await.expect("get");
        assert_eq!(
            healed.trial_anchor.as_deref().and_then(trial::parse_ts),
            Some(early)
        );

        // Opposite direction: wipe the keychain copy; it heals from the blob.
        kc::delete(kc::TRIAL_ANCHOR_ENTRY).expect("wipe keychain anchor");
        let eval = evaluate(&pool, &ctx, now).await;
        assert_eq!(eval.status.days_left, Some(4), "anchor survived via blob");
        assert_eq!(
            trial::read_keychain_anchor(),
            Some(early),
            "keychain healed"
        );
    }

    #[tokio::test]
    async fn clock_rollback_does_not_increase_days_left() {
        let _guard = keychain_guard().await;
        let (pool, _dir) = open_migrated_pool().await;
        let ctx = ctx();
        let t0 = ts("2026-07-01T00:00:00Z");

        let eval = evaluate(&pool, &ctx, t0).await;
        assert_eq!(eval.status.days_left, Some(14));

        // Ten days pass (high-water advances to t0+10d).
        let eval = evaluate(&pool, &ctx, t0 + Duration::days(10)).await;
        assert_eq!(eval.status.days_left, Some(4));

        // The user sets the clock back eight days: days_left must NOT grow.
        let eval = evaluate(&pool, &ctx, t0 + Duration::days(2)).await;
        assert_eq!(
            eval.status.days_left,
            Some(4),
            "rolled-back clock must not extend the trial"
        );
    }

    // ===== licensed lifecycle =====

    #[tokio::test]
    async fn stored_license_evaluates_licensed_and_heals_display_key() {
        let _guard = keychain_guard().await;
        let (pool, _dir) = open_migrated_pool().await;
        let ctx = ctx();
        install_license("MITYU-AAAA-BBBB-1234", "act-1");

        let eval = evaluate(&pool, &ctx, ts("2026-07-01T00:00:00Z")).await;
        assert_eq!(eval.status.state, LicensingStateKind::Licensed);
        assert_eq!(eval.status.plan.as_deref(), Some("pro"));
        assert_eq!(
            eval.status.display_key.as_deref(),
            Some("MITYU-****-****-1234")
        );
        assert!(
            eval.validation_due,
            "never-validated license is due for validation"
        );

        // The display key healed into the blob cache.
        let blob = store::get_blob(&pool, &ctx).await.expect("get");
        assert_eq!(blob.display_key.as_deref(), Some("MITYU-****-****-1234"));
    }

    #[tokio::test]
    async fn revoked_validation_flips_state_to_revoked() {
        let _guard = keychain_guard().await;
        let (pool, _dir) = open_migrated_pool().await;
        let ctx = ctx();
        install_license("MITYU-AAAA-BBBB-1234", "act-1");
        let now = ts("2026-07-01T00:00:00Z");
        assert_eq!(
            evaluate(&pool, &ctx, now).await.status.state,
            LicensingStateKind::Licensed
        );

        let api = FakeApi {
            validate_outcome: Some(ValidateOutcome::Revoked),
            ..Default::default()
        };
        run_validation(&pool, &ctx, &api, ORG, now).await;

        let status = evaluate(&pool, &ctx, now).await.status;
        assert_eq!(status.state, LicensingStateKind::Revoked);
        let reason = status.reason.expect("revoked carries a reason");
        assert!(reason.contains("revoked"), "reason: {reason}");
        // The key stays stored (re-activation path) — only 404 clears it.
        assert!(kc::get(kc::KEY_ENTRY).unwrap().is_some());
    }

    #[tokio::test]
    async fn validation_404_clears_license_and_falls_back_to_trial() {
        let _guard = keychain_guard().await;
        let (pool, _dir) = open_migrated_pool().await;
        let ctx = ctx();
        let t0 = ts("2026-07-01T00:00:00Z");

        // An old, already-expired trial anchor exists (pre-purchase usage)…
        evaluate(&pool, &ctx, t0).await;
        // …then the user licensed the device.
        install_license("MITYU-AAAA-BBBB-1234", "act-1");
        let now = t0 + Duration::days(30);
        assert_eq!(
            evaluate(&pool, &ctx, now).await.status.state,
            LicensingStateKind::Licensed
        );

        // The seat is freed via the Polar portal ⇒ validate 404s.
        let api = FakeApi {
            validate_outcome: Some(ValidateOutcome::NotFound),
            ..Default::default()
        };
        run_validation(&pool, &ctx, &api, ORG, now).await;

        // Keychain license is gone; the trial machine takes over — and this
        // trial expired 16 days ago.
        assert_eq!(kc::get(kc::KEY_ENTRY).unwrap(), None);
        assert_eq!(kc::get(kc::ACTIVATION_ID_ENTRY).unwrap(), None);
        let status = evaluate(&pool, &ctx, now).await.status;
        assert_eq!(status.state, LicensingStateKind::TrialExpired);
        assert_eq!(status.display_key, None);
    }

    #[tokio::test]
    async fn transport_error_keeps_licensed_and_stamps_attempt() {
        let _guard = keychain_guard().await;
        let (pool, _dir) = open_migrated_pool().await;
        let ctx = ctx();
        install_license("MITYU-AAAA-BBBB-1234", "act-1");
        let now = ts("2026-07-01T00:00:00Z");

        let api = FakeApi::default(); // validate ⇒ transport error
        run_validation(&pool, &ctx, &api, ORG, now).await;
        assert_eq!(api.validate_calls.load(Ordering::SeqCst), 1);

        // Fail-open: still licensed.
        let eval = evaluate(&pool, &ctx, now).await;
        assert_eq!(eval.status.state, LicensingStateKind::Licensed);
        // The attempt was stamped, so the 7-day throttle now holds…
        assert!(!eval.validation_due);
        let blob = store::get_blob(&pool, &ctx).await.expect("get");
        assert!(should_validate(&blob, now + Duration::days(8)));
        assert!(!should_validate(&blob, now + Duration::days(6)));
    }

    #[tokio::test]
    async fn granted_validation_refreshes_expiry_and_clears_revocation() {
        let _guard = keychain_guard().await;
        let (pool, _dir) = open_migrated_pool().await;
        let ctx = ctx();
        install_license("MITYU-AAAA-BBBB-1234", "act-1");
        let now = ts("2026-07-01T00:00:00Z");

        // Previously marked revoked (e.g. a billing hiccup, later resolved).
        let api = FakeApi {
            validate_outcome: Some(ValidateOutcome::Revoked),
            ..Default::default()
        };
        run_validation(&pool, &ctx, &api, ORG, now).await;
        assert_eq!(
            evaluate(&pool, &ctx, now).await.status.state,
            LicensingStateKind::Revoked
        );

        let api = FakeApi {
            validate_outcome: Some(ValidateOutcome::Granted {
                expires_at: Some("2027-07-01T00:00:00Z".into()),
            }),
            ..Default::default()
        };
        run_validation(&pool, &ctx, &api, ORG, now).await;
        let status = evaluate(&pool, &ctx, now).await.status;
        assert_eq!(status.state, LicensingStateKind::Licensed);
        assert_eq!(status.expires_at.as_deref(), Some("2027-07-01T00:00:00Z"));
        assert_eq!(status.reason, None);
    }

    #[tokio::test]
    async fn cached_past_expiry_is_honored_offline() {
        let _guard = keychain_guard().await;
        let (pool, _dir) = open_migrated_pool().await;
        let ctx = ctx();
        install_license("MITYU-AAAA-BBBB-1234", "act-1");
        let now = ts("2026-07-01T00:00:00Z");

        let api = FakeApi {
            validate_outcome: Some(ValidateOutcome::Granted {
                expires_at: Some("2026-08-01T00:00:00Z".into()),
            }),
            ..Default::default()
        };
        run_validation(&pool, &ctx, &api, ORG, now).await;
        assert_eq!(
            evaluate(&pool, &ctx, now).await.status.state,
            LicensingStateKind::Licensed
        );

        // Two months later — no network involved — the cached expiry gates.
        let later = ts("2026-09-01T00:00:00Z");
        let status = evaluate(&pool, &ctx, later).await.status;
        assert_eq!(status.state, LicensingStateKind::Revoked);
        assert!(status.reason.unwrap().contains("expired"));
    }

    // ===== activate / deactivate cores =====

    #[tokio::test]
    async fn activate_success_stores_keychain_and_returns_licensed() {
        let _guard = keychain_guard().await;
        let (pool, _dir) = open_migrated_pool().await;
        let ctx = ctx();
        let now = ts("2026-07-01T00:00:00Z");

        let api = FakeApi {
            activate_outcome: Some(ActivateOutcome::Activated {
                activation_id: "act-42".into(),
                expires_at: None,
            }),
            ..Default::default()
        };
        let status = activate(
            &pool,
            &ctx,
            &api,
            ORG,
            "  MITYU-AAAA-BBBB-1234  ",
            "host",
            now,
        )
        .await
        .expect("activation succeeds");

        assert_eq!(status.state, LicensingStateKind::Licensed);
        assert_eq!(status.display_key.as_deref(), Some("MITYU-****-****-1234"));
        assert_eq!(status.plan.as_deref(), Some("pro"));
        assert_eq!(
            kc::get(kc::KEY_ENTRY).unwrap().as_deref(),
            Some("MITYU-AAAA-BBBB-1234"),
            "key stored trimmed, keychain-only"
        );
        assert_eq!(
            kc::get(kc::ACTIVATION_ID_ENTRY).unwrap().as_deref(),
            Some("act-42")
        );
    }

    #[tokio::test]
    async fn activate_maps_limit_and_not_found_to_clear_errors() {
        let _guard = keychain_guard().await;
        let (pool, _dir) = open_migrated_pool().await;
        let ctx = ctx();
        let now = ts("2026-07-01T00:00:00Z");

        let api = FakeApi {
            activate_outcome: Some(ActivateOutcome::LimitReached),
            ..Default::default()
        };
        let err = activate(&pool, &ctx, &api, ORG, "K-123456789", "host", now)
            .await
            .expect_err("limit reached errors");
        assert!(err.contains("activation limit"), "err: {err}");

        let api = FakeApi {
            activate_outcome: Some(ActivateOutcome::KeyNotFound),
            ..Default::default()
        };
        let err = activate(&pool, &ctx, &api, ORG, "K-123456789", "host", now)
            .await
            .expect_err("unknown key errors");
        assert!(err.contains("not found"), "err: {err}");

        // Transport failure is a human sentence, not a panic; nothing stored.
        let api = FakeApi::default();
        let err = activate(&pool, &ctx, &api, ORG, "K-123456789", "host", now)
            .await
            .expect_err("transport error surfaces");
        assert!(err.contains("licensing service"), "err: {err}");
        assert_eq!(kc::get(kc::KEY_ENTRY).unwrap(), None);
    }

    #[tokio::test]
    async fn deactivate_clears_local_license_even_when_network_fails() {
        let _guard = keychain_guard().await;
        let (pool, _dir) = open_migrated_pool().await;
        let ctx = ctx();
        install_license("MITYU-AAAA-BBBB-1234", "act-1");
        let now = ts("2026-07-01T00:00:00Z");

        // Best-effort call fails (offline) — local clear still happens.
        let api = FakeApi::default(); // deactivate ⇒ transport error
        let status = deactivate(&pool, &ctx, &api, Some(ORG), now)
            .await
            .expect("deactivate succeeds locally");
        assert_eq!(api.deactivate_calls.load(Ordering::SeqCst), 1);
        assert_eq!(status.state, LicensingStateKind::Trial);
        assert_eq!(kc::get(kc::KEY_ENTRY).unwrap(), None);
        assert_eq!(kc::get(kc::ACTIVATION_ID_ENTRY).unwrap(), None);
        assert_eq!(status.display_key, None);
    }

    // ===== the capture gate =====

    #[tokio::test]
    async fn gate_blocks_expired_trial_with_license_required_prefix() {
        let _guard = keychain_guard().await;
        let (pool, _dir) = open_migrated_pool().await;
        let ctx = ctx();
        let t0 = ts("2026-07-01T00:00:00Z");

        // Fresh trial passes.
        assert!(check_capture_allowed(&pool, &ctx, t0).await.is_ok());

        // Expired trial blocks with the machine-readable prefix.
        let err = check_capture_allowed(&pool, &ctx, t0 + Duration::days(20))
            .await
            .expect_err("expired trial must gate capture");
        assert!(
            err.starts_with(LICENSE_REQUIRED_PREFIX),
            "gate error must start with {LICENSE_REQUIRED_PREFIX}, got: {err}"
        );

        // A license lifts the gate…
        install_license("MITYU-AAAA-BBBB-1234", "act-1");
        assert!(check_capture_allowed(&pool, &ctx, t0 + Duration::days(20))
            .await
            .is_ok());

        // …and a revocation re-arms it.
        let api = FakeApi {
            validate_outcome: Some(ValidateOutcome::Revoked),
            ..Default::default()
        };
        run_validation(&pool, &ctx, &api, ORG, t0 + Duration::days(20)).await;
        let err = check_capture_allowed(&pool, &ctx, t0 + Duration::days(20))
            .await
            .expect_err("revoked license must gate capture");
        assert!(err.starts_with(LICENSE_REQUIRED_PREFIX));
    }

    // ===== helpers =====

    #[test]
    fn mask_key_reveals_only_prefix_and_last4() {
        assert_eq!(mask_key("MITYU-AAAA-BBBB-1234"), "MITYU-****-****-1234");
        assert_eq!(
            mask_key("  0195f9a1-7c33-7a30-9f6d-83fcdd0acdc1  "),
            "0195f9a1-****-****-cdc1"
        );
        // Short keys reveal nothing.
        assert_eq!(mask_key("SHORTKEY"), "****");
        assert_eq!(mask_key(""), "****");
        // Mid-length keys reveal only the tail.
        assert_eq!(mask_key("ABCDEFGHIJKL"), "****-IJKL");
        // Dash-less long keys fall back to a 5-char prefix.
        assert_eq!(mask_key("ABCDEFGHIJKLMNOP"), "ABCDE-****-****-MNOP");
    }

    #[test]
    fn should_validate_honors_seven_day_throttle() {
        let now = ts("2026-07-10T00:00:00Z");
        let mut blob = LicensingBlob::default();
        assert!(should_validate(&blob, now), "never validated ⇒ due");
        blob.last_validated_at = Some(ts("2026-07-05T00:00:00Z").to_rfc3339());
        assert!(!should_validate(&blob, now), "5 days ago ⇒ not due");
        blob.last_validated_at = Some(ts("2026-07-01T00:00:00Z").to_rfc3339());
        assert!(should_validate(&blob, now), "9 days ago ⇒ due");
        blob.last_validated_at = Some("garbage".to_string());
        assert!(should_validate(&blob, now), "unparseable ⇒ due");
    }
}
