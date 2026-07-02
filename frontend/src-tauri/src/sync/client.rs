//! Sync client skeleton — DORMANT until Phase 2 (`docs/CONTRACTS.md` §5).
//!
//! This is the client half of the optional sync/collaboration seam. It is
//! **disabled by default and reachable from nothing**: no Tauri command is
//! registered, no code on the capture → transcript → summary → store path calls
//! it, and there is no UI toggle. With sync off the app behaves byte-identically
//! to a build without this module (the local-first invariant, `CLAUDE.md` §0.1).
//!
//! ## No network dependency
//!
//! There is intentionally **no HTTP/WebSocket client here** (no `reqwest`,
//! `tokio-tungstenite`, etc.). Transport is abstracted behind the [`Transport`]
//! trait; the only implementation is [`NoopTransport`], which performs no I/O.
//! The Phase-2 HTTPS transport (talking to `server/`, ADR-0003 Rust/Axum) slots
//! in as a new `Transport` impl behind a feature flag — this file does not change.
//!
//! ## Disabled semantics
//!
//! While [`SyncConfig::enabled`] is `false` (the default), every network-facing
//! method ([`SyncClient::push`] / [`SyncClient::pull`]) short-circuits with
//! [`SyncError::Disabled`] **before touching the transport or any I/O**. This is
//! asserted by the tests: a disabled client never calls the transport.
//!
//! ## CRITICAL design seam: applying a remote change must bypass the Phase-1 repos
//!
//! Guardian-flagged, recorded in ADR-0012. The Phase-1 tenant-scoped repositories
//! (`database/repositories/`, ADR-0010) bump `rev = rev + 1` and stamp
//! `updated_by = ctx.user_id` on **every** write. That is correct for a *local*
//! edit, but replaying an inbound *remote* change through them would:
//!   1. masquerade the remote change as a fresh local edit (`updated_by` becomes
//!      the local user instead of the true remote author),
//!   2. re-bump `rev`, destroying the `rev` the server assigned and the
//!      `rev = 1 / updated_by = NULL` never-synced baseline
//!      (`docs/DATA_MODEL.md`), and
//!   3. therefore make the row look "modified since last sync" again — a change
//!      the next push re-sends, which the server re-acks, forever (a sync
//!      ping-pong / infinite echo).
//!
//! So inbound application is a **distinct seam**, [`RemoteApply`], defined (not
//! wired) here. Its contract: write `rev`, `updated_by`, `updated_at` and
//! `deleted_at` **VERBATIM** from the [`PushItem`] payload — never re-derive them
//! from an `AuthContext`. Its future implementation is a separate "remote-apply"
//! repository path (`INSERT ... ON CONFLICT DO UPDATE SET rev = :remote_rev,
//! updated_by = :remote_updated_by, ...`), NOT any method on the existing
//! [`crate::database::repositories`] types. Settings/config tables stay entirely
//! out of sync scope (`provider_credential` secrets never sync — see
//! [`super::protocol`] and `docs/MULTITENANCY.md`).

use crate::context::AuthContext;
use crate::sync::protocol::{PushItem, ServerAck};
use thiserror::Error;

/// Dormant-sync configuration. `enabled` defaults to `false`, so a
/// default-constructed [`SyncClient`] is inert (local-first: sync is OFF unless
/// something in Phase 2 explicitly turns it on).
///
/// `Default` is **derived**: `bool::default()` is `false`, which is exactly the
/// off state we require, so the derive is the single source of that default (the
/// [`tests::config_default_is_disabled`] test pins it, guarding against a future
/// field defaulting "on"). Phase 2 will grow this (server URL, tenant scope,
/// credentials-by-reference, etc.); today it carries only the on/off switch so
/// the seam is typed and the "off" path is testable. It is deliberately **not**
/// surfaced in the UI or any config file yet.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct SyncConfig {
    /// Master switch. `false` (the derived default) ⇒ every network-facing method
    /// returns [`SyncError::Disabled`] and does no I/O.
    pub enabled: bool,
}

/// Typed sync errors (`docs/CONVENTIONS.md`: typed domain errors via `thiserror`).
///
/// In this dormant phase only [`SyncError::Disabled`] can actually occur; the
/// other variants are the vocabulary the Phase-2 transport will use, defined now
/// so the error surface is stable and matching on it is exhaustive from day one.
#[derive(Debug, Error)]
pub enum SyncError {
    /// Sync is turned off ([`SyncConfig::enabled`] is `false`). Returned by every
    /// network-facing method in Phase 1. Not an error condition the user needs to
    /// see — callers (Phase 2) treat it as "feature not active".
    #[error("sync is disabled")]
    Disabled,

    /// No transport is capable of I/O (e.g. the [`NoopTransport`] placeholder, or
    /// a Phase-2 transport that has not been configured). Distinct from
    /// [`SyncError::Disabled`]: sync may be enabled but wired to a no-op.
    #[error("no sync transport configured")]
    NoTransport,

