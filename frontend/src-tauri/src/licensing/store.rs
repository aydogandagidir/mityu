//! Persistence for the per-workspace, non-secret `licensingState` JSON blob
//! (ADR-0023 §4/§6).
//!
//! Mirrors the ADR-0015 `redactionConfig` pattern
//! (`SettingsRepository::{get,set}_redaction_config`): a small JSON blob, read
//! back as the [`Default`] when absent or unparseable (a corrupt blob can never
//! *extend* a trial or *grant* a license — the trial anchor then heals from the
//! keychain copy), written via a workspace-guarded upsert.
//!
//! ## Where the blob lives (no schema migration)
//! ADR-0023 mandates **no schema migration**, and the `settings` table has no
//! spare TEXT column, so the blob row lives in the pre-existing, otherwise-dead
//! upstream `licensing` table (created by migration `20251105120000`, zero code
//! references since the upstream RSA licensing was removed). One row per
//! workspace, keyed `license_key = 'licensingState:<workspace_id>'` with
//! `signature_hash = 'licensingState:v1'` as the versioned marker (the
//! `secrets::KEYCHAIN_MARKER` style); the JSON goes in the `encrypted_key` TEXT
//! column and the legacy NOT NULL columns are filled with inert values
//! (`activation_date`/`generated_on` double as created/updated timestamps).
//! The column names are historical — nothing here is a secret (constitution §7:
//! the real license key + activation id are keychain-only, see
//! `secrets::licensing`), and the DB file itself is SQLCipher-encrypted at rest
//! (ADR-0014). A Phase-2 schema pass may relocate this into a proper
//! `settings.licensingState` column; the marker makes those rows findable.

use crate::context::AuthContext;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

/// Versioned marker stored in `signature_hash` identifying a row as a
/// `licensingState` blob (vs. a legacy upstream RSA row). Frozen.
pub const BLOB_MARKER: &str = "licensingState:v1";

/// Row key for a workspace's blob: `licensingState:<workspace_id>`.
fn row_key(ctx: &AuthContext) -> String {
    format!("licensingState:{}", ctx.tenant_id)
}

/// The non-secret licensing state persisted per workspace. All fields optional
/// with [`Default`] = empty, so old/partial blobs keep parsing (`serde(default)`).
///
/// NEVER put the raw license key or the activation id here (constitution §7 —
/// keychain only). `display_key` is the masked form built for the UI.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct LicensingBlob {
    /// First-launch trial anchor (RFC 3339). Second copy of the keychain
    /// `licensing:trial-anchor` entry — earliest wins, each heals the other.
    pub trial_anchor: Option<String>,
    /// Clock high-water mark (RFC 3339): the latest wall-clock time this app has
    /// observed. `effective_now = max(system_now, last_seen_at)` defeats
    /// set-the-clock-back trial extension. Writes are throttled to ~1/hour.
    pub last_seen_at: Option<String>,
    /// Masked license key for display (e.g. `MITYU-****-****-1234`).
    pub display_key: Option<String>,
    /// Plan label (e.g. `pro`).
    pub plan: Option<String>,
    /// License expiry (RFC 3339) as last reported by Polar; `None` = perpetual.
    pub expires_at: Option<String>,
    /// When the license was last validated against Polar — actually the last
    /// validation *attempt* (success or transport failure), which is what the
    /// ≤ once-per-7-days phone-home throttle rate-limits (ADR-0023 §7).
    pub last_validated_at: Option<String>,
    /// Human sentence set when Polar explicitly reported the key
    /// revoked/disabled. Cleared when a later validation grants again.
    pub revoked_reason: Option<String>,
}

