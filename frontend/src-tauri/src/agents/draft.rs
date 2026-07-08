//! Agent draft data types — pure, serde-serializable, no I/O. DORMANT (EPIC F).
//!
//! These are the shapes an agent produces and a future `agent_runs` table
//! persists. They are decoupled from any provider or transport — like
//! `sync/protocol.rs`, this file is data only, pinned by round-trip tests. Every
//! output carries its transcript evidence ([`SourceRef`]) and starts life as a
//! [`DraftStatus::Draft`] (HITL, `CLAUDE.md` §0.5).

use super::AgentKind;
use serde::{Deserialize, Serialize};

/// Source-link back to the transcript evidence a draft is grounded in
/// (`CLAUDE.md` §0.5: bind every AI-generated item to its source segment). Mirrors
/// the summary source-link (`source_chunk_id`, BACKLOG C1) so an agent draft is
/// always traceable to what was actually said.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceRef {
    /// The meeting the evidence belongs to.
    pub meeting_id: String,
    /// The transcript chunk id (evidence anchor), same identifier space as C1.
    pub source_chunk_id: String,
    /// Optional verbatim quote copied for display; `None` when only the anchor is
    /// kept.
    pub quote: Option<String>,
}

/// Lifecycle of an agent draft. Defaults to [`DraftStatus::Draft`] — nothing is
/// ever auto-approved (HITL). Approval/rejection is an explicit human action.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DraftStatus {
    /// Just produced by an agent; awaiting human review. The only status a run
    /// may return.
    #[default]
    Draft,
    /// A human approved this draft. Set only by an explicit user action, never by
    /// an agent or the runner.
    Approved,
    /// A human rejected this draft.
    Rejected,
}

/// An action an agent PROPOSES. It is **data only**: this seam has no method that
/// executes it. Sending a message / creating a task is always a separate, explicit
/// human step (`CLAUDE.md` §10: no autonomous irreversible actions).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum ProposedAction {
    /// A drafted follow-up message. The agent only writes text; it never sends.
    DraftMessage {
        /// Suggested recipient(s), if the agent inferred any.
        to: Option<String>,
        /// Suggested subject line.
        subject: Option<String>,
        /// The drafted body the human edits then sends manually.
        body: String,
    },
    /// An open action item surfaced for tracking. Never auto-notifies anyone.
    TrackActionItem {
        /// The action text.
        text: String,
        /// Suggested owner, if inferred.
        assignee: Option<String>,
        /// Suggested due date (free text / ISO-8601), if inferred.
        due: Option<String>,
    },
}

/// An agent's output: proposed actions grounded in sources, pending human review.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentDraft {
    /// Always [`DraftStatus::Draft`] when returned by [`super::engine::AgentRunner::run`].
    pub status: DraftStatus,
    /// What the agent suggests (never executed automatically).
    pub actions: Vec<ProposedAction>,
    /// Transcript evidence backing the suggestions.
    pub sources: Vec<SourceRef>,
}

impl AgentDraft {
    /// A fresh draft (status = [`DraftStatus::Draft`]).
    pub fn new(actions: Vec<ProposedAction>, sources: Vec<SourceRef>) -> Self {
        Self {
            status: DraftStatus::Draft,
            actions,
            sources,
        }
    }

    /// An empty draft (no proposals) — what the placeholder agent returns.
    pub fn empty() -> Self {
        Self {
            status: DraftStatus::Draft,
            actions: Vec::new(),
            sources: Vec::new(),
        }
    }
}