    /// Phase-2 network/transport failure. Local-first: the caller must degrade
    /// gracefully (surface a clear message, keep working on local data — never
    /// crash, never lose data). Carries a human-readable cause.
    #[error("sync transport error: {0}")]
    Transport(String),

    /// Phase-2 payload (de)serialization failure on the wire.
    #[error("sync protocol error: {0}")]
    Protocol(#[from] serde_json::Error),
}

/// The wire transport abstraction. DORMANT: no networking implementation exists
/// in Phase 1.
///
/// A Phase-2 HTTPS/WebSocket client (talking to `server/`) is added as a new
/// implementor behind a feature flag, without changing [`SyncClient`]. Methods
/// are deliberately non-`async` in this skeleton — no runtime/executor is pulled
/// in for the dormant seam; the Phase-2 trait can be widened to `async` (or use
/// an async-trait) when a real transport lands.
pub trait Transport: Send + Sync {
    /// True iff this transport can actually perform network I/O. The only Phase-1
    /// implementation ([`NoopTransport`]) returns `false`.
    fn is_operational(&self) -> bool;

    /// Send a batch of push items and return one [`ServerAck`] per item.
    ///
    /// Phase 1: unreachable in normal operation (a disabled [`SyncClient`] returns
    /// [`SyncError::Disabled`] before ever calling this). The [`NoopTransport`]
    /// impl returns [`SyncError::NoTransport`] rather than fabricating acks.
    fn send_push(&self, tenant_id: &str, items: &[PushItem]) -> Result<Vec<ServerAck>, SyncError>;

    /// Fetch changes for `tenant_id` newer than `since_rev`.
    ///
    /// Phase 1: same as [`Transport::send_push`] — the [`NoopTransport`] returns
    /// [`SyncError::NoTransport`].
    fn fetch_pull(&self, tenant_id: &str, since_rev: u64) -> Result<Vec<PushItem>, SyncError>;
}

/// The placeholder transport: performs **no I/O** and is the only [`Transport`] in
/// Phase 1. Every operation reports "not operational" / [`SyncError::NoTransport`]
/// so that even if a future caller enables sync without wiring a real transport,
/// nothing silently talks to a network and no acks are invented.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoopTransport;

impl Transport for NoopTransport {
    fn is_operational(&self) -> bool {
        false
    }

    fn send_push(
        &self,
        _tenant_id: &str,
        _items: &[PushItem],
    ) -> Result<Vec<ServerAck>, SyncError> {
        // No network in Phase 1. A real HTTPS transport replaces this in Phase 2.
        Err(SyncError::NoTransport)
    }

    fn fetch_pull(&self, _tenant_id: &str, _since_rev: u64) -> Result<Vec<PushItem>, SyncError> {
        Err(SyncError::NoTransport)
    }
}

/// The dormant sync client. Owns a [`SyncConfig`] and a [`Transport`]; while
/// disabled it is inert.
///
/// Generic over the transport so the Phase-2 HTTPS transport can be injected
/// without a trait object if desired; defaults to [`NoopTransport`].
pub struct SyncClient<T: Transport = NoopTransport> {
    config: SyncConfig,
    transport: T,
}

impl SyncClient<NoopTransport> {
    /// Construct a dormant client with the no-op transport. With the default
    /// [`SyncConfig`] (`enabled: false`) this client does nothing.
    pub fn new(config: SyncConfig) -> Self {
        Self {
            config,
            transport: NoopTransport,
        }
    }
}

impl<T: Transport> SyncClient<T> {
    /// Construct with an explicit transport (Phase 2 injects a real one here).
    pub fn with_transport(config: SyncConfig, transport: T) -> Self {
        Self { config, transport }
    }

    /// Whether sync is currently enabled (mirrors [`SyncConfig::enabled`]).
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Push local changes to the server.
    ///
    /// Phase 1: always returns [`SyncError::Disabled`] (sync is off) and performs
    /// **no I/O** — the transport is not touched. `ctx` scopes the push to
    /// `ctx.tenant_id` (Phase 2 will forward `ctx.tenant_id`/`ctx.user_id` and the
    /// per-item `tenant_id` must match).
    pub fn push(&self, ctx: &AuthContext, items: &[PushItem]) -> Result<Vec<ServerAck>, SyncError> {
        // Guard FIRST: never touch the transport while disabled.
        if !self.config.enabled {
            return Err(SyncError::Disabled);
        }
        // Phase 2 fills this in (validate per-item tenant_id == ctx.tenant_id,
        // then delegate to the transport). Today, if somehow enabled, the only
        // transport is the no-op, which reports NoTransport.
        self.transport.send_push(ctx.tenant_id.as_str(), items)
    }

