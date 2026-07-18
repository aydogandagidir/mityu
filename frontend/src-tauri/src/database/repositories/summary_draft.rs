//! Tenant-scoped repository for the C1 structured `summaries` table (BACKLOG
//! C1.3 — docs/CONTRACTS.md §2 + §4, ADR-0019).
//!
//! Persists [`MeetingNotesDraft`] (src/summary/draft.rs) into the `summaries`
//! table created by migration `20260706000000`: ONE row per meeting
//! (`meeting_id` UNIQUE), the §4 sections JSON in the `sections` column, and
//! the HITL lifecycle in `status` / `approved_at` / `approved_by`.
//!
//! House rules (B2 / ADR-0010), all upheld here:
//! - every method takes [`AuthContext`] and scopes EVERY statement with
//!   `workspace_id = ctx.tenant_id`;
//! - every write bumps `rev` and stamps `updated_by` + RFC 3339 `updated_at`
//!   (bound as `chrono::DateTime<Utc>`, the standardized writer format);
//! - cross-workspace access degrades to `Ok(false)` (or a typed not-found
//!   error on creation paths) without touching the foreign row — the Phase-2
//!   server maps these to 404 (ADR-0010 Phase-2 rules).
//!
//! HITL invariants (CLAUDE.md §0.5, docs/CONTRACTS.md §4, ADR-0019):
//! - **Generation may never produce an approved summary.**
//!   [`SummariesRepository::upsert_draft`] persists per-block statuses exactly
//!   as given but FORCES the summary-level status to `draft` on every write.
//!   Only [`SummariesRepository::approve_summary`] — an explicit human action —
//!   can set `approved`, stamping `approved_at`/`approved_by`.
//! - **Evidence must resolve.** [`resolve_source_chunk_ids`] is THE single
//!   definition of "resolvable" (also used by the action-items repository): the
//!   cited id exists in `transcripts` for the same meeting AND the caller's
//!   workspace. It is enforced at write time (upsert) and RE-checked at approve
//!   time, because there is deliberately no SQL FK on `source_chunk_id`
//!   (retranscription deletes + re-inserts segment rows; see the migration
//!   header and ADR-0019).
//! - **Errors carry counts, never content.** A failed validation names how many
//!   ids did not resolve; no block text or transcript content is ever placed in
//!   an error message or a log line (ids, counts, and reason codes only).
//!
//! Unlike the older repositories (hard-delete flows only — ADR-0010), the two
//! C1 tables carry soft-delete flows from birth: reads exclude rows with
//! `deleted_at IS NOT NULL`, and [`SummariesRepository::soft_delete`] sets
//! `deleted_at` rather than removing the row (sync-class table).
//!
//! Concurrency note: block-level mutations are a read-modify-write over the
//! sections JSON issued on the pool (approve-time revalidation must run between
//! the read and the write, and [`resolve_source_chunk_ids`] takes the pool).
//! Phase 1 is a single-writer local SQLite app, so a lost update between two
//! concurrent block writes is not a reachable state today; Phase-2 server
//! endpoints get optimistic concurrency via `rev` instead.

use crate::context::AuthContext;
use crate::database::repositories::correction_event::{
    block_type_to_db, CorrectionAction, CorrectionEventsRepository, CorrectionSubject,
    NewCorrectionEvent,
};
use crate::learning::rule::AppliedRule;
use crate::summary::draft::{BlockStatus, DraftSection, MeetingNotesDraft, SummaryStatus};
use chrono::Utc;
use serde::Serialize;
use sqlx::{Row, SqlitePool};
use std::collections::HashSet;
use thiserror::Error;
use tracing::{info, warn};
use uuid::Uuid;

/// Maximum number of ids bound into a single `IN (...)` clause by
/// [`resolve_source_chunk_ids`]. Kept comfortably under SQLite's default
/// 999-host-parameter limit (each chunk also binds meeting + workspace ids).
pub(crate) const MAX_IN_CLAUSE_IDS: usize = 500;

/// Typed error for the C1 summary/action-item repositories
/// (docs/CONVENTIONS.md: typed domain errors via `thiserror`).
///
/// Deliberately content-free: variants carry ids/counts only, never block or
/// transcript text (CLAUDE.md §0.6 — no meeting content in errors/logs).
#[derive(Debug, Error)]
pub enum SummaryDraftError {
    /// Underlying database failure.
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    /// One or more `source_chunk_id` references do not resolve to a transcript
    /// segment of this meeting in this workspace (docs/CONTRACTS.md §4). Names
    /// the COUNT of distinct unresolvable ids, never their content.
    #[error(
        "{count} source_chunk_id reference(s) do not resolve to transcript \
         segments of this meeting in this workspace"
    )]
    UnresolvableSources {
        /// Number of DISTINCT cited ids that failed to resolve.
        count: usize,
    },
    /// The target meeting does not exist in the caller's workspace. The
    /// cross-workspace analogue of the repositories' `Ok(false)` convention for
    /// creation paths that must return an id (Phase-2 servers map this to 404,
    /// ADR-0010).
    #[error("meeting not found in this workspace")]
    MeetingNotFound,
    /// The persisted `sections` JSON (or an outgoing payload) failed to
    /// (de)serialize against the §4 shapes.
    #[error("summary sections payload failed to (de)serialize: {0}")]
    Sections(#[from] serde_json::Error),
    /// A `status` column held a token outside the §4 vocabulary (corrupt or
    /// hand-edited row). Status tokens are not user content.
    #[error("unknown status token in database: {token}")]
    InvalidStatus {
        /// The offending stored token.
        token: String,
    },
    /// A `correction_events.subject_kind` or `.action` column held a token
    /// outside the ADR-0024 §2 vocabulary (corrupt or hand-edited row). Like
    /// [`Self::InvalidStatus`], these tokens are a closed vocabulary, not user
    /// content, so echoing one leaks nothing.
    #[error("unknown correction token in database: {token}")]
    InvalidCorrectionToken {
        /// The offending stored token.
        token: String,
    },
}

