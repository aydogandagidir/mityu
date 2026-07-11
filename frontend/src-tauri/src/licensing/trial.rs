//! Trial mechanics (ADR-0023 §4): 14 days, full-featured, no account, no card.
//!
//! - **Dual-store anchor:** the first-launch timestamp is stored twice — in the
//!   per-workspace `licensingState` blob ([`super::store`]) and in the keychain
//!   entry `licensing:trial-anchor` ([`crate::secrets::licensing`]). The
//!   **earliest** stored value wins, and each store heals the other when one is
//!   missing (or lost — e.g. the app data dir was wiped but the keychain
//!   survived, or vice versa).
//! - **Clock-rollback guard:** a persisted high-water `last_seen_at`;
//!   `effective_now = max(system_now, high_water)`, so setting the system clock
//!   back never increases `days_left`. High-water writes are throttled to at
//!   most ~once/hour to avoid DB churn on frequent status reads.
//! - **Ceil semantics:** `days_left = 14 - floor(elapsed_days)` clamped to
//!   `[0, 14]` — on day 0 the UI shows "14 days left"; after 14 full days the
//!   trial is expired.
//!
//! This is deliberately an honest-user mechanism, not DRM (ADR-0023 §4): a
//! determined user can reset it; the privacy promise (no phone-home for the
//! trial) outranks enforcement strength.
//!
//! The functions here are pure (testable without IO) except the two thin
//! keychain accessors at the bottom.

use super::TRIAL_DAYS;
use chrono::{DateTime, Duration, Utc};

/// Minimum gap between persisted high-water updates (write throttle).
pub fn high_water_throttle() -> Duration {
    Duration::hours(1)
}

/// Lenient RFC 3339 parse for stored timestamps. `None` on any malformed value
/// (the caller then treats that store as missing and heals it from the other).
pub fn parse_ts(raw: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(raw.trim())
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
}

/// Outcome of [`resolve_anchor`]: the winning anchor plus which stores need to
/// be (re)written to converge on it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AnchorResolution {
    /// The trial anchor to use: the earliest surviving copy, or `now` when no
    /// store holds one (true first launch).
    pub anchor: DateTime<Utc>,
    /// The keychain copy is missing or later than `anchor` — heal it.
    pub write_keychain: bool,
    /// The blob copy is missing or later than `anchor` — heal it.
    pub write_blob: bool,
}

/// Earliest-wins anchor resolution over the two stores (pure).
pub fn resolve_anchor(
    keychain: Option<DateTime<Utc>>,
    blob: Option<DateTime<Utc>>,
    now: DateTime<Utc>,
) -> AnchorResolution {
    let anchor = match (keychain, blob) {
        (Some(k), Some(b)) => k.min(b),
        (Some(k), None) => k,
        (None, Some(b)) => b,
        (None, None) => now,
    };
    AnchorResolution {
        anchor,
        write_keychain: keychain != Some(anchor),
        write_blob: blob != Some(anchor),
    }
}

/// Outcome of [`clock_guard`]: the tamper-resistant "now" plus whether the
/// persisted high-water should be advanced (throttled).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClockGuard {
    /// `max(system_now, stored high-water)` — never runs backwards.
    pub effective_now: DateTime<Utc>,
    /// `true` when `system_now` is far enough past the stored high-water that
    /// it is worth persisting (`> high_water + 1h`, or no high-water yet).
    pub persist_high_water: bool,
}

/// Clock-rollback guard (pure). `high_water` is the stored `last_seen_at`.
pub fn clock_guard(high_water: Option<DateTime<Utc>>, system_now: DateTime<Utc>) -> ClockGuard {
    match high_water {
        Some(hw) => ClockGuard {
            effective_now: system_now.max(hw),
            persist_high_water: system_now > hw + high_water_throttle(),
        },
        None => ClockGuard {
            effective_now: system_now,
            persist_high_water: true,
        },
    }
}

/// Whole trial days remaining, ceil semantics, clamped to `[0, TRIAL_DAYS]`:
/// day 0 ⇒ 14 left; after 14 full days ⇒ 0 (expired). A (pathological) anchor
/// in the future counts as elapsed 0.
pub fn days_left(anchor: DateTime<Utc>, effective_now: DateTime<Utc>) -> i64 {
    let elapsed_days = (effective_now - anchor).num_days().max(0);
    (TRIAL_DAYS - elapsed_days).clamp(0, TRIAL_DAYS)
}

// ===== keychain accessors (`licensing:trial-anchor`) =====
//
// Best-effort by design: the anchor is not a secret and a locked keychain must
// never break the offline trial — the blob copy carries it alone then. Errors
// are logged (values never are — module discipline) and swallowed.

/// Read + parse the keychain trial anchor. `None` on missing, unparseable, or
/// store failure (logged).
pub fn read_keychain_anchor() -> Option<DateTime<Utc>> {
    match crate::secrets::licensing::get(crate::secrets::licensing::TRIAL_ANCHOR_ENTRY) {
        Ok(Some(raw)) => {
            let parsed = parse_ts(&raw);
            if parsed.is_none() {
                tracing::warn!(
                    "licensing: keychain trial anchor is malformed; treating as missing"
                );
            }
            parsed
        }
        Ok(None) => None,
        Err(e) => {
            tracing::warn!(error = %format!("{e:#}"), "licensing: could not read keychain trial anchor");
            None
        }
    }
}

