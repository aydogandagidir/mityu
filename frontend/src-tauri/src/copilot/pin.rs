//! Pins: the live copilot's answers a human chose to keep (BACKLOG **I3c**,
//! ADR-0041 + ADR-0046).
//!
//! ## Why this is a separate buffer from the live context
//!
//! The obvious place for a pin is inside [`crate::copilot::session::LiveContext`],
//! next to the turns it cites. That is wrong, and silently so — which is the
//! reason this module has a header instead of a line of code.
//!
//! The pin can only be *written* once the meeting exists in the database, and
//! the meeting is created by `api_save_transcript`, which the renderer calls
//! **after** the recording has stopped. The stop path emits `recording-stopped`,
//! and `session::reset_context` empties the live context on that event. A pin
//! living in the context would therefore be cleared a moment before the save
//! that is supposed to persist it: the feature would appear to work, pin
//! nothing, and report nothing wrong.
//!
//! So pins live here, with a lifecycle bound to the SAVE rather than to the
//! recording:
//!
//! - cleared when a new session starts (`recording-started`) — a pin made
//!   during a recording the user then abandoned has no meeting to attach to and
//!   is gone, which the panel says out loud;
//! - **not** cleared by `recording-stopped`, nor by the idle-discard that drops
//!   a stale transcript buffer: between stop and save is exactly when pins must
//!   still exist;
//! - drained by the flush at save time, whether or not every pin could be
//!   written;
//! - dropped when the copilot is switched off.
//!
//! ## What a pin holds, and what it does not
//!
//! The claim text as the user saw it (already redacted — it came back through
//! [`crate::copilot::insight`], which redacts on the way into the prompt), the
//! `sequence_id`s it cited, and the label shown next to it. It does NOT hold
//! transcript text: the evidence is re-resolved from the saved segments at
//! flush time, so a pin can never carry a second copy of the conversation
//! around in memory.
//!
//! The buffer is bounded. A hotkey can be held down; an unbounded `Vec` of
//! model answers is an unbounded allocation driven by a keypress.

use crate::context::{AuthContext, TenantId};
use crate::modes::LiveAction;
use serde::Serialize;
use std::sync::{Mutex, MutexGuard};

/// The most pins one meeting may hold.
///
/// Generous for the use it is for — a human pressing a button during a
/// conversation — and small enough that the buffer can never become a memory
/// problem. Past the cap a pin is refused with a message, not dropped
/// silently: the user pressed a button and is owed an answer.
pub const MAX_PINS_PER_MEETING: usize = 100;

/// One kept answer, waiting for a meeting to attach to.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Pin {
    /// Stable id, so the panel can show a pin as pinned and un-pin it.
    pub id: String,
    /// The claim exactly as the user read it in the card.
    pub text: String,
    /// The `sequence_id`s this claim cited, in audio order. Resolved to real
    /// `transcripts` row ids at flush time; a pin whose ids do not resolve is
    /// reported, never guessed at (ADR-0046 decision 1).
    pub evidence: Vec<u64>,
    /// `mm:ss` label shown with the pin, from the cited turn.
    pub timestamp: String,
    /// Which action produced it.
    pub action: LiveAction,
    /// The mode that answered, for the block's label.
    pub mode_name: String,
}

/// The pins of the session in progress, owned by one workspace.
struct PinBuffer {
    tenant_id: TenantId,
    pins: Vec<Pin>,
}

static PINS: Mutex<Option<PinBuffer>> = Mutex::new(None);

fn buffer() -> MutexGuard<'static, Option<PinBuffer>> {
    PINS.lock().unwrap_or_else(|e| e.into_inner())
}

/// Why a pin was refused. Each renders as its own sentence in the panel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PinRefusal {
    /// The buffer already holds [`MAX_PINS_PER_MEETING`].
    Full,
    /// A claim with no citation cannot be pinned: the block it would become
    /// requires a `source_chunk_id`, and inventing one is the one thing this
    /// epic must never do.
    NoEvidence,
    /// The buffer belongs to another workspace (`docs/MULTITENANCY.md` rule 2).
    ForeignWorkspace,
}