impl SummaryDraftError {
    /// Adapter for callers living in `sqlx::Error` space (e.g. the ADR-0019
    /// retranscription hook inside the transcripts repository): unwraps the
    /// database variant verbatim and folds every other variant into
    /// `sqlx::Error::Protocol` (message is content-free by construction).
    pub fn into_sqlx(self) -> sqlx::Error {
        match self {
            SummaryDraftError::Database(e) => e,
            other => sqlx::Error::Protocol(other.to_string()),
        }
    }
}

/// One persisted structured summary, hydrated back into the §4 draft shape.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SummaryDraftRow {
    /// Row id (uuid v4, minted on first insert).
    pub id: String,
    /// Owning meeting.
    pub meeting_id: String,
    /// Summary-level HITL status (mirrors `draft.status`).
    pub status: SummaryStatus,
    /// Provider/model that generated the draft, if recorded.
    pub model: Option<String>,
    /// Summary template used, if recorded.
    pub template_id: Option<String>,
    /// The §4 draft reconstructed from the stored sections JSON + status.
    pub draft: MeetingNotesDraft,
    /// The learned rules that shaped this draft, snapshotted at generation
    /// (ADR-0024 §5).
    ///
    /// `None` and `Some(vec![])` are DIFFERENT answers and must stay that way:
    /// `None` means nothing was ever recorded here (a pre-migration row, the
    /// first-run sample, or a summary from the legacy markdown path), while
    /// `Some(vec![])` means the learning system ran for this summary and applied
    /// nothing. "We do not know" is not "no".
    pub applied_rules: Option<Vec<AppliedRule>>,
    /// When the draft was (re)generated (RFC 3339, as stored).
    pub generated_at: Option<String>,
    /// When a human approved the summary (RFC 3339, as stored).
    pub approved_at: Option<String>,
    /// Who approved the summary (`AuthContext::user_id` at approve time).
    pub approved_by: Option<String>,
    /// Sync revision counter.
    pub rev: i64,
}

/// THE single definition of "resolvable" for `source_chunk_id` values
/// (docs/CONTRACTS.md §4; shared by [`SummariesRepository`] and
/// `ActionItemsRepository`): an id resolves iff a `transcripts` row with that
/// id exists for the SAME meeting in the CALLER'S workspace. Returns the subset
/// of `ids` that resolve. Chunks the `IN` clause at [`MAX_IN_CLAUSE_IDS`] ids.
pub(crate) async fn resolve_source_chunk_ids(
    pool: &SqlitePool,
    ctx: &AuthContext,
    meeting_id: &str,
    ids: &HashSet<String>,
) -> Result<HashSet<String>, sqlx::Error> {
    let mut resolved = HashSet::with_capacity(ids.len());
    if ids.is_empty() {
        return Ok(resolved);
    }
    let all: Vec<&str> = ids.iter().map(String::as_str).collect();
    for chunk in all.chunks(MAX_IN_CLAUSE_IDS) {
        let placeholders = vec!["?"; chunk.len()].join(", ");
        let sql = format!(
            "SELECT id FROM transcripts \
             WHERE meeting_id = ? AND workspace_id = ? AND id IN ({placeholders})"
        );
        let mut query = sqlx::query_scalar::<_, String>(&sql)
            .bind(meeting_id)
            .bind(ctx.tenant_id.as_str());
        for id in chunk {
            query = query.bind(*id);
        }
        for id in query.fetch_all(pool).await? {
            resolved.insert(id);
        }
    }
    Ok(resolved)
}

/// Write-time evidence gate shared by `upsert_draft` and
/// `ActionItemsRepository::insert_drafts`: every id in `required` must resolve
/// per [`resolve_source_chunk_ids`], else a typed error naming the count of
/// distinct unresolvable ids (never their content) is returned.
pub(crate) async fn ensure_sources_resolve(
    pool: &SqlitePool,
    ctx: &AuthContext,
    meeting_id: &str,
    required: &HashSet<String>,
) -> Result<(), SummaryDraftError> {
    let resolved = resolve_source_chunk_ids(pool, ctx, meeting_id, required).await?;
    // `resolved` is always a subset of `required` (the query filters on the
    // bound id list), so plain subtraction is the count of missing ids.
    let count = required.len() - resolved.len();
    if count > 0 {
        warn!(
            meeting_id = %meeting_id,
            unresolvable = count,
            "source_chunk_id validation failed: cited segment(s) not found in this workspace"
        );
        return Err(SummaryDraftError::UnresolvableSources { count });
    }
    Ok(())
}

