//! On-device AI agents — DORMANT until EPIC F (post-C8).
//!
//! This is the client seam for the "library of on-device AI agents" the product
//! promises (About screen "Coming soon"). It is **inert**: nothing here is
//! registered as a Tauri command, nothing on the capture → transcript → summary →
//! store path calls it, and there is no UI. Agents are OFF by default
//! ([`engine::AgentsConfig`] `enabled: false`) and there is **no network
//! dependency** — with agents off the app is byte-identical to a build without
//! this module (the local-first invariant, `CLAUDE.md` §0.1). See ADR-0013 for the
//! design and the HITL rule; see BACKLOG EPIC F for the sequenced tasks (F0–F5).
//!
//! ## The two invariants this seam encodes structurally
//!
//! 1. **Human-in-the-loop (`CLAUDE.md` §0.5):** an agent run yields an
//!    [`draft::AgentDraft`] whose [`draft::DraftStatus`] is always `Draft`.
//!    [`engine::AgentRunner::run`] forces it back to `Draft` even if an agent tries
//!    to self-approve — approval is a separate, explicit human step.
//! 2. **No autonomous external actions (`CLAUDE.md` §10):** an agent can only
//!    *propose* work as a [`draft::ProposedAction`] (data). There is deliberately
//!    **no method anywhere in this seam that sends an email, creates a task, or
//!    otherwise acts on the outside world** — "send" stays a manual user step.
//!
//! Every output is source-linked ([`draft::SourceRef`] → transcript chunk) and
//! tenant-scoped (`ctx: &AuthContext`), so a Phase-2 `agent_runs` table + sync are
//! additive (`CLAUDE.md` §3, §6).
//!
//! Layout (mirrors `sync/`): [`draft`] = the pure data types (serde, tested);
//! [`engine`] = the [`engine::Agent`] trait, the dormant [`engine::AgentRunner`],
//! the typed [`engine::AgentError`], and the placeholder [`engine::NoopAgent`].

pub mod draft;
pub mod engine;

// Stable surface of the seam so future call sites can `use crate::agents::{...}`.
pub use draft::{AgentDraft, AgentRun, DraftStatus, ProposedAction, SourceRef};
pub use engine::{Agent, AgentError, AgentInput, AgentRunner, AgentsConfig, NoopAgent};

use serde::{Deserialize, Serialize};

/// Which agent produced (or should produce) a draft. Only the two EPIC F v1 agents
/// exist; more are added as new variants (each still draft-only, HITL).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentKind {
    /// F2 — drafts a follow-up message from an approved meeting summary. Draft
    /// only; sending is a manual user action.
    FollowUpDrafter,
    /// F3 — aggregates open action items across meetings for review. Never
    /// auto-notifies.
    ActionTracker,
}

impl AgentKind {
    /// Stable wire/log token, identical to the serialized form.
    pub fn wire_name(self) -> &'static str {
        match self {
            AgentKind::FollowUpDrafter => "follow_up_drafter",
            AgentKind::ActionTracker => "action_tracker",
        }
    }
}