/// Keep one answer. Idempotent on `id`: pressing Pin twice on the same claim
/// leaves one pin, so a double-click cannot duplicate a block.
pub fn add(ctx: &AuthContext, pin: Pin) -> Result<usize, PinRefusal> {
    if pin.evidence.is_empty() {
        return Err(PinRefusal::NoEvidence);
    }
    let mut guard = buffer();
    let buf = guard.get_or_insert_with(|| PinBuffer {
        tenant_id: ctx.tenant_id.clone(),
        pins: Vec::new(),
    });
    if buf.tenant_id != ctx.tenant_id {
        return Err(PinRefusal::ForeignWorkspace);
    }
    if let Some(existing) = buf.pins.iter_mut().find(|p| p.id == pin.id) {
        *existing = pin;
        return Ok(buf.pins.len());
    }
    if buf.pins.len() >= MAX_PINS_PER_MEETING {
        return Err(PinRefusal::Full);
    }
    buf.pins.push(pin);
    Ok(buf.pins.len())
}

/// Drop one pin by id. `true` when something was removed.
pub fn remove(ctx: &AuthContext, id: &str) -> bool {
    let mut guard = buffer();
    let Some(buf) = guard.as_mut() else {
        return false;
    };
    if buf.tenant_id != ctx.tenant_id {
        return false;
    }
    let before = buf.pins.len();
    buf.pins.retain(|p| p.id != id);
    before != buf.pins.len()
}

/// The ids currently pinned, for the panel's pinned/not-pinned rendering.
/// Empty for another workspace rather than an error: the panel is asking a
/// question about its own session, and the answer is "none".
pub fn pinned_ids(ctx: &AuthContext) -> Vec<String> {
    buffer()
        .as_ref()
        .filter(|buf| buf.tenant_id == ctx.tenant_id)
        .map(|buf| buf.pins.iter().map(|p| p.id.clone()).collect())
        .unwrap_or_default()
}

/// How many pins are waiting to be saved. Used by the panel's count and by
/// status; never fails and never leaks another workspace's number.
pub fn count(ctx: &AuthContext) -> usize {
    buffer()
        .as_ref()
        .filter(|buf| buf.tenant_id == ctx.tenant_id)
        .map_or(0, |buf| buf.pins.len())
}

/// Take every pin, leaving the buffer empty.
///
/// Called by the flush at save time. Draining rather than copying is
/// deliberate: the pins now belong to a saved meeting, and a pin left behind
/// would be written again into the NEXT meeting the user records.
pub fn drain(ctx: &AuthContext) -> Vec<Pin> {
    let mut guard = buffer();
    let Some(buf) = guard.as_mut() else {
        return Vec::new();
    };
    if buf.tenant_id != ctx.tenant_id {
        return Vec::new();
    }
    std::mem::take(&mut buf.pins)
}