/// The §4 block/action-item status machine (BACKLOG C1 plan):
/// `draft → approved | edited | rejected`, `approved → edited | rejected`,
/// `edited → approved | rejected`, `rejected → draft` (restore). Everything
/// else — including self-transitions — is illegal.
pub(crate) fn block_transition_allowed(from: BlockStatus, to: BlockStatus) -> bool {
    use BlockStatus::{Approved, Draft, Edited, Rejected};
    matches!(
        (from, to),
        (Draft, Approved)
            | (Draft, Edited)
            | (Draft, Rejected)
            | (Approved, Edited)
            | (Approved, Rejected)
            | (Edited, Approved)
            | (Edited, Rejected)
            | (Rejected, Draft)
    )
}

/// §4 wire token for a [`BlockStatus`], as stored in status columns
/// (`draft | approved | edited | rejected` — pinned by `summary::draft` tests).
pub(crate) fn block_status_to_db(status: BlockStatus) -> &'static str {
    match status {
        BlockStatus::Draft => "draft",
        BlockStatus::Approved => "approved",
        BlockStatus::Edited => "edited",
        BlockStatus::Rejected => "rejected",
    }
}

/// Parse a stored [`BlockStatus`] token; unknown tokens are a typed error.
pub(crate) fn block_status_from_db(token: &str) -> Result<BlockStatus, SummaryDraftError> {
    match token {
        "draft" => Ok(BlockStatus::Draft),
        "approved" => Ok(BlockStatus::Approved),
        "edited" => Ok(BlockStatus::Edited),
        "rejected" => Ok(BlockStatus::Rejected),
        other => Err(SummaryDraftError::InvalidStatus {
            token: other.to_string(),
        }),
    }
}

/// Parse a stored [`SummaryStatus`] token (`draft | approved`).
pub(crate) fn summary_status_from_db(token: &str) -> Result<SummaryStatus, SummaryDraftError> {
    match token {
        "draft" => Ok(SummaryStatus::Draft),
        "approved" => Ok(SummaryStatus::Approved),
        other => Err(SummaryDraftError::InvalidStatus {
            token: other.to_string(),
        }),
    }
}

pub struct SummariesRepository;

