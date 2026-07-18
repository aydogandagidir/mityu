//! Append-only capture of the human-in-the-loop correction signal (ADR-0024 §2).
//!
//! Every explicit human verdict on an AI draft — an edit, a rejection, an
//! approval, a restore — is appended here as one immutable row.
//!
//! The app has always *kept* the last correction
//! (`summaries.sections[].original_content`, `action_items.original_text`), but
//! that is state, not a log, and it cannot survive:
//! [`SummariesRepository::upsert_draft`](super::summary_draft::SummariesRepository::upsert_draft)
//! rewrites the whole `sections` JSON on every regeneration, so a user who
//! regenerates loses every delta they had produced — precisely while iterating
//! hardest. This table exists to outlive that rewrite. It is the fix, not a
//! convenience (see the `20260716000000` migration header).
//!
//! **Append-only is enforced HERE, at the repository layer**, because SQLite
//! cannot express it without a trigger and a trigger would fight the
//! `ON DELETE CASCADE` that erasure requires (ADR-0024 §10). The surface below is
//! therefore deliberately insert + read ONLY: no update method, no delete method.
//! The `updated_at`/`updated_by`/`rev`/`deleted_at` columns exist solely to
//! satisfy the CLAUDE.md §6 common-column contract and keep their insert-time
//! values forever (`rev = 1`, `updated_by` NULL — the never-synced baseline of
//! `20260702000000`).
//!
//! Only HUMAN actions are appended. System-driven state changes — notably the
//! ADR-0019 retranscription downgrade, which resets block statuses without anyone
//! saying anything — must pass `None` and leave no event: a learning signal
//! polluted with the app's own bookkeeping would teach the miner nothing about
//! the user.
//!
//! House rules (B2 / ADR-0010), all upheld: every method takes [`AuthContext`]
//! and scopes EVERY statement with `workspace_id = ctx.tenant_id`; reads never
//! cross a workspace boundary.
//!
//! CONTENT WARNING for maintainers: unlike every other table in this schema
//! except `transcripts`, these rows hold raw meeting content — both what the
//! model wrote and what the human replaced it with. Three consequences, none
//! optional: the rows are LOCAL-ONLY (no `SyncEntity`; ADR-0024 §11); they
//! CASCADE away with their meeting, so "delete my data" stays a real DELETE
//! (ADR-0024 §10); and the §0.6 rule that no content ever reaches a log line or
//! an error message applies with full force — every `tracing` call below carries
//! ids, tokens and counts only.

use crate::context::AuthContext;
use crate::database::repositories::summary_draft::SummaryDraftError;
use crate::summary::draft::BlockType;
use chrono::Utc;
use serde::Serialize;
use sqlx::{Row, SqliteConnection, SqlitePool};
use tracing::info;
use uuid::Uuid;

/// Which kind of item a correction was made against. Stored in
/// `correction_events.subject_kind`.
///
/// `subject_id` is polymorphic over this: a [`Self::SummaryBlock`] names a block
/// id living *inside* the `summaries.sections` JSON, while an [`Self::ActionItem`]
/// names a real `action_items` row. That is why the migration deliberately
/// carries no FK on `subject_id` (see its DELIBERATE header note).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CorrectionSubject {
    /// A block of a structured summary.
    SummaryBlock,
    /// An extracted action item.
    ActionItem,
}

impl CorrectionSubject {
    /// Wire/DB token, pinned by the tests below.
    pub(crate) fn to_db(self) -> &'static str {
        match self {
            Self::SummaryBlock => "summary_block",
            Self::ActionItem => "action_item",
        }
    }

    /// Parse a stored token; unknown tokens are a typed error (never content).
    pub(crate) fn from_db(token: &str) -> Result<Self, SummaryDraftError> {
        match token {
            "summary_block" => Ok(Self::SummaryBlock),
            "action_item" => Ok(Self::ActionItem),
            other => Err(SummaryDraftError::InvalidCorrectionToken {
                token: other.to_string(),
            }),
        }
    }
}

/// The human action that produced an event. Stored in `correction_events.action`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CorrectionAction {
    /// The human rewrote the text. The strongest signal there is: it carries both
    /// what the model said and what the human wanted instead.
    Edit,
    /// The human threw the item away. Weak on its own — "this was wrong" is not
    /// teachable — which is why `reason` exists (ADR-0024 §3).
    Reject,
    /// The human accepted the item. Approving an UNEDITED block is the strongest
    /// *positive* signal available: original and final are identical, i.e. the
    /// model got it exactly right, and the correction-burden metric (E1) reads
    /// that as zero burden.
    Approve,
    /// The human un-rejected an item (`rejected → draft`). Recorded because it
    /// RETRACTS an earlier reject: a miner that counts rejections without
    /// discounting the restores that undid them will learn a preference the user
    /// explicitly took back.
    Restore,
}