/// Read the workspace's blob. Missing row/NULL/corrupt JSON ⇒ [`Default`] (the
/// fail-safe direction — see module docs); a real DB error is surfaced so the
/// caller can decide how to degrade.
pub async fn get_blob(pool: &SqlitePool, ctx: &AuthContext) -> Result<LicensingBlob, sqlx::Error> {
    let json: Option<String> = sqlx::query_scalar(
        "SELECT encrypted_key FROM licensing WHERE license_key = ? AND signature_hash = ? LIMIT 1",
    )
    .bind(row_key(ctx))
    .bind(BLOB_MARKER)
    .fetch_optional(pool)
    .await?;

    match json {
        Some(raw) if !raw.trim().is_empty() => match serde_json::from_str(&raw) {
            Ok(blob) => Ok(blob),
            Err(e) => {
                // Never log the blob itself (it carries no secret, but stay
                // uniform with the redactionConfig pattern).
                tracing::warn!(
                    workspace_id = %ctx.tenant_id,
                    error = %e,
                    "licensing: licensingState blob failed to parse; falling back to default"
                );
                Ok(LicensingBlob::default())
            }
        },
        _ => Ok(LicensingBlob::default()),
    }
}

/// Upsert the workspace's blob. The `WHERE license_key = excluded.license_key`
/// guard is implicit in the PK (the key embeds the workspace id, so a foreign
/// workspace can never clobber another's row). Stamps `generated_on` as the
/// updated-at timestamp.
pub async fn set_blob(
    pool: &SqlitePool,
    ctx: &AuthContext,
    blob: &LicensingBlob,
) -> Result<(), sqlx::Error> {
    let json = serde_json::to_string(blob).map_err(|e| {
        sqlx::Error::Protocol(format!("Failed to serialize LicensingBlob to JSON: {}", e))
    })?;
    let now = Utc::now().to_rfc3339();

    sqlx::query(
        r#"
        INSERT INTO licensing (
            license_key, encrypted_key, signature_hash, activation_date,
            expiry_date, soft_expiry_date, max_activation_time, duration,
            generated_on, is_soft_expired
        )
        VALUES (?, ?, ?, ?, '', '', '', 0, ?, 0)
        ON CONFLICT(license_key) DO UPDATE SET
            encrypted_key = excluded.encrypted_key,
            signature_hash = excluded.signature_hash,
            generated_on = excluded.generated_on
        "#,
    )
    .bind(row_key(ctx))
    .bind(json)
    .bind(BLOB_MARKER)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;

    Ok(())
}

#[cfg(test)]
pub(crate) mod test_support {
    //! Shared pool/keychain scaffolding for the licensing unit tests.

    use sqlx::migrate::Migrator;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use sqlx::SqlitePool;

    /// The app's real, compile-time-embedded migration set (same source as
    /// `DatabaseManager::new`), so the `licensing` table exists exactly as it
    /// does in production DBs.
    static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

    /// Open a fresh migrated temp-file pool (the repository test pattern).
    /// Returns the tempdir too — dropping it deletes the DB file.
    pub async fn open_migrated_pool() -> (SqlitePool, tempfile::TempDir) {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("licensing-test.sqlite");
        let options = SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .expect("open temp sqlite db");
        MIGRATOR.run(&pool).await.expect("migrations must apply");
        (pool, dir)
    }

