//! Optional sync/collaboration **client** — DORMANT until Phase 2.
//!
//! This module is the client seam for the future optional sync server (`server/`,
//! ADR-0003 Rust/Axum). It is **inert**: nothing here is registered as a Tauri
//! command, nothing on the capture → transcript → summary → store path calls it,
//! and there is no UI toggle. Sync is off by default ([`client::SyncConfig`]
//! `enabled: false`) and there is **no network dependency** — no HTTP/WebSocket
//! client is pulled in. With sync off the app is byte-identical to a build
//! without this module (the local-first invariant, `CLAUDE.md` §0.1). See
//! ADR-0012 for the design and the apply-remote rule.
//!
//! Layout (`docs/SCAFFOLD.md`: `sync/ = mod.rs protocol.rs client.rs`):
//! - [`protocol`] — the `docs/CONTRACTS.md` §5 wire types (`PushItem`,
//!   `ServerAck`, `SyncEntity`) with serde, pinned by round-trip tests.
//! - [`client`] — the [`client::SyncClient`] skeleton, the [`client::Transport`]
//!   abstraction (Phase-1 impl is the no-op [`client::NoopTransport`]), the typed
//!   [`client::SyncError`], and the [`client::RemoteApply`] seam that inbound
//!   remote changes MUST use instead of the Phase-1 repositories (ADR-0012).
//!
//! ## Data classification (what may sync)
//!
//! Only the four synced domain entities exist as a [`protocol::SyncEntity`]
//! variant (meeting / transcript / summary / action_item — `Synced? = yes` in
//! `docs/DATA_MODEL.md`). `settings` / `transcript_settings` are local-only and
//! `provider_credential` secrets NEVER sync (only a non-secret label may, a future
//! decision) — enforced structurally by the absence of a matching variant
//! (`docs/MULTITENANCY.md` "Data classification & sync scope").

pub mod client;
pub mod protocol;

// Convenience re-exports so future call sites can `use crate::sync::{...}`
// without reaching into submodules. These are the stable surface of the seam.
pub use client::{RemoteApply, SyncClient, SyncConfig, SyncError, Transport};
pub use protocol::{PushItem, ServerAck, SyncEntity};