impl SummariesRepository {
    /// Persists a generated [`MeetingNotesDraft`] for `draft.meeting_id`:
    /// INSERT on first write (id = uuid v4), UPDATE on regeneration (the table
    /// holds ONE row per meeting — `meeting_id` UNIQUE). Returns the row id.
    ///
    /// Gates, in order:
    /// 1. the meeting must exist in the caller's workspace (else
    ///    [`SummaryDraftError::MeetingNotFound`] — also prevents a foreign
    ///    context from squatting the UNIQUE `meeting_id` slot of another
    ///    workspace's meeting);
    /// 2. EVERY block's `source_chunk_id` must resolve per
    ///    [`resolve_source_chunk_ids`] (else
    ///    [`SummaryDraftError::UnresolvableSources`] naming the count).
    ///
    /// INVARIANT (docs/CONTRACTS.md §4, ADR-0019): per-block statuses are
    /// persisted exactly as given, but the summary-level status is FORCED to
    /// `draft` — generation may never produce an approved summary; only
    /// [`Self::approve_summary`] (a human action) sets `approved`. On update,
    /// `approved_at`/`approved_by` are cleared (stale approval evidence must
    /// not survive a regeneration), `generated_at` is re-stamped, `rev` bumps,
    /// and a soft-deleted row is revived (`deleted_at = NULL`) — regeneration
    /// is the designated way to bring a meeting's summary back.
    ///
    /// `applied_rules` is the ADR-0024 §5 snapshot of the learned rules that
    /// shaped THIS draft — a parameter of this method, not a later `UPDATE`, so
    /// that a draft can never exist without the record of what shaped it. An
    /// empty slice is written as `'[]'`, which is a claim ("the learning system
    /// ran and applied nothing") and is deliberately distinct from the column's
    /// `NULL` ("nothing was ever recorded"); regeneration REPLACES the snapshot,
    /// because the snapshot describes the draft now in the row, not the history
    /// of that meeting.
    pub async fn upsert_draft(
        pool: &SqlitePool,
        ctx: &AuthContext,
        draft: &MeetingNotesDraft,
        model: Option<&str>,
        template_id: Option<&str>,
        applied_rules: &[AppliedRule],
    ) -> Result<String, SummaryDraftError> {
        let meeting_id = draft.meeting_id.as_str();

        let meeting_exists: Option<(i64,)> =
            sqlx::query_as("SELECT 1 FROM meetings WHERE id = ? AND workspace_id = ?")
                .bind(meeting_id)
                .bind(ctx.tenant_id.as_str())
                .fetch_optional(pool)
                .await?;
        if meeting_exists.is_none() {
            info!(
                meeting_id = %meeting_id,
                "upsert_draft: meeting not found in this workspace"
            );
            return Err(SummaryDraftError::MeetingNotFound);
        }

        let required: HashSet<String> = draft
            .sections
            .iter()
            .flat_map(|section| section.blocks.iter())
            .map(|block| block.source_chunk_id.clone())
            .collect();
        ensure_sources_resolve(pool, ctx, meeting_id, &required).await?;

        let sections_json = serde_json::to_string(&draft.sections)?;
        let applied_rules_json = serde_json::to_string(applied_rules)?;
        let now = Utc::now();

        let mut transaction = pool.begin().await?;

        let existing: Option<(String,)> =
            sqlx::query_as("SELECT id FROM summaries WHERE meeting_id = ? AND workspace_id = ?")
                .bind(meeting_id)
                .bind(ctx.tenant_id.as_str())
                .fetch_optional(&mut *transaction)
                .await?;

        let id = match existing {
            Some((id,)) => {
                sqlx::query(
                    "UPDATE summaries SET status = 'draft', model = ?, template_id = ?, \
                     sections = ?, applied_rules = ?, generated_at = ?, approved_at = NULL, \
                     approved_by = NULL, deleted_at = NULL, updated_at = ?, updated_by = ?, \
                     rev = rev + 1 \
                     WHERE id = ? AND workspace_id = ?",
                )
                .bind(model)
                .bind(template_id)
                .bind(&sections_json)
                .bind(&applied_rules_json)
                .bind(now)
                .bind(now)
                .bind(ctx.user_id.as_str())
                .bind(&id)
                .bind(ctx.tenant_id.as_str())
                .execute(&mut *transaction)
                .await?;
                id
            }
            None => {
                let id = Uuid::new_v4().to_string();
                sqlx::query(
                    "INSERT INTO summaries \
                     (id, meeting_id, workspace_id, status, model, template_id, sections, \
                      applied_rules, generated_at, created_at, updated_at, updated_by, rev) \
                     VALUES (?, ?, ?, 'draft', ?, ?, ?, ?, ?, ?, ?, ?, 1)",
                )
                .bind(&id)
                .bind(meeting_id)
                .bind(ctx.tenant_id.as_str())
                .bind(model)
                .bind(template_id)
                .bind(&sections_json)
                .bind(&applied_rules_json)
                .bind(now)
                .bind(now)
                .bind(now)
                .bind(ctx.user_id.as_str())
                .execute(&mut *transaction)
                .await?;
                id
            }
        };

        transaction.commit().await?;

        info!(
            meeting_id = %meeting_id,
            summary_id = %id,
            "persisted structured summary draft"
        );
        Ok(id)
    }

    /// Fetches the meeting's structured summary (workspace-scoped; excludes
    /// soft-deleted rows) and hydrates the stored sections JSON back into the
    /// §4 [`MeetingNotesDraft`] shape.
    pub async fn get_by_meeting(
        pool: &SqlitePool,
        ctx: &AuthContext,
        meeting_id: &str,
    ) -> Result<Option<SummaryDraftRow>, SummaryDraftError> {
        let row = sqlx::query(
            "SELECT id, status, model, template_id, sections, applied_rules, generated_at, \
             approved_at, approved_by, rev FROM summaries \
             WHERE meeting_id = ? AND workspace_id = ? AND deleted_at IS NULL",
        )
        .bind(meeting_id)
        .bind(ctx.tenant_id.as_str())
        .fetch_optional(pool)
        .await?;

        let Some(row) = row else {
            return Ok(None);
        };

        let status = summary_status_from_db(&row.get::<String, _>("status"))?;
        let sections: Vec<DraftSection> = serde_json::from_str(&row.get::<String, _>("sections"))?;
        // `Option` all the way out: `NULL` ("never recorded") and `[]` ("recorded,
        // nothing applied") are different answers to an audit, and collapsing them
        // here would throw away the distinction the column exists to preserve
        // (ADR-0024 §5).
        let applied_rules: Option<Vec<AppliedRule>> = row
            .get::<Option<String>, _>("applied_rules")
            .map(|json| serde_json::from_str(&json))
            .transpose()?;

        Ok(Some(SummaryDraftRow {
            id: row.get("id"),
            meeting_id: meeting_id.to_string(),
            status,
            model: row.get("model"),
            template_id: row.get("template_id"),
            draft: MeetingNotesDraft {
                meeting_id: meeting_id.to_string(),
                status,
                sections,
            },
            applied_rules,
            generated_at: row.get("generated_at"),
            approved_at: row.get("approved_at"),
            approved_by: row.get("approved_by"),
            rev: row.get("rev"),
        }))
    }