    /// Serializes tests that touch the FIXED-NAME licensing keychain entries
    /// (`licensing:*` are device-scoped, so parallel tests in the shared
    /// in-memory store would collide) and wipes those entries first.
    ///
    /// A `tokio` mutex (not `std::sync`) because most callers hold the guard
    /// across `.await` points (`clippy::await_holding_lock`).
    pub async fn keychain_guard() -> tokio::sync::MutexGuard<'static, ()> {
        let guard = lock().lock().await;
        wipe_licensing_entries();
        guard
    }

    /// [`keychain_guard`] for non-async tests (no runtime ⇒ `blocking_lock`).
    pub fn keychain_guard_blocking() -> tokio::sync::MutexGuard<'static, ()> {
        let guard = lock().blocking_lock();
        wipe_licensing_entries();
        guard
    }

    fn lock() -> &'static tokio::sync::Mutex<()> {
        use std::sync::OnceLock;
        static LOCK: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| tokio::sync::Mutex::new(()))
    }

    fn wipe_licensing_entries() {
        crate::secrets::test_store::install();
        for entry in [
            crate::secrets::licensing::TRIAL_ANCHOR_ENTRY,
            crate::secrets::licensing::KEY_ENTRY,
            crate::secrets::licensing::ACTIVATION_ID_ENTRY,
        ] {
            let _ = crate::secrets::licensing::delete(entry);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::open_migrated_pool;
    use super::*;
    use crate::context::{AuthContext, RequestId, Role, TenantId, UserId};

    fn ctx_for(tenant: &str) -> AuthContext {
        AuthContext {
            tenant_id: TenantId::new(tenant),
            user_id: UserId::new("unit-user"),
            roles: vec![Role::Owner],
            request_id: RequestId::generate(),
        }
    }

    #[tokio::test]
    async fn blob_round_trips_and_defaults_when_absent() {
        let (pool, _dir) = open_migrated_pool().await;
        let ctx = ctx_for("local");

        // Absent row ⇒ default.
        let blob = get_blob(&pool, &ctx).await.expect("get");
        assert_eq!(blob, LicensingBlob::default());

        let written = LicensingBlob {
            trial_anchor: Some("2026-07-01T00:00:00+00:00".into()),
            last_seen_at: Some("2026-07-02T00:00:00+00:00".into()),
            plan: Some("pro".into()),
            ..Default::default()
        };
        set_blob(&pool, &ctx, &written).await.expect("set");
        let read = get_blob(&pool, &ctx).await.expect("get");
        assert_eq!(read, written);

        // Upsert overwrites.
        let updated = LicensingBlob {
            display_key: Some("MITYU-****-****-1234".into()),
            ..written.clone()
        };
        set_blob(&pool, &ctx, &updated).await.expect("set 2");
        assert_eq!(get_blob(&pool, &ctx).await.expect("get 2"), updated);
    }

    #[tokio::test]
    async fn blob_rows_are_workspace_scoped() {
        let (pool, _dir) = open_migrated_pool().await;
        let local = ctx_for("local");
        let other = ctx_for("other-ws");

        let local_blob = LicensingBlob {
            trial_anchor: Some("2026-01-01T00:00:00+00:00".into()),
            ..Default::default()
        };
        set_blob(&pool, &local, &local_blob).await.expect("set");

        // The other workspace sees no blob, and writing its own does not
        // clobber local's.
        assert_eq!(
            get_blob(&pool, &other).await.expect("get other"),
            LicensingBlob::default()
        );
        let other_blob = LicensingBlob {
            trial_anchor: Some("2026-06-01T00:00:00+00:00".into()),
            ..Default::default()
        };
        set_blob(&pool, &other, &other_blob)
            .await
            .expect("set other");
        assert_eq!(
            get_blob(&pool, &local).await.expect("get local"),
            local_blob
        );
        assert_eq!(
            get_blob(&pool, &other).await.expect("get other"),
            other_blob
        );
    }

    #[tokio::test]
    async fn corrupt_blob_falls_back_to_default() {
        let (pool, _dir) = open_migrated_pool().await;
        let ctx = ctx_for("local");
        sqlx::query(
            "INSERT INTO licensing (license_key, encrypted_key, signature_hash, activation_date, \
             expiry_date, soft_expiry_date, max_activation_time, duration, generated_on, is_soft_expired) \
             VALUES ('licensingState:local', '{not json', 'licensingState:v1', '', '', '', '', 0, '', 0)",
        )
        .execute(&pool)
        .await
        .expect("seed corrupt row");

        let blob = get_blob(&pool, &ctx).await.expect("get");
        assert_eq!(
            blob,
            LicensingBlob::default(),
            "corrupt blob must not grant anything"
        );
    }

    #[tokio::test]
    async fn legacy_upstream_rows_are_ignored() {
        let (pool, _dir) = open_migrated_pool().await;
        let ctx = ctx_for("local");
        // A leftover upstream RSA-licensing row (different PK, no marker) must
        // be invisible to the blob reader.
        sqlx::query(
            "INSERT INTO licensing (license_key, encrypted_key, signature_hash, activation_date, \
             expiry_date, soft_expiry_date, max_activation_time, duration, generated_on, is_soft_expired) \
             VALUES ('LIC-UPSTREAM-1', 'AAAA', 'deadbeef', '2025-01-01', '2026-01-01', '2026-02-01', \
             '2026-01-01', 31536000, '2025-01-01', 0)",
        )
        .execute(&pool)
        .await
        .expect("seed legacy row");

        assert_eq!(
            get_blob(&pool, &ctx).await.expect("get"),
            LicensingBlob::default()
        );
    }
}
