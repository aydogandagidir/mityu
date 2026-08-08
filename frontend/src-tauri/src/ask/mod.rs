//! "Ask This Meeting" — retrieval-first answers over one meeting's transcript
//! (Product Intelligence slice 3).
//!
//! The shape is deliberate and is what separates this from a chatbot over a
//! transcript: **retrieve first, then answer only from what was retrieved, and
//! refuse rather than guess.**
//!
//! 1. Retrieve the meeting's most relevant passages
//!    ([`TranscriptsRepository::retrieve_meeting_evidence`]) — local, lexical,
//!    tenant-scoped.
//! 2. If nothing is retrieved, **no model is called**. A question the transcript
//!    cannot support must not be answered from a model's prior knowledge.
//! 3. Otherwise the model sees only those passages and must cite one per claim.
//! 4. Every claim is then ground-checked against the retrieved window
//!    ([`grounding`]); a citation to anything else is dropped, never repaired.
//! 5. If nothing survives, the answer is a refusal — not an empty answer.
//!
//! ## Invariants
//!
//! - **Draft, never approved output.** An answer is a read-only view: it is not
//!   persisted, does not enter the summary/action-item tables, and cannot
//!   become approved content (`CLAUDE.md` §0.5). Nothing here writes.
//! - **Source-linked or absent.** Every rendered claim carries a resolvable
//!   `source_chunk_id` plus timing taken from the retrieved passage.
//! - **Tenant-scoped.** Retrieval binds `workspace_id` from `AuthContext` on
//!   every joined table, and the meeting id on both the derived index and the
//!   authoritative row.
//! - **Local-first.** Retrieval is pure SQLite FTS5 and needs no network. The
//!   answering step uses whichever provider the user configured — the built-in
//!   local model included.

pub mod commands;
pub mod grounding;
pub mod service;

pub use grounding::{
    ground_claims, DropReason, DroppedClaim, GroundedClaim, GroundingOutcome, RawClaim,
};
pub use service::{ask_meeting, AskError, AskOutcome};