    /// Applies the [`block_transition_allowed`] status machine to one block
    /// (read-modify-write of the sections JSON). Approving RE-validates that
    /// the block's `source_chunk_id` resolves NOW (retranscription may have
    /// removed the cited segment since generation — ADR-0019).
    ///
    /// `Ok(false)` (with a content-free `tracing` reason) when: no summary row
    /// exists in this workspace, the block id is unknown, the transition is
    /// illegal, or approve-time evidence no longer resolves.
    ///
    /// INVARIANT: any successful block-level mutation forces the summary-level
    /// status back to `draft` and clears `approved_at`/`approved_by` — every
    /// reachable transition on a block of an approved summary (`approved →
    /// edited|rejected`, `rejected → draft`) breaks or alters the approved set,
    /// so the whole summary must be re-approved by a human.
    /// `reason` is the human's optional rationale, recorded on the correction
    /// event (ADR-0024 §3). It is most meaningful on a reject — "this was wrong"
    /// teaches nothing, "wrong because X" teaches — but is accepted on any
    /// transition and NEVER gates one: a refusal to explain must never cost the
    /// user their verdict.
    pub async fn set_block_status(
        pool: &SqlitePool,
        ctx: &AuthContext,
        meeting_id: &str,
        block_id: &str,
        new: BlockStatus,
        reason: Option<&str>,
    ) -> Result<bool, SummaryDraftError> {
        let row = sqlx::query(
            "SELECT id, sections, model, template_id FROM summaries \
             WHERE meeting_id = ? AND workspace_id = ? AND deleted_at IS NULL",
        )
        .bind(meeting_id)
        .bind(ctx.tenant_id.as_str())
        .fetch_optional(pool)
        .await?;
        let Some(row) = row else {
            info!(
                meeting_id = %meeting_id,
                block_id = %block_id,
                reason_code = "summary_not_found",
                "set_block_status refused"
            );
            return Ok(false);
        };
        let summary_id: String = row.get("id");
        let model: Option<String> = row.get("model");
        let template_id: Option<String> = row.get("template_id");
        let mut sections: Vec<DraftSection> =
            serde_json::from_str(&row.get::<String, _>("sections"))?;

        // Located by index rather than by `flat_map(..).find(..)` so the owning
        // section stays reachable: its title scopes the correction event, and
        // `sections` will not survive a regeneration to be asked later.
        let Some((section_index, block_index)) = sections.iter().enumerate().find_map(|(si, s)| {
            s.blocks
                .iter()
                .position(|b| b.id == block_id)
                .map(|bi| (si, bi))
        }) else {
            info!(
                meeting_id = %meeting_id,
                block_id = %block_id,
                reason_code = "block_not_found",
                "set_block_status refused"
            );
            return Ok(false);
        };

        let section_title = sections[section_index].title.clone();
        let block = &sections[section_index].blocks[block_index];
        let current = block.status;
        let block_type = block_type_to_db(block.block_type);
        let source_chunk_id = block.source_chunk_id.clone();

        if !block_transition_allowed(current, new) {
            info!(
                meeting_id = %meeting_id,
                block_id = %block_id,
                from = block_status_to_db(current),
                to = block_status_to_db(new),
                reason_code = "illegal_transition",
                "set_block_status refused"
            );
            return Ok(false);
        }

        if new == BlockStatus::Approved {
            let required = HashSet::from([source_chunk_id.clone()]);
            let resolved = resolve_source_chunk_ids(pool, ctx, meeting_id, &required).await?;
            if resolved.len() != required.len() {
                warn!(
                    meeting_id = %meeting_id,
                    block_id = %block_id,
                    reason_code = "unresolvable_source",
                    "set_block_status refused: cited segment no longer resolves"
                );
                return Ok(false);
            }
        }

        // What the MODEL wrote: `original_content` once an edit has happened,
        // otherwise the content itself (ADR-0024 §2 — the signal is always
        // model→human, never human→human).
        let block = &mut sections[section_index].blocks[block_index];
        let model_text = block
            .original_content
            .clone()
            .unwrap_or_else(|| block.content.clone());
        let surviving_text = block.content.clone();
        block.status = new;

        let sections_json = serde_json::to_string(&sections)?;

        let action = match new {
            BlockStatus::Approved => CorrectionAction::Approve,
            BlockStatus::Rejected => CorrectionAction::Reject,
            BlockStatus::Draft => CorrectionAction::Restore,
            // Not reached from the commands (`edit_block` owns edits), but the
            // status machine permits `draft|approved → edited`, so map it rather
            // than record an approve that never happened.
            BlockStatus::Edited => CorrectionAction::Edit,
        };
        let event = NewCorrectionEvent {
            meeting_id,
            subject_kind: CorrectionSubject::SummaryBlock,
            subject_id: block_id,
            action,
            original_text: Some(model_text.as_str()),
            // Text survives an approve; a reject leaves nothing standing, and a
            // restore only retracts an earlier verdict.
            final_text: match new {
                BlockStatus::Approved | BlockStatus::Edited => Some(surviving_text.as_str()),
                BlockStatus::Rejected | BlockStatus::Draft => None,
            },
            reason,
            block_type: Some(block_type),
            section_title: Some(section_title.as_str()),
            template_id: template_id.as_deref(),
            model: model.as_deref(),
            source_chunk_id: Some(source_chunk_id.as_str()),
        };

        Self::write_sections_as_draft(pool, ctx, &summary_id, &sections_json, Some(&event)).await?;
        Ok(true)
    }