impl CorrectionAction {
    /// Wire/DB token, pinned by the tests below.
    pub(crate) fn to_db(self) -> &'static str {
        match self {
            Self::Edit => "edit",
            Self::Reject => "reject",
            Self::Approve => "approve",
            Self::Restore => "restore",
        }
    }

    /// Parse a stored token; unknown tokens are a typed error (never content).
    pub(crate) fn from_db(token: &str) -> Result<Self, SummaryDraftError> {
        match token {
            "edit" => Ok(Self::Edit),
            "reject" => Ok(Self::Reject),
            "approve" => Ok(Self::Approve),
            "restore" => Ok(Self::Restore),
            other => Err(SummaryDraftError::InvalidCorrectionToken {
                token: other.to_string(),
            }),
        }
    }
}

/// `BlockType` → the `correction_events.block_type` token. Matches the §4 wire
/// vocabulary (`text | bullet | heading1 | heading2`) so a stored event reads the
/// same as the block it describes.
pub(crate) fn block_type_to_db(block_type: BlockType) -> &'static str {
    match block_type {
        BlockType::Text => "text",
        BlockType::Bullet => "bullet",
        BlockType::Heading1 => "heading1",
        BlockType::Heading2 => "heading2",
    }
}

/// One correction about to be appended. Borrowed on purpose: it is built at the
/// exact point of mutation, where the pre- and post-state are both already in
/// hand, so nothing needs cloning to record it.
///
/// `original_text` must always be the text **the model produced**, not the
/// previous human revision — on a re-edit that means `original_content`, not
/// `content`. The miner and the burden metric both compare model → human, so a
/// human→human delta would silently understate the burden.
#[derive(Debug)]
pub struct NewCorrectionEvent<'a> {
    /// Meeting the corrected item belongs to (CASCADE anchor — ADR-0024 §10).
    pub meeting_id: &'a str,
    /// Which table/space `subject_id` lives in.
    pub subject_kind: CorrectionSubject,
    /// Block id (inside `summaries.sections` JSON) or `action_items.id`.
    pub subject_id: &'a str,
    /// What the human did.
    pub action: CorrectionAction,
    /// What the MODEL wrote (see the struct note).
    pub original_text: Option<&'a str>,
    /// What the human left behind. `None` for reject/restore (nothing survives).
    pub final_text: Option<&'a str>,
    /// Optional free-text rationale (ADR-0024 §3). Never blocks the action.
    pub reason: Option<&'a str>,
    /// §4 block-kind token; `None` for action items (they have no block kind).
    pub block_type: Option<&'a str>,
    /// Section heading the block sat under — lets the miner scope a rule to
    /// "in Risks sections". Captured HERE because `summaries.sections` will not
    /// survive a regeneration to be asked later.
    pub section_title: Option<&'a str>,
    /// Template in play at generation — same reason.
    pub template_id: Option<&'a str>,
    /// Model that produced the draft — lets the miner avoid attributing one
    /// model's habits to another.
    pub model: Option<&'a str>,
    /// The corrected item's evidence anchor, copied for convenience.
    pub source_chunk_id: Option<&'a str>,
}

/// One stored correction, hydrated back.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CorrectionEventRow {
    /// Row id (uuid v4).
    pub id: String,
    /// Owning meeting.
    pub meeting_id: String,
    /// Which space `subject_id` lives in.
    pub subject_kind: CorrectionSubject,
    /// Corrected block/item id.
    pub subject_id: String,
    /// What the human did.
    pub action: CorrectionAction,
    /// What the model wrote.
    pub original_text: Option<String>,
    /// What the human left behind.
    pub final_text: Option<String>,
    /// Free-text rationale, if the human gave one.
    pub reason: Option<String>,
    /// §4 block-kind token.
    pub block_type: Option<String>,
    /// Section heading at generation time.
    pub section_title: Option<String>,
    /// Template at generation time.
    pub template_id: Option<String>,
    /// Model at generation time.
    pub model: Option<String>,
    /// When the human did it (RFC 3339, as stored).
    pub created_at: String,
}