/// Persisted record of an agent run. Maps to a future `agent_runs` table and
/// carries the tenant + sync-ready fields (`CLAUDE.md` §6: `id`, `workspace_id`
/// (=`tenant_id`), `created_at`, `updated_at`, `updated_by`, `rev`, soft-delete)
/// so a Phase-2 migration is additive. DORMANT: nothing writes this yet.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentRun {
    /// UUID primary key.
    pub id: String,
    /// Workspace/tenant scope (local mode: the constant `local`).
    pub tenant_id: String,
    /// Which agent produced the draft.
    pub kind: AgentKind,
    /// The produced draft (with its status + source links).
    pub draft: AgentDraft,
    /// Creation timestamp (ISO-8601 / RFC 3339 text).
    pub created_at: String,
    /// Last-modified timestamp.
    pub updated_at: String,
    /// Last writer, or `None` for the never-synced baseline (`docs/DATA_MODEL.md`).
    pub updated_by: Option<String>,
    /// Monotonic revision (sync baseline).
    pub rev: u64,
    /// Soft-delete tombstone: `None` = live, `Some(ts)` = deleted at that instant.
    ///
    /// `CLAUDE.md` §6 mandates `deleted_at` for every domain table, and that is what
    /// the shipped tables use (`database/repositories/action_item.rs`: `SET deleted_at
    /// = ?` / `WHERE deleted_at IS NULL`). This field used to be a bare `deleted: bool`,
    /// which mirrored the *sync envelope* (`sync/protocol.rs`: `"deleted": false`)
    /// rather than the *row* — a `bool` cannot answer "when", so tombstone retention
    /// and pruning would have had nowhere to read from. Corrected before any
    /// `agent_runs` migration could bake it in (ADR-0022 / ADR-0013 amendment).
    pub deleted_at: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The default status is `Draft` — the load-bearing HITL invariant.
    #[test]
    fn draft_status_defaults_to_draft() {
        assert_eq!(DraftStatus::default(), DraftStatus::Draft);
        assert_eq!(AgentDraft::empty().status, DraftStatus::Draft);
        assert_eq!(
            AgentDraft::new(Vec::new(), Vec::new()).status,
            DraftStatus::Draft
        );
    }

    /// A `ProposedAction` serializes with its `action` tag and round-trips.
    #[test]
    fn proposed_action_tagged_and_round_trips() {
        let a = ProposedAction::DraftMessage {
            to: Some("a@b.co".to_string()),
            subject: None,
            body: "Thanks for the call.".to_string(),
        };
        let v = serde_json::to_value(&a).expect("serialize");
        assert_eq!(v["action"], "draft_message");
        assert_eq!(v["body"], "Thanks for the call.");
        let back: ProposedAction = serde_json::from_value(v).expect("deserialize");
        assert_eq!(back, a);
    }

    /// An `AgentRun` round-trips and stays tenant-scoped + draft-status.
    #[test]
    fn agent_run_round_trips_tenant_scoped_draft() {
        let run = AgentRun {
            id: "run-1".to_string(),
            tenant_id: "local".to_string(),
            kind: AgentKind::ActionTracker,
            draft: AgentDraft::new(
                vec![ProposedAction::TrackActionItem {
                    text: "Ship the deck".to_string(),
                    assignee: Some("aydogan".to_string()),
                    due: None,
                }],
                vec![SourceRef {
                    meeting_id: "m1".to_string(),
                    source_chunk_id: "chunk-9".to_string(),
                    quote: Some("let's ship the deck by Friday".to_string()),
                }],
            ),
            created_at: "2026-07-02T10:00:00Z".to_string(),
            updated_at: "2026-07-02T10:00:00Z".to_string(),
            updated_by: None,
            rev: 1,
            deleted_at: None,
        };
        let text = serde_json::to_string(&run).expect("serialize");
        let back: AgentRun = serde_json::from_str(&text).expect("deserialize");
        assert_eq!(back, run);
        assert_eq!(back.tenant_id, "local");
        assert_eq!(back.draft.status, DraftStatus::Draft);
        assert_eq!(back.draft.sources.len(), 1);
        assert!(back.deleted_at.is_none(), "a fresh run is not a tombstone");
    }

    /// A soft-deleted run records *when* — the thing a bare `deleted: bool` could not.
    #[test]
    fn agent_run_tombstone_carries_a_timestamp() {
        let json = r#"{"id":"run-2","tenant_id":"local","kind":"action_tracker",
            "draft":{"status":"draft","actions":[],"sources":[]},
            "created_at":"2026-07-02T10:00:00Z","updated_at":"2026-07-08T09:00:00Z",
            "updated_by":null,"rev":2,"deleted_at":"2026-07-08T09:00:00Z"}"#;
        let run: AgentRun = serde_json::from_str(json).expect("deserialize tombstone");
        assert_eq!(run.deleted_at.as_deref(), Some("2026-07-08T09:00:00Z"));
    }
}