    /// Human edit of one block's content (read-modify-write). NEVER touches
    /// `source_chunk_id` (the evidence anchor is immutable — §4). On the FIRST
    /// edit, the pre-edit text is preserved in `original_content`; the block's
    /// status becomes [`BlockStatus::Edited`], and the summary-level status
    /// drops back to `draft` if it was approved (the approved content changed).
    ///
    /// A rejected block cannot be edited (`rejected → draft` restore first —
    /// the status machine has no `rejected → edited` arc). Re-editing an
    /// already-edited block is allowed: that is a content operation, not a
    /// status transition, and `original_content` (first generated text) is kept.
    pub async fn edit_block(
        pool: &SqlitePool,
        ctx: &AuthContext,
        meeting_id: &str,
        block_id: &str,
        content: &str,
    ) -> Result<bool, SummaryDraftError> {
        let row = sqlx::query(
            "SELECT id, sections, model, template_id FROM summaries \
             WHERE meeting_id = ? AND workspace_id = ? AND deleted_at IS NULL",
        )
        .bind(meeting_id)
        .bind(ctx.tenant_id.as_str())
        .fetch_optional(pool)
        .await?;
        let Some(row) = row else {
            info!(
                meeting_id = %meeting_id,
                block_id = %block_id,
                reason_code = "summary_not_found",
                "edit_block refused"
            );
            return Ok(false);
        };
        let summary_id: String = row.get("id");
        let model: Option<String> = row.get("model");
        let template_id: Option<String> = row.get("template_id");
        let mut sections: Vec<DraftSection> =
            serde_json::from_str(&row.get::<String, _>("sections"))?;

        // By index, so the owning section's title stays reachable for the
        // correction event (see `set_block_status`).
        let Some((section_index, block_index)) = sections.iter().enumerate().find_map(|(si, s)| {
            s.blocks
                .iter()
                .position(|b| b.id == block_id)
                .map(|bi| (si, bi))
        }) else {
            info!(
                meeting_id = %meeting_id,
                block_id = %block_id,
                reason_code = "block_not_found",
                "edit_block refused"
            );
            return Ok(false);
        };

        let section_title = sections[section_index].title.clone();
        let block = &mut sections[section_index].blocks[block_index];

        if block.status == BlockStatus::Rejected {
            info!(
                meeting_id = %meeting_id,
                block_id = %block_id,
                reason_code = "illegal_transition",
                "edit_block refused: rejected blocks must be restored to draft first"
            );
            return Ok(false);
        }

        // Capture what the MODEL wrote BEFORE mutating — order matters here: on a
        // re-edit that is `original_content`, but on the FIRST edit it is
        // `content`, which the very next lines are about to move into
        // `original_content` and overwrite. Reading it afterwards would record a
        // human→human delta on every re-edit and understate the burden (E1).
        let model_text = block
            .original_content
            .clone()
            .unwrap_or_else(|| block.content.clone());
        let block_type = block_type_to_db(block.block_type);
        let source_chunk_id = block.source_chunk_id.clone();

        if block.original_content.is_none() {
            block.original_content = Some(block.content.clone());
        }
        block.content = content.to_string();
        block.status = BlockStatus::Edited;
        let sections_json = serde_json::to_string(&sections)?;

        let event = NewCorrectionEvent {
            meeting_id,
            subject_kind: CorrectionSubject::SummaryBlock,
            subject_id: block_id,
            action: CorrectionAction::Edit,
            original_text: Some(model_text.as_str()),
            final_text: Some(content),
            // Edits speak for themselves — the delta IS the rationale. `reason`
            // earns its keep on rejects (ADR-0024 §3).
            reason: None,
            block_type: Some(block_type),
            section_title: Some(section_title.as_str()),
            template_id: template_id.as_deref(),
            model: model.as_deref(),
            source_chunk_id: Some(source_chunk_id.as_str()),
        };

        Self::write_sections_as_draft(pool, ctx, &summary_id, &sections_json, Some(&event)).await?;
        Ok(true)
    }