/// Forget everything. A new session begins, or the copilot was switched off.
pub fn clear() {
    *buffer() = None;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::{RequestId, Role, UserId};

    /// The buffer is process-global, so these tests must not run beside each
    /// other — the same lock `session`'s tests use, for the same reason. Every
    /// test holds it for its whole body.
    fn serialised() -> MutexGuard<'static, ()> {
        static TEST_LOCK: Mutex<()> = Mutex::new(());
        TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Start from a known-empty buffer rather than from whatever ran before.
    fn fresh() -> AuthContext {
        clear();
        AuthContext::local()
    }

    fn other_tenant() -> AuthContext {
        AuthContext {
            tenant_id: TenantId::new("other-tenant"),
            user_id: UserId::new("other-user"),
            roles: vec![Role::Owner],
            request_id: RequestId::generate(),
        }
    }

    fn pin(id: &str) -> Pin {
        Pin {
            id: id.to_string(),
            text: "Budget was approved.".to_string(),
            evidence: vec![4],
            timestamp: "00:14".to_string(),
            action: LiveAction::Recap,
            mode_name: "General".to_string(),
        }
    }

    #[test]
    fn a_pin_is_held_until_it_is_drained() {
        let _guard = serialised();
        let ctx = fresh();
        assert_eq!(count(&ctx), 0);
        assert_eq!(add(&ctx, pin("p1")), Ok(1));
        assert_eq!(count(&ctx), 1);
        assert_eq!(pinned_ids(&ctx), vec!["p1".to_string()]);

        let drained = drain(&ctx);
        assert_eq!(drained.len(), 1);
        assert_eq!(count(&ctx), 0, "draining leaves nothing behind");
    }

    /// Draining rather than copying matters: a pin left in the buffer would be
    /// written into the NEXT meeting the user records.
    #[test]
    fn a_drained_pin_cannot_be_written_twice() {
        let _guard = serialised();
        let ctx = fresh();
        add(&ctx, pin("p1")).expect("held");
        assert_eq!(drain(&ctx).len(), 1);
        assert!(drain(&ctx).is_empty());
    }

    /// Pressing Pin twice on one claim is one pin. The panel keys a pin by the
    /// claim's own content, so a double-click arrives as the same id.
    #[test]
    fn pinning_the_same_claim_twice_stores_one() {
        let _guard = serialised();
        let ctx = fresh();
        assert_eq!(add(&ctx, pin("p1")), Ok(1));
        assert_eq!(add(&ctx, pin("p1")), Ok(1));
        assert_eq!(count(&ctx), 1);
    }

    /// A claim with no citation cannot become a block: `source_chunk_id` is
    /// required and inventing one is the one thing this epic must never do.
    #[test]
    fn a_claim_with_no_citation_is_refused() {
        let _guard = serialised();
        let ctx = fresh();
        let mut p = pin("p1");
        p.evidence.clear();
        assert_eq!(add(&ctx, p), Err(PinRefusal::NoEvidence));
        assert_eq!(count(&ctx), 0);
    }

    #[test]
    fn the_buffer_is_bounded_and_says_so() {
        let _guard = serialised();
        let ctx = fresh();
        for i in 0..MAX_PINS_PER_MEETING {
            add(&ctx, pin(&format!("p{i}"))).expect("held");
        }
        assert_eq!(add(&ctx, pin("one-too-many")), Err(PinRefusal::Full));
        assert_eq!(count(&ctx), MAX_PINS_PER_MEETING);
    }

    /// `docs/MULTITENANCY.md` rule 2. Another workspace can neither add to nor
    /// read nor drain this buffer.
    #[test]
    fn another_workspace_cannot_reach_these_pins() {
        let _guard = serialised();
        let ctx = fresh();
        add(&ctx, pin("p1")).expect("held");

        let foreign = other_tenant();
        assert_eq!(add(&foreign, pin("p2")), Err(PinRefusal::ForeignWorkspace));
        assert_eq!(count(&foreign), 0);
        assert!(pinned_ids(&foreign).is_empty());
        assert!(drain(&foreign).is_empty());
        assert!(!remove(&foreign, "p1"));

        // And ours is untouched by any of it.
        assert_eq!(count(&ctx), 1);
    }

    #[test]
    fn removing_a_pin_is_idempotent() {
        let _guard = serialised();
        let ctx = fresh();
        add(&ctx, pin("p1")).expect("held");
        assert!(remove(&ctx, "p1"));
        assert!(!remove(&ctx, "p1"));
        assert_eq!(count(&ctx), 0);
    }

    #[test]
    fn clear_forgets_everything() {
        let _guard = serialised();
        let ctx = fresh();
        add(&ctx, pin("p1")).expect("held");
        clear();
        assert_eq!(count(&ctx), 0);
    }
}