    /// Pull remote changes newer than `since_rev` for the caller's tenant.
    ///
    /// Phase 1: always returns [`SyncError::Disabled`] and performs **no I/O**.
    ///
    /// NOTE: the returned [`PushItem`]s must be applied via the [`RemoteApply`]
    /// seam (verbatim `rev`/`updated_by`/`deleted_at`), **never** by feeding them
    /// through the Phase-1 repositories — see the module docs and ADR-0012.
    pub fn pull(&self, ctx: &AuthContext, since_rev: u64) -> Result<Vec<PushItem>, SyncError> {
        if !self.config.enabled {
            return Err(SyncError::Disabled);
        }
        self.transport.fetch_pull(ctx.tenant_id.as_str(), since_rev)
    }
}

/// Applies an inbound **remote** change to local storage — the seam that MUST NOT
/// be the Phase-1 repositories. DORMANT: defined, not wired, and has no
/// implementation in Phase 1.
///
/// ## Why this is separate (do not "reuse the repository")
///
/// The Phase-1 repositories (ADR-0010) bump `rev` and set
/// `updated_by = ctx.user_id` on every write. A remote change already carries its
/// own authoritative `rev`, `updated_by`, `updated_at` and `deleted_at`
/// (soft-delete tombstone). An implementor of this trait writes those fields
/// **verbatim** from [`PushItem`] and must **not**:
///   - increment `rev` (use the value on the wire),
///   - overwrite `updated_by` with the local user (use the payload's, which may be
///     `None` for the never-synced baseline), or
///   - hard-delete on a tombstone (set `deleted_at`, keep the row).
///
/// Doing otherwise recreates the ping-pong described in the module docs. The
/// concrete Phase-2 implementation is a dedicated remote-apply repository path
/// (its own SQL: `... ON CONFLICT DO UPDATE SET rev = :rev, updated_by =
/// :updated_by, updated_at = :updated_at, deleted_at = :deleted_at`), distinct
/// from every method on [`crate::database::repositories`].
///
/// `ctx` is passed for **tenant scoping only** (write within `ctx.tenant_id`);
/// the identity fields on the row come from the payload, never from `ctx`.
/// Settings / `provider_credential` are out of scope by construction — there is
/// no [`super::protocol::SyncEntity`] variant for them.
pub trait RemoteApply: Send + Sync {
    /// Apply one remote item, writing its sync fields verbatim. Phase 2 provides
    /// the implementation; Phase 1 has none (the trait exists to pin the contract
    /// and keep it off the repository path).
    fn apply_remote(&self, ctx: &AuthContext, item: &PushItem) -> Result<(), SyncError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::AuthContext;
    use crate::sync::protocol::SyncEntity;
    use serde_json::json;
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::sync::Mutex;

    fn sample_items() -> Vec<PushItem> {
        vec![PushItem {
            tenant_id: "local".to_string(),
            entity: SyncEntity::Meeting,
            id: "meeting-1".to_string(),
            rev: 1,
            updated_by: None,
            updated_at: "2026-07-02T10:00:00.000+00:00".to_string(),
            deleted: false,
            payload: json!({ "title": "x" }),
        }]
    }

    /// The default config is OFF — the load-bearing invariant for this whole
    /// module (`CLAUDE.md` §0.1 local-first).
    #[test]
    fn config_default_is_disabled() {
        assert!(!SyncConfig::default().enabled);
        assert!(!SyncClient::new(SyncConfig::default()).is_enabled());
    }

    /// A `Transport` that records whether it was ever called, so we can prove a
    /// disabled client performs NO I/O (never reaches the transport). Uses an
    /// atomic flag so the type is naturally `Send + Sync` (no `unsafe`).
    #[derive(Default)]
    struct SpyTransport {
        touched: AtomicBool,
    }
    impl SpyTransport {
        fn was_touched(&self) -> bool {
            self.touched.load(Ordering::SeqCst)
        }
    }
    impl Transport for SpyTransport {
        fn is_operational(&self) -> bool {
            self.touched.store(true, Ordering::SeqCst);
            true
        }
        fn send_push(
            &self,
            _tenant_id: &str,
            _items: &[PushItem],
        ) -> Result<Vec<ServerAck>, SyncError> {
            self.touched.store(true, Ordering::SeqCst);
            Ok(Vec::new())
        }
        fn fetch_pull(
            &self,
            _tenant_id: &str,
            _since_rev: u64,
        ) -> Result<Vec<PushItem>, SyncError> {
            self.touched.store(true, Ordering::SeqCst);
            Ok(Vec::new())
        }
    }