/// Write the keychain trial anchor (best-effort; failure logged, not fatal).
pub fn write_keychain_anchor(anchor: DateTime<Utc>) {
    if let Err(e) = crate::secrets::licensing::set(
        crate::secrets::licensing::TRIAL_ANCHOR_ENTRY,
        &anchor.to_rfc3339(),
    ) {
        tracing::warn!(error = %format!("{e:#}"), "licensing: could not write keychain trial anchor");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ts(raw: &str) -> DateTime<Utc> {
        parse_ts(raw).expect("test timestamp must parse")
    }

    #[test]
    fn parse_ts_accepts_rfc3339_and_rejects_junk() {
        assert!(parse_ts("2026-07-01T10:00:00+00:00").is_some());
        assert!(parse_ts("  2026-07-01T10:00:00Z  ").is_some());
        assert!(parse_ts("not a date").is_none());
        assert!(parse_ts("").is_none());
    }

    #[test]
    fn anchor_earliest_wins_and_later_side_heals() {
        let early = ts("2026-07-01T00:00:00Z");
        let late = ts("2026-07-05T00:00:00Z");
        let now = ts("2026-07-08T00:00:00Z");

        // Keychain earliest ⇒ blob heals down to it.
        let r = resolve_anchor(Some(early), Some(late), now);
        assert_eq!(r.anchor, early);
        assert!(!r.write_keychain);
        assert!(r.write_blob);

        // Blob earliest ⇒ keychain heals down to it.
        let r = resolve_anchor(Some(late), Some(early), now);
        assert_eq!(r.anchor, early);
        assert!(r.write_keychain);
        assert!(!r.write_blob);

        // Agreement ⇒ nothing to write.
        let r = resolve_anchor(Some(early), Some(early), now);
        assert_eq!(r.anchor, early);
        assert!(!r.write_keychain && !r.write_blob);
    }

    #[test]
    fn anchor_missing_side_is_healed_from_the_other() {
        let early = ts("2026-07-01T00:00:00Z");
        let now = ts("2026-07-08T00:00:00Z");

        let r = resolve_anchor(Some(early), None, now);
        assert_eq!(r.anchor, early);
        assert!(!r.write_keychain);
        assert!(r.write_blob, "missing blob copy must be healed");

        let r = resolve_anchor(None, Some(early), now);
        assert_eq!(r.anchor, early);
        assert!(r.write_keychain, "missing keychain copy must be healed");
        assert!(!r.write_blob);
    }

    #[test]
    fn anchor_first_launch_mints_now_into_both_stores() {
        let now = ts("2026-07-08T09:30:00Z");
        let r = resolve_anchor(None, None, now);
        assert_eq!(r.anchor, now);
        assert!(r.write_keychain && r.write_blob);
    }

    #[test]
    fn clock_guard_never_runs_backwards() {
        let hw = ts("2026-07-10T00:00:00Z");
        // System clock rolled back 5 days ⇒ effective time holds at high-water.
        let rolled_back = ts("2026-07-05T00:00:00Z");
        let g = clock_guard(Some(hw), rolled_back);
        assert_eq!(g.effective_now, hw);
        assert!(
            !g.persist_high_water,
            "rollback must not rewrite high-water"
        );

        // Normal forward progress within the throttle window ⇒ no write.
        let plus_30m = ts("2026-07-10T00:30:00Z");
        let g = clock_guard(Some(hw), plus_30m);
        assert_eq!(g.effective_now, plus_30m);
        assert!(!g.persist_high_water);

        // Past the throttle window ⇒ persist the new high-water.
        let plus_2h = ts("2026-07-10T02:00:00Z");
        let g = clock_guard(Some(hw), plus_2h);
        assert_eq!(g.effective_now, plus_2h);
        assert!(g.persist_high_water);

        // No high-water yet ⇒ always persist.
        let g = clock_guard(None, plus_2h);
        assert_eq!(g.effective_now, plus_2h);
        assert!(g.persist_high_water);
    }

    #[test]
    fn days_left_uses_ceil_semantics() {
        let anchor = ts("2026-07-01T12:00:00Z");
        // Day 0 (a minute in) ⇒ 14 left.
        assert_eq!(days_left(anchor, ts("2026-07-01T12:01:00Z")), 14);
        // 23h59m in — still day 0.
        assert_eq!(days_left(anchor, ts("2026-07-02T11:59:00Z")), 14);
        // 1 full day ⇒ 13 left.
        assert_eq!(days_left(anchor, ts("2026-07-02T12:00:00Z")), 13);
        // 13 days + 23h ⇒ still 1 left.
        assert_eq!(days_left(anchor, ts("2026-07-15T11:00:00Z")), 1);
        // Exactly 14 full days ⇒ 0 (expired).
        assert_eq!(days_left(anchor, ts("2026-07-15T12:00:00Z")), 0);
        // Long past ⇒ clamped at 0.
        assert_eq!(days_left(anchor, ts("2027-01-01T00:00:00Z")), 0);
        // Pathological future anchor ⇒ clamped at 14, never more.
        assert_eq!(days_left(ts("2030-01-01T00:00:00Z"), anchor), 14);
    }

    #[test]
    fn keychain_anchor_round_trips_and_tolerates_junk() {
        let _guard = super::super::store::test_support::keychain_guard_blocking();

        assert_eq!(read_keychain_anchor(), None, "starts empty");
        let anchor = ts("2026-07-01T00:00:00Z");
        write_keychain_anchor(anchor);
        assert_eq!(read_keychain_anchor(), Some(anchor));

        // Malformed stored value reads as missing (heal path), not a panic.
        crate::secrets::licensing::set(
            crate::secrets::licensing::TRIAL_ANCHOR_ENTRY,
            "definitely-not-a-timestamp",
        )
        .expect("test store write");
        assert_eq!(read_keychain_anchor(), None);
    }
}