fn map_row(row: &sqlx::sqlite::SqliteRow) -> Result<CorrectionEventRow, SummaryDraftError> {
    Ok(CorrectionEventRow {
        id: row.get("id"),
        meeting_id: row.get("meeting_id"),
        subject_kind: CorrectionSubject::from_db(&row.get::<String, _>("subject_kind"))?,
        subject_id: row.get("subject_id"),
        action: CorrectionAction::from_db(&row.get::<String, _>("action"))?,
        original_text: row.get("original_text"),
        final_text: row.get("final_text"),
        reason: row.get("reason"),
        block_type: row.get("block_type"),
        section_title: row.get("section_title"),
        template_id: row.get("template_id"),
        model: row.get("model"),
        created_at: row.get("created_at"),
    })
}

/// Columns every read below selects, in `map_row` order.
const SELECT_COLS: &str = "id, meeting_id, subject_kind, subject_id, action, original_text, \
                           final_text, reason, block_type, section_title, template_id, model, \
                           created_at";

/// Append-only repository over `correction_events` (ADR-0024 §2).
///
/// Insert + read only, by design — see the module docs.
pub struct CorrectionEventsRepository;

impl CorrectionEventsRepository {
    /// Appends one event **inside the caller's transaction**.
    ///
    /// Transactional on purpose (ADR-0024 §2): the mutation and the record of it
    /// either both land or neither does, so the learning signal can never
    /// silently diverge from the state it claims to describe. Callers pass
    /// `&mut *tx`.
    pub(crate) async fn append_tx(
        conn: &mut SqliteConnection,
        ctx: &AuthContext,
        event: &NewCorrectionEvent<'_>,
    ) -> Result<String, SummaryDraftError> {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now();

        sqlx::query(
            "INSERT INTO correction_events \
             (id, workspace_id, meeting_id, subject_kind, subject_id, action, original_text, \
              final_text, reason, block_type, section_title, template_id, model, \
              source_chunk_id, created_at, updated_at, rev) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 1)",
        )
        .bind(&id)
        .bind(ctx.tenant_id.as_str())
        .bind(event.meeting_id)
        .bind(event.subject_kind.to_db())
        .bind(event.subject_id)
        .bind(event.action.to_db())
        .bind(event.original_text)
        .bind(event.final_text)
        .bind(event.reason)
        .bind(event.block_type)
        .bind(event.section_title)
        .bind(event.template_id)
        .bind(event.model)
        .bind(event.source_chunk_id)
        .bind(now)
        .bind(now)
        .execute(conn)
        .await?;

        // ids/tokens only — never `original_text`/`final_text`/`reason` (§0.6).
        info!(
            meeting_id = %event.meeting_id,
            event_id = %id,
            subject = event.subject_kind.to_db(),
            action = event.action.to_db(),
            "correction event appended"
        );
        Ok(id)
    }

    /// Every correction recorded for one meeting, oldest first — the evidence
    /// view behind a learned rule (ADR-0024 §4 `evidence`).
    ///
    /// Returns an empty vec, never an error, when the meeting has been deleted:
    /// its events cascaded away, and a rule pointing at them must degrade to
    /// "kanıt silindi" rather than blow up (ADR-0024 §10).
    pub async fn list_for_meeting(
        pool: &SqlitePool,
        ctx: &AuthContext,
        meeting_id: &str,
    ) -> Result<Vec<CorrectionEventRow>, SummaryDraftError> {
        let sql = format!(
            "SELECT {SELECT_COLS} FROM correction_events \
             WHERE workspace_id = ? AND meeting_id = ? ORDER BY created_at ASC, id ASC"
        );
        let rows = sqlx::query(&sql)
            .bind(ctx.tenant_id.as_str())
            .bind(meeting_id)
            .fetch_all(pool)
            .await?;
        rows.iter().map(map_row).collect()
    }

    /// The `limit` most recent corrections in this workspace, newest first —
    /// the miner's input (C1/C2).
    pub async fn list_recent(
        pool: &SqlitePool,
        ctx: &AuthContext,
        limit: i64,
    ) -> Result<Vec<CorrectionEventRow>, SummaryDraftError> {
        let sql = format!(
            "SELECT {SELECT_COLS} FROM correction_events \
             WHERE workspace_id = ? ORDER BY created_at DESC, id DESC LIMIT ?"
        );
        let rows = sqlx::query(&sql)
            .bind(ctx.tenant_id.as_str())
            .bind(limit.max(0))
            .fetch_all(pool)
            .await?;
        rows.iter().map(map_row).collect()
    }