    /// Disabled `push` returns `Disabled` AND never touches the transport (no I/O).
    #[test]
    fn disabled_push_returns_disabled_and_does_no_io() {
        let client = SyncClient::with_transport(SyncConfig::default(), SpyTransport::default());
        let ctx = AuthContext::local();

        let err = client.push(&ctx, &sample_items()).unwrap_err();
        assert!(matches!(err, SyncError::Disabled), "got {err:?}");
        // The spy must be pristine: the guard short-circuited before the transport.
        assert!(
            !client.transport.was_touched(),
            "disabled push must not touch the transport (no I/O)"
        );
    }

    /// Disabled `pull` returns `Disabled` AND never touches the transport.
    #[test]
    fn disabled_pull_returns_disabled_and_does_no_io() {
        let client = SyncClient::with_transport(SyncConfig::default(), SpyTransport::default());
        let ctx = AuthContext::local();

        let err = client.pull(&ctx, 0).unwrap_err();
        assert!(matches!(err, SyncError::Disabled), "got {err:?}");
        assert!(
            !client.transport.was_touched(),
            "disabled pull must not touch the transport (no I/O)"
        );
    }

    /// The no-op transport reports itself non-operational and refuses I/O with
    /// `NoTransport` (it never fabricates acks).
    #[test]
    fn noop_transport_is_not_operational() {
        let t = NoopTransport;
        assert!(!t.is_operational());
        assert!(matches!(
            t.send_push("local", &sample_items()),
            Err(SyncError::NoTransport)
        ));
        assert!(matches!(
            t.fetch_pull("local", 0),
            Err(SyncError::NoTransport)
        ));
    }

    /// Even if a client is (hypothetically) enabled, with the Phase-1 no-op
    /// transport it degrades to `NoTransport` — it does not crash and does not
    /// invent a server response (local-first graceful degradation).
    #[test]
    fn enabled_with_noop_transport_degrades_to_no_transport() {
        let client = SyncClient::new(SyncConfig { enabled: true });
        let ctx = AuthContext::local();
        assert!(matches!(
            client.push(&ctx, &sample_items()),
            Err(SyncError::NoTransport)
        ));
        assert!(matches!(client.pull(&ctx, 0), Err(SyncError::NoTransport)));
    }

    /// Compile-time proof that the remote-apply seam is a DISTINCT type from the
    /// repositories: a `RemoteApply` implementor that writes fields VERBATIM,
    /// with no `AuthContext`-derived `rev`/`updated_by`. If a future refactor
    /// tried to satisfy `RemoteApply` by delegating to a normal repository write
    /// (which bumps rev / stamps updated_by), this test's recorded values would
    /// not match the payload — encoding the ADR-0012 rule as an executable check.
    #[test]
    fn apply_remote_writes_fields_verbatim_not_via_repo() {
        // A stand-in "remote-apply store" that records exactly what it was told to
        // persist — standing in for the future remote-apply repository path.
        // Atomics/Mutex keep it naturally `Send + Sync` (no `unsafe`).
        #[derive(Default)]
        struct RecordingRemoteStore {
            last_rev: AtomicU64,
            last_updated_by: Mutex<Option<String>>,
            last_deleted: AtomicBool,
        }
        impl RemoteApply for RecordingRemoteStore {
            fn apply_remote(&self, _ctx: &AuthContext, item: &PushItem) -> Result<(), SyncError> {
                // VERBATIM: copy the wire fields, do NOT bump rev, do NOT substitute
                // ctx.user_id for updated_by.
                self.last_rev.store(item.rev, Ordering::SeqCst);
                *self.last_updated_by.lock().unwrap() = item.updated_by.clone();
                self.last_deleted.store(item.deleted, Ordering::SeqCst);
                Ok(())
            }
        }

        let store = RecordingRemoteStore::default();
        let ctx = AuthContext::local(); // ctx.user_id == "local-user"

        // A remote change authored by SOMEONE ELSE at rev 7.
        let remote = PushItem {
            tenant_id: "local".to_string(),
            entity: SyncEntity::Summary,
            id: "summary-1".to_string(),
            rev: 7,
            updated_by: Some("remote-user".to_string()),
            updated_at: "2026-07-02T12:00:00.000+00:00".to_string(),
            deleted: true,
            payload: json!({ "status": "approved" }),
        };
        store.apply_remote(&ctx, &remote).expect("apply_remote");

        // The stored rev/updated_by are the REMOTE values, not rev+1 / local-user.
        assert_eq!(
            store.last_rev.load(Ordering::SeqCst),
            7,
            "rev must be written verbatim, not bumped"
        );
        assert_eq!(
            store.last_updated_by.lock().unwrap().clone(),
            Some("remote-user".to_string()),
            "updated_by must be the remote author, never ctx.user_id"
        );
        assert_ne!(
            ctx.user_id.as_str(),
            "remote-user",
            "sanity: the local user is distinct from the remote author"
        );
        assert!(
            store.last_deleted.load(Ordering::SeqCst),
            "soft-delete tombstone preserved"
        );
    }
}