    /// The explicit HUMAN approval of a whole summary (docs/CONTRACTS.md §4).
    /// Gate — ALL must hold at this moment:
    /// 1. at least one non-rejected block exists;
    /// 2. every non-rejected block is [`BlockStatus::Approved`];
    /// 3. every such block's `source_chunk_id` resolves NOW
    ///    ([`resolve_source_chunk_ids`] — re-checked because retranscription
    ///    may have removed cited segments since the blocks were approved).
    ///
    /// On success: `status = 'approved'`, `approved_at = now`,
    /// `approved_by = ctx.user_id`, `rev` bump. Any gate failure returns
    /// `Ok(false)` with a content-free `tracing` reason code.
    pub async fn approve_summary(
        pool: &SqlitePool,
        ctx: &AuthContext,
        meeting_id: &str,
    ) -> Result<bool, SummaryDraftError> {
        let row = sqlx::query(
            "SELECT id, sections FROM summaries \
             WHERE meeting_id = ? AND workspace_id = ? AND deleted_at IS NULL",
        )
        .bind(meeting_id)
        .bind(ctx.tenant_id.as_str())
        .fetch_optional(pool)
        .await?;
        let Some(row) = row else {
            info!(
                meeting_id = %meeting_id,
                reason_code = "summary_not_found",
                "approve_summary refused"
            );
            return Ok(false);
        };
        let summary_id: String = row.get("id");
        let sections: Vec<DraftSection> = serde_json::from_str(&row.get::<String, _>("sections"))?;

        let non_rejected: Vec<_> = sections
            .iter()
            .flat_map(|section| section.blocks.iter())
            .filter(|block| block.status != BlockStatus::Rejected)
            .collect();

        if non_rejected.is_empty() {
            warn!(
                meeting_id = %meeting_id,
                reason_code = "no_approvable_blocks",
                "approve_summary refused"
            );
            return Ok(false);
        }
        let unapproved = non_rejected
            .iter()
            .filter(|block| block.status != BlockStatus::Approved)
            .count();
        if unapproved > 0 {
            warn!(
                meeting_id = %meeting_id,
                unapproved,
                reason_code = "unapproved_blocks",
                "approve_summary refused"
            );
            return Ok(false);
        }

        let required: HashSet<String> = non_rejected
            .iter()
            .map(|block| block.source_chunk_id.clone())
            .collect();
        let resolved = resolve_source_chunk_ids(pool, ctx, meeting_id, &required).await?;
        if resolved.len() != required.len() {
            warn!(
                meeting_id = %meeting_id,
                unresolvable = required.len() - resolved.len(),
                reason_code = "unresolvable_sources",
                "approve_summary refused: cited segment(s) no longer resolve"
            );
            return Ok(false);
        }

        let now = Utc::now();
        sqlx::query(
            "UPDATE summaries SET status = 'approved', approved_at = ?, approved_by = ?, \
             updated_at = ?, updated_by = ?, rev = rev + 1 WHERE id = ? AND workspace_id = ?",
        )
        .bind(now)
        .bind(ctx.user_id.as_str())
        .bind(now)
        .bind(ctx.user_id.as_str())
        .bind(&summary_id)
        .bind(ctx.tenant_id.as_str())
        .execute(pool)
        .await?;

        info!(
            meeting_id = %meeting_id,
            summary_id = %summary_id,
            "summary approved by human reviewer"
        );
        Ok(true)
    }

    /// ADR-0019 decision 2 (retranscription downgrade): drops the summary back
    /// to `draft` — every `approved`/`edited` block becomes `draft`
    /// (`original_content` KEPT for auditability), summary-level status becomes
    /// `draft`, `approved_at`/`approved_by` are cleared, `rev` bumps. Called by
    /// `TranscriptsRepository::replace_meeting_transcripts` after a successful
    /// replace so evidence links are re-reviewed against the changed segments.
    /// `Ok(false)` when the meeting has no (non-deleted) summary row in this
    /// workspace.
    pub async fn downgrade_to_draft(
        pool: &SqlitePool,
        ctx: &AuthContext,
        meeting_id: &str,
    ) -> Result<bool, SummaryDraftError> {
        let row = sqlx::query(
            "SELECT id, sections FROM summaries \
             WHERE meeting_id = ? AND workspace_id = ? AND deleted_at IS NULL",
        )
        .bind(meeting_id)
        .bind(ctx.tenant_id.as_str())
        .fetch_optional(pool)
        .await?;
        let Some(row) = row else {
            return Ok(false);
        };
        let summary_id: String = row.get("id");
        let mut sections: Vec<DraftSection> =
            serde_json::from_str(&row.get::<String, _>("sections"))?;

        for block in sections
            .iter_mut()
            .flat_map(|section| section.blocks.iter_mut())
        {
            if matches!(block.status, BlockStatus::Approved | BlockStatus::Edited) {
                block.status = BlockStatus::Draft;
            }
        }
        let sections_json = serde_json::to_string(&sections)?;

        // `None`: retranscription is the APP resetting its own bookkeeping, not a
        // human passing judgement. Recording it would teach the miner that the
        // user rejects work they never even saw (ADR-0024 §2).
        Self::write_sections_as_draft(pool, ctx, &summary_id, &sections_json, None).await?;
        info!(
            meeting_id = %meeting_id,
            summary_id = %summary_id,
            "summary downgraded to draft (evidence must be re-reviewed)"
        );
        Ok(true)
    }