    /// Fetch specific events by id, oldest first — resolves a rule's `evidence`
    /// list. Ids that no longer exist are simply absent from the result (the
    /// meeting was deleted); callers MUST treat a short result as dangling
    /// evidence, not as an error (ADR-0024 §10).
    pub async fn list_by_ids(
        pool: &SqlitePool,
        ctx: &AuthContext,
        ids: &[String],
    ) -> Result<Vec<CorrectionEventRow>, SummaryDraftError> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let mut out = Vec::with_capacity(ids.len());
        // Chunked like `resolve_source_chunk_ids`, for the same reason: SQLite's
        // default 999-host-parameter ceiling.
        for chunk in ids.chunks(super::summary_draft::MAX_IN_CLAUSE_IDS) {
            let placeholders = vec!["?"; chunk.len()].join(", ");
            let sql = format!(
                "SELECT {SELECT_COLS} FROM correction_events \
                 WHERE workspace_id = ? AND id IN ({placeholders}) \
                 ORDER BY created_at ASC, id ASC"
            );
            let mut query = sqlx::query(&sql).bind(ctx.tenant_id.as_str());
            for id in chunk {
                query = query.bind(id.as_str());
            }
            for row in query.fetch_all(pool).await? {
                out.push(map_row(&row)?);
            }
        }
        Ok(out)
    }

    /// How many corrections this workspace has recorded — the denominator of the
    /// correction-burden headline (D2/E1) and the `autoActivateMinSupport` sanity
    /// check (C3).
    pub async fn count(pool: &SqlitePool, ctx: &AuthContext) -> Result<i64, SummaryDraftError> {
        let count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM correction_events WHERE workspace_id = ?")
                .bind(ctx.tenant_id.as_str())
                .fetch_one(pool)
                .await?;
        Ok(count.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subject_tokens_round_trip() {
        for subject in [
            CorrectionSubject::SummaryBlock,
            CorrectionSubject::ActionItem,
        ] {
            assert_eq!(
                CorrectionSubject::from_db(subject.to_db()).unwrap(),
                subject
            );
        }
    }

    #[test]
    fn action_tokens_round_trip() {
        for action in [
            CorrectionAction::Edit,
            CorrectionAction::Reject,
            CorrectionAction::Approve,
            CorrectionAction::Restore,
        ] {
            assert_eq!(CorrectionAction::from_db(action.to_db()).unwrap(), action);
        }
    }

    /// The stored tokens are a contract the miner (C1) and any future migration
    /// read back; pin them literally so a rename cannot pass silently.
    #[test]
    fn tokens_are_pinned_literals() {
        assert_eq!(CorrectionSubject::SummaryBlock.to_db(), "summary_block");
        assert_eq!(CorrectionSubject::ActionItem.to_db(), "action_item");
        assert_eq!(CorrectionAction::Edit.to_db(), "edit");
        assert_eq!(CorrectionAction::Reject.to_db(), "reject");
        assert_eq!(CorrectionAction::Approve.to_db(), "approve");
        assert_eq!(CorrectionAction::Restore.to_db(), "restore");
    }

    /// `block_type` tokens must match the §4 wire vocabulary the blocks
    /// themselves serialize to, or an event will not describe its own block.
    #[test]
    fn block_type_tokens_match_the_ss4_wire_vocabulary() {
        assert_eq!(block_type_to_db(BlockType::Text), "text");
        assert_eq!(block_type_to_db(BlockType::Bullet), "bullet");
        assert_eq!(block_type_to_db(BlockType::Heading1), "heading1");
        assert_eq!(block_type_to_db(BlockType::Heading2), "heading2");
        // Cross-check against serde, the other definition of these tokens.
        for (block_type, token) in [
            (BlockType::Text, "text"),
            (BlockType::Bullet, "bullet"),
            (BlockType::Heading1, "heading1"),
            (BlockType::Heading2, "heading2"),
        ] {
            assert_eq!(
                serde_json::to_string(&block_type).unwrap(),
                format!("\"{token}\"")
            );
        }
    }

    #[test]
    fn unknown_tokens_are_typed_errors_and_carry_no_content() {
        let err = CorrectionAction::from_db("sabotage").unwrap_err();
        assert!(matches!(
            err,
            SummaryDraftError::InvalidCorrectionToken { .. }
        ));
        assert!(CorrectionSubject::from_db("").is_err());
    }
}