    /// Soft-deletes the meeting's summary (sets `deleted_at`, bumps `rev`).
    /// `Ok(false)` when no live row exists in this workspace.
    pub async fn soft_delete(
        pool: &SqlitePool,
        ctx: &AuthContext,
        meeting_id: &str,
    ) -> Result<bool, SummaryDraftError> {
        let now = Utc::now();
        let result = sqlx::query(
            "UPDATE summaries SET deleted_at = ?, updated_at = ?, updated_by = ?, \
             rev = rev + 1 \
             WHERE meeting_id = ? AND workspace_id = ? AND deleted_at IS NULL",
        )
        .bind(now)
        .bind(now)
        .bind(ctx.user_id.as_str())
        .bind(meeting_id)
        .bind(ctx.tenant_id.as_str())
        .execute(pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Shared write-back for block-level mutations and the downgrade path:
    /// stores the new sections JSON and enforces the "any block mutation
    /// de-approves the summary" invariant (`status = 'draft'`, approval stamps
    /// cleared), bumping `rev` and stamping `updated_by`/`updated_at`.
    ///
    /// `event` is `Some` for a HUMAN correction and `None` for system-driven
    /// writes — notably the ADR-0019 retranscription downgrade, where nobody
    /// said anything and a recorded "correction" would teach the miner about the
    /// app's own bookkeeping (ADR-0024 §2).
    ///
    /// When present, the event is appended to `correction_events` in the SAME
    /// transaction as the sections write: both land or neither does, so the
    /// learning signal can never silently diverge from the state it describes.
    /// That is also why this method now opens a transaction at all — the sections
    /// UPDATE alone never needed one.
    async fn write_sections_as_draft(
        pool: &SqlitePool,
        ctx: &AuthContext,
        summary_id: &str,
        sections_json: &str,
        event: Option<&NewCorrectionEvent<'_>>,
    ) -> Result<(), SummaryDraftError> {
        let mut transaction = pool.begin().await?;

        sqlx::query(
            "UPDATE summaries SET sections = ?, status = 'draft', approved_at = NULL, \
             approved_by = NULL, updated_at = ?, updated_by = ?, rev = rev + 1 \
             WHERE id = ? AND workspace_id = ?",
        )
        .bind(sections_json)
        .bind(Utc::now())
        .bind(ctx.user_id.as_str())
        .bind(summary_id)
        .bind(ctx.tenant_id.as_str())
        .execute(&mut *transaction)
        .await?;

        if let Some(event) = event {
            CorrectionEventsRepository::append_tx(&mut transaction, ctx, event).await?;
        }

        transaction.commit().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Full 16-cell matrix of the §4 status machine — the legal arcs are
    /// exactly `draft→approved|edited|rejected`, `approved→edited|rejected`,
    /// `edited→approved|rejected`, `rejected→draft`; self-transitions and every
    /// other pair are illegal.
    #[test]
    fn block_transition_matrix() {
        use BlockStatus::{Approved, Draft, Edited, Rejected};
        let cases: [(BlockStatus, BlockStatus, bool); 16] = [
            (Draft, Draft, false),
            (Draft, Approved, true),
            (Draft, Edited, true),
            (Draft, Rejected, true),
            (Approved, Draft, false),
            (Approved, Approved, false),
            (Approved, Edited, true),
            (Approved, Rejected, true),
            (Edited, Draft, false),
            (Edited, Approved, true),
            (Edited, Edited, false),
            (Edited, Rejected, true),
            (Rejected, Draft, true),
            (Rejected, Approved, false),
            (Rejected, Edited, false),
            (Rejected, Rejected, false),
        ];
        for (from, to, expected) in cases {
            assert_eq!(
                block_transition_allowed(from, to),
                expected,
                "transition {from:?} -> {to:?} must be {}",
                if expected { "legal" } else { "illegal" }
            );
        }
    }

    /// Status tokens round-trip through the DB mapping and match the §4 wire
    /// vocabulary pinned by `summary::draft`; unknown tokens are typed errors
    /// that name the token (a status token is not user content).
    #[test]
    fn status_tokens_round_trip_and_reject_unknown() {
        for status in [
            BlockStatus::Draft,
            BlockStatus::Approved,
            BlockStatus::Edited,
            BlockStatus::Rejected,
        ] {
            let token = block_status_to_db(status);
            assert_eq!(
                block_status_from_db(token).expect("known token must parse"),
                status
            );
            // Must match the serde wire token exactly (drift guard).
            assert_eq!(
                serde_json::to_string(&status).expect("serialize"),
                format!("\"{token}\"")
            );
        }
        assert_eq!(
            summary_status_from_db("draft").expect("draft"),
            SummaryStatus::Draft
        );
        assert_eq!(
            summary_status_from_db("approved").expect("approved"),
            SummaryStatus::Approved
        );
        for parse in [
            block_status_from_db("APPROVED").expect_err("case-sensitive"),
            summary_status_from_db("pending").expect_err("unknown token"),
        ] {
            assert!(
                matches!(&parse, SummaryDraftError::InvalidStatus { .. }),
                "unknown tokens must map to InvalidStatus, got {parse:?}"
            );
        }
    }

    /// `into_sqlx` unwraps database errors verbatim and folds domain variants
    /// into `Protocol` with the content-free display message.
    #[test]
    fn into_sqlx_maps_variants() {
        let db = SummaryDraftError::Database(sqlx::Error::RowNotFound);
        assert!(matches!(db.into_sqlx(), sqlx::Error::RowNotFound));

        let domain = SummaryDraftError::UnresolvableSources { count: 3 };
        match domain.into_sqlx() {
            sqlx::Error::Protocol(message) => {
                assert!(message.contains('3'), "message must carry the count");
            }
            other => panic!("expected Protocol, got {other:?}"),
        }
    }
}
