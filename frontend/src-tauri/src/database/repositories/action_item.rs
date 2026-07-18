//! Tenant-scoped repository for the C1 `action_items` table (BACKLOG C1.3 —
//! docs/CONTRACTS.md §2 + §4, ADR-0019).
//!
//! One row per extracted [`ActionItemDraft`] (src/summary/draft.rs), many per
//! meeting, ordered by `position`. Same house rules and HITL invariants as the
//! summaries repository (see `summary_draft.rs` module docs): every statement
//! is scoped by `workspace_id = ctx.tenant_id`; writes bump `rev` and stamp
//! `updated_by`/`updated_at`; cross-workspace access degrades to `Ok(false)` /
//! a typed not-found error (Phase-2 → 404, ADR-0010); "resolvable" is defined
//! ONCE by [`resolve_source_chunk_ids`]; errors and logs carry ids/counts,
//! never item text. Reads exclude soft-deleted rows; deletion is soft
//! (sync-class table).

use crate::context::AuthContext;
use crate::database::repositories::correction_event::{
    CorrectionAction, CorrectionEventsRepository, CorrectionSubject, NewCorrectionEvent,
};
use crate::database::repositories::summary_draft::{
    block_status_from_db, block_status_to_db, block_transition_allowed, ensure_sources_resolve,
    SummaryDraftError,
};
use crate::summary::draft::{ActionItemDraft, BlockStatus};
use chrono::Utc;
use serde::Serialize;
use sqlx::{Row, SqlitePool};
use std::collections::HashSet;
use tracing::{info, warn};

/// Default and maximum page sizes for the read-only Approved Action Center.
pub const DEFAULT_ACTION_CENTER_PAGE_SIZE: u32 = 100;
pub const MAX_ACTION_CENTER_PAGE_SIZE: u32 = 200;

/// One persisted action item, hydrated from an `action_items` row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ActionItemRow {
    /// Item id (preserved from the generated draft — the identity the UI holds).
    pub id: String,
    /// Owning meeting.
    pub meeting_id: String,
    /// The action text as currently displayed (generated, or human-edited).
    pub text: String,
    /// Suggested owner, if any.
    pub assignee: Option<String>,
    /// Suggested due date (free text / ISO-8601), if any.
    pub due: Option<String>,
    /// HITL review status.
    pub status: BlockStatus,
    /// REQUIRED and IMMUTABLE evidence anchor (§4) — never touched by edits.
    pub source_chunk_id: String,
    /// Display order within the meeting (insertion index of its generation).
    pub position: i64,
    /// Pre-edit text, preserved on the FIRST human edit (row-level analogue of
    /// `DraftBlock::original_content`).
    pub original_text: Option<String>,
    /// Sync revision counter.
    pub rev: i64,
}

/// One human-approved action item with the active meeting/source metadata that
/// makes it safe to show in the cross-meeting Action Center.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApprovedActionItemRow {
    pub id: String,
    pub meeting_id: String,
    pub meeting_title: String,
    pub meeting_created_at: String,
    pub text: String,
    pub assignee: Option<String>,
    pub due: Option<String>,
    /// HITL review state, deliberately named separately from future work state.
    pub review_status: BlockStatus,
    pub source_chunk_id: String,
    pub source_timestamp: String,
    pub audio_start_time: Option<f64>,
}

/// Bounded Action Center page. `has_more`/`next_offset` prevent a hard cap from
/// silently hiding approved items.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApprovedActionItemsPage {
    pub items: Vec<ApprovedActionItemRow>,
    pub has_more: bool,
    pub next_offset: Option<u32>,
}

pub struct ActionItemsRepository;

impl ActionItemsRepository {
    /// Persists a generation batch of action items for `meeting_id` and
    /// returns how many were inserted (`items.len()`).
    ///
    /// Gates, in order (same as `SummariesRepository::upsert_draft`): the
    /// meeting must exist in the caller's workspace
    /// ([`SummaryDraftError::MeetingNotFound`]); every item's
    /// `source_chunk_id` must resolve ([`ensure_sources_resolve`], typed error
    /// naming the count).
    ///
    /// REGENERATION POLICY (v1, documented deviation from a blind replace):
    /// regeneration must not silently destroy human-touched items, so inside
    /// one transaction this soft-deletes ONLY the meeting's live items whose
    /// status is `draft` or `rejected` (never-reviewed or explicitly discarded
    /// machine output) and keeps `approved`/`edited` rows untouched, then
    /// inserts the new drafts with `position` = batch index. Kept human-touched
    /// rows retain their original `position`, so a later generation's positions
    /// may interleave with them; [`Self::list_by_meeting`] orders
    /// deterministically. Item ids are preserved from the drafts (generation
    /// mints fresh uuids per batch; re-inserting a previously used id is a
    /// primary-key error, not silent data loss).
    ///
    /// Incoming statuses are ignored and every generated item is persisted as
    /// `draft`. Nothing here can mint an approval (HITL: approval is only
    /// [`Self::set_status`] with an explicit human action).
    pub async fn insert_drafts(
        pool: &SqlitePool,
        ctx: &AuthContext,
        meeting_id: &str,
        items: &[ActionItemDraft],
    ) -> Result<usize, SummaryDraftError> {
        let meeting_exists: Option<(i64,)> = sqlx::query_as(
            "SELECT 1 FROM meetings \
             WHERE id = ? AND workspace_id = ? AND deleted_at IS NULL",
        )
        .bind(meeting_id)
        .bind(ctx.tenant_id.as_str())
        .fetch_optional(pool)
        .await?;
        if meeting_exists.is_none() {
            info!(
                meeting_id = %meeting_id,
                "insert_drafts: meeting not found in this workspace"
            );
            return Err(SummaryDraftError::MeetingNotFound);
        }

        let required: HashSet<String> = items
            .iter()
            .map(|item| item.source_chunk_id.clone())
            .collect();
        ensure_sources_resolve(pool, ctx, meeting_id, &required).await?;

        let now = Utc::now();
        let mut transaction = pool.begin().await?;

        let non_draft_inputs = items
            .iter()
            .filter(|item| item.status != BlockStatus::Draft)
            .count();
        if non_draft_inputs > 0 {
            warn!(
                meeting_id = %meeting_id,
                forced_to_draft = non_draft_inputs,
                "insert_drafts ignored non-draft generation status"
            );
        }

        // Regeneration keeps human-touched (approved/edited) rows; only
        // never-reviewed or rejected machine output is replaced (soft delete).
        sqlx::query(
            "UPDATE action_items SET deleted_at = ?, updated_at = ?, updated_by = ?, \
             rev = rev + 1 \
             WHERE meeting_id = ? AND workspace_id = ? AND deleted_at IS NULL \
               AND status IN ('draft', 'rejected')",
        )
        .bind(now)
        .bind(now)
        .bind(ctx.user_id.as_str())
        .bind(meeting_id)
        .bind(ctx.tenant_id.as_str())
        .execute(&mut *transaction)
        .await?;

        // The initial gates fail fast before opening a transaction. Re-check
        // after the UPDATE has upgraded this SQLite transaction to a writer so
        // retranscription/soft-delete cannot invalidate evidence between this
        // check and the inserts below.
        let meeting_still_active: Option<(i64,)> = sqlx::query_as(
            "SELECT 1 FROM meetings \
             WHERE id = ? AND workspace_id = ? AND deleted_at IS NULL",
        )
        .bind(meeting_id)
        .bind(ctx.tenant_id.as_str())
        .fetch_optional(&mut *transaction)
        .await?;
        if meeting_still_active.is_none() {
            warn!(
                meeting_id = %meeting_id,
                "insert_drafts refused after acquiring write transaction: meeting inactive"
            );
            return Err(SummaryDraftError::MeetingNotFound);
        }

        let mut unresolvable_after_lock = 0usize;
        for source_chunk_id in &required {
            let source_active: Option<(i64,)> = sqlx::query_as(
                "SELECT 1 FROM transcripts t \
                 INNER JOIN meetings m \
                    ON m.id = t.meeting_id AND m.workspace_id = t.workspace_id \
                 WHERE t.id = ? AND t.meeting_id = ? \
                   AND t.workspace_id = ? AND m.workspace_id = ? \
                   AND t.deleted_at IS NULL AND m.deleted_at IS NULL",
            )
            .bind(source_chunk_id)
            .bind(meeting_id)
            .bind(ctx.tenant_id.as_str())
            .bind(ctx.tenant_id.as_str())
            .fetch_optional(&mut *transaction)
            .await?;
            if source_active.is_none() {
                unresolvable_after_lock += 1;
            }
        }
        if unresolvable_after_lock > 0 {
            warn!(
                meeting_id = %meeting_id,
                unresolvable = unresolvable_after_lock,
                "insert_drafts refused after acquiring write transaction: source inactive"
            );
            return Err(SummaryDraftError::UnresolvableSources {
                count: unresolvable_after_lock,
            });
        }

        for (position, item) in items.iter().enumerate() {
            sqlx::query(
                "INSERT INTO action_items \
                 (id, meeting_id, workspace_id, text, assignee, due, status, source_chunk_id, \
                  position, created_at, updated_at, updated_by, rev) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 1)",
            )
            .bind(&item.id)
            .bind(meeting_id)
            .bind(ctx.tenant_id.as_str())
            .bind(&item.text)
            .bind(&item.assignee)
            .bind(&item.due)
            .bind(block_status_to_db(BlockStatus::Draft))
            .bind(&item.source_chunk_id)
            .bind(position as i64)
            .bind(now)
            .bind(now)
            .bind(ctx.user_id.as_str())
            .execute(&mut *transaction)
            .await?;
        }

        transaction.commit().await?;

        info!(
            meeting_id = %meeting_id,
            inserted = items.len(),
            "persisted action item drafts"
        );
        Ok(items.len())
    }

    /// Lists the meeting's live action items (workspace-scoped, excludes
    /// soft-deleted), ordered by `position` — with `created_at`/`id` as
    /// deterministic tie-breakers, since human-kept rows from an earlier
    /// generation retain their original positions (see
    /// [`Self::insert_drafts`]).
    pub async fn list_by_meeting(
        pool: &SqlitePool,
        ctx: &AuthContext,
        meeting_id: &str,
    ) -> Result<Vec<ActionItemRow>, SummaryDraftError> {
        let rows = sqlx::query(
            "SELECT id, meeting_id, text, assignee, due, status, source_chunk_id, position, \
             original_text, rev FROM action_items \
             WHERE meeting_id = ? AND workspace_id = ? AND deleted_at IS NULL \
             ORDER BY position ASC, created_at ASC, id ASC",
        )
        .bind(meeting_id)
        .bind(ctx.tenant_id.as_str())
        .fetch_all(pool)
        .await?;

        rows.into_iter()
            .map(|row| {
                Ok(ActionItemRow {
                    id: row.get("id"),
                    meeting_id: row.get("meeting_id"),
                    text: row.get("text"),
                    assignee: row.get("assignee"),
                    due: row.get("due"),
                    status: block_status_from_db(&row.get::<String, _>("status"))?,
                    source_chunk_id: row.get("source_chunk_id"),
                    position: row.get("position"),
                    original_text: row.get("original_text"),
                    rev: row.get("rev"),
                })
            })
            .collect()
    }

    /// Cross-meeting "open" action items for the Home dashboard (Phase C):
    /// every non-rejected, non-deleted item in the workspace, newest meetings
    /// first, joined with the meeting title so the UI can attribute each item.
    /// Tenant-scoped on BOTH tables (the join predicate carries workspace_id,
    /// the C3 lesson) and capped by `limit`.
    pub async fn list_open(
        pool: &SqlitePool,
        ctx: &AuthContext,
        limit: i64,
    ) -> Result<Vec<(ActionItemRow, String)>, SummaryDraftError> {
        let rows = sqlx::query(
            "SELECT a.id, a.meeting_id, a.text, a.assignee, a.due, a.status, \
             a.source_chunk_id, a.position, a.original_text, a.rev, \
             m.title AS meeting_title \
             FROM action_items a \
             JOIN meetings m ON m.id = a.meeting_id AND m.workspace_id = a.workspace_id \
             WHERE a.workspace_id = ? AND a.deleted_at IS NULL AND a.status != 'rejected' \
             ORDER BY a.created_at DESC, a.position ASC, a.id ASC \
             LIMIT ?",
        )
        .bind(ctx.tenant_id.as_str())
        .bind(limit)
        .fetch_all(pool)
        .await?;

        rows.into_iter()
            .map(|row| {
                Ok((
                    ActionItemRow {
                        id: row.get("id"),
                        meeting_id: row.get("meeting_id"),
                        text: row.get("text"),
                        assignee: row.get("assignee"),
                        due: row.get("due"),
                        status: block_status_from_db(&row.get::<String, _>("status"))?,
                        source_chunk_id: row.get("source_chunk_id"),
                        position: row.get("position"),
                        original_text: row.get("original_text"),
                        rev: row.get("rev"),
                    },
                    row.get("meeting_title"),
                ))
            })
            .collect()
    }

    /// Lists only explicitly human-approved items whose meeting and evidence
    /// anchor are still active in the caller's workspace.
    ///
    /// `edited` is deliberately excluded: it is a human-touched draft awaiting
    /// re-approval, not an approved work item. The existing `status` column is
    /// the HITL review axis, not a work-progress axis. Ordering is deterministic
    /// and does not interpret the free-text `due` field as a date.
    pub async fn list_approved_with_sources(
        pool: &SqlitePool,
        ctx: &AuthContext,
        limit: u32,
        offset: u32,
    ) -> Result<ApprovedActionItemsPage, SummaryDraftError> {
        let page_size = limit.clamp(1, MAX_ACTION_CENTER_PAGE_SIZE);
        let fetch_limit = i64::from(page_size) + 1;

        let rows = sqlx::query(
            "SELECT a.id, a.meeting_id, m.title AS meeting_title, \
                    m.created_at AS meeting_created_at, a.text, a.assignee, a.due, \
                    a.source_chunk_id, t.timestamp AS source_timestamp, t.audio_start_time \
             FROM action_items a \
             INNER JOIN meetings m \
                ON m.id = a.meeting_id AND m.workspace_id = a.workspace_id \
             INNER JOIN transcripts t \
                ON t.id = a.source_chunk_id \
               AND t.meeting_id = a.meeting_id \
               AND t.workspace_id = a.workspace_id \
             WHERE a.workspace_id = ? AND m.workspace_id = ? AND t.workspace_id = ? \
               AND a.status = 'approved' \
               AND a.deleted_at IS NULL \
               AND m.deleted_at IS NULL \
               AND t.deleted_at IS NULL \
             ORDER BY m.created_at DESC, a.position ASC, a.id ASC \
             LIMIT ? OFFSET ?",
        )
        .bind(ctx.tenant_id.as_str())
        .bind(ctx.tenant_id.as_str())
        .bind(ctx.tenant_id.as_str())
        .bind(fetch_limit)
        .bind(i64::from(offset))
        .fetch_all(pool)
        .await?;

        let has_more = rows.len() > page_size as usize;
        let items = rows
            .into_iter()
            .take(page_size as usize)
            .map(|row| ApprovedActionItemRow {
                id: row.get("id"),
                meeting_id: row.get("meeting_id"),
                meeting_title: row.get("meeting_title"),
                meeting_created_at: row.get("meeting_created_at"),
                text: row.get("text"),
                assignee: row.get("assignee"),
                due: row.get("due"),
                review_status: BlockStatus::Approved,
                source_chunk_id: row.get("source_chunk_id"),
                source_timestamp: row.get("source_timestamp"),
                audio_start_time: row.get("audio_start_time"),
            })
            .collect();

        Ok(ApprovedActionItemsPage {
            items,
            has_more,
            next_offset: has_more.then(|| offset.saturating_add(page_size)),
        })
    }

    /// Applies the shared status machine ([`block_transition_allowed`]) to one
    /// item. Approval uses one compare-and-swap UPDATE whose `EXISTS` predicate
    /// re-validates an active same-meeting source at write time (ADR-0019).
    /// `Ok(false)` when the item is not visible in this workspace, the
    /// transition is illegal, the row changed concurrently, or approve-time
    /// evidence no longer resolves.
    ///
    /// Every human verdict also appends ONE `correction_events` row inside the
    /// SAME transaction as the compare-and-swap write (ADR-0030 §2): its
    /// `original_text` is the MODEL's text (`original_text` once an edit has
    /// happened, else the current text), never a prior human revision. `reason`
    /// is the human's optional rationale, recorded on the event (ADR-0030 §3) and
    /// never a gate — see
    /// [`SummariesRepository::set_block_status`](super::summary_draft::SummariesRepository::set_block_status).
    /// `model`/`template_id` come from the meeting's `summaries` row via LEFT
    /// JOIN: action items carry no generation context of their own, but they are
    /// produced by the same `run_structured_generation` pass, and the miner needs
    /// it to scope a rule to a template. The join is LEFT so a soft-deleted or
    /// absent summary must not hide the item.
    pub async fn set_status(
        pool: &SqlitePool,
        ctx: &AuthContext,
        item_id: &str,
        new: BlockStatus,
        reason: Option<&str>,
    ) -> Result<bool, SummaryDraftError> {
        let row = sqlx::query(
            "SELECT ai.meeting_id AS meeting_id, ai.status AS status, ai.rev AS rev, \
                    ai.source_chunk_id AS source_chunk_id, ai.text AS text, \
                    ai.original_text AS original_text, \
                    s.model AS model, s.template_id AS template_id \
             FROM action_items ai \
             LEFT JOIN summaries s \
                    ON s.meeting_id = ai.meeting_id AND s.workspace_id = ai.workspace_id \
             WHERE ai.id = ? AND ai.workspace_id = ? AND ai.deleted_at IS NULL",
        )
        .bind(item_id)
        .bind(ctx.tenant_id.as_str())
        .fetch_optional(pool)
        .await?;
        let Some(row) = row else {
            info!(
                item_id = %item_id,
                reason_code = "item_not_found",
                "set_status refused"
            );
            return Ok(false);
        };
        let current = block_status_from_db(&row.get::<String, _>("status"))?;
        let current_rev: i64 = row.get("rev");
        if !block_transition_allowed(current, new) {
            info!(
                item_id = %item_id,
                from = block_status_to_db(current),
                to = block_status_to_db(new),
                reason_code = "illegal_transition",
                "set_status refused"
            );
            return Ok(false);
        }

        let meeting_id: String = row.get("meeting_id");
        let source_chunk_id: String = row.get("source_chunk_id");

        // What the MODEL wrote: `original_text` once an edit has happened,
        // otherwise the text itself (ADR-0030 §2 — always model→human).
        let current_text: String = row.get("text");
        let original_text: Option<String> = row.get("original_text");
        let model_text = original_text.unwrap_or_else(|| current_text.clone());
        let model: Option<String> = row.get("model");
        let template_id: Option<String> = row.get("template_id");

        let action = match new {
            BlockStatus::Approved => CorrectionAction::Approve,
            BlockStatus::Rejected => CorrectionAction::Reject,
            BlockStatus::Draft => CorrectionAction::Restore,
            BlockStatus::Edited => CorrectionAction::Edit,
        };
        let event = NewCorrectionEvent {
            meeting_id: &meeting_id,
            subject_kind: CorrectionSubject::ActionItem,
            subject_id: item_id,
            action,
            original_text: Some(model_text.as_str()),
            final_text: match new {
                BlockStatus::Approved | BlockStatus::Edited => Some(current_text.as_str()),
                BlockStatus::Rejected | BlockStatus::Draft => None,
            },
            reason,
            // Action items have neither a §4 block kind nor a section.
            block_type: None,
            section_title: None,
            template_id: template_id.as_deref(),
            model: model.as_deref(),
            source_chunk_id: Some(source_chunk_id.as_str()),
        };

        // ADR-0030 §7: record a correction only when the workspace has learning
        // ON. Off suppresses the `correction_events` row, not the status change.
        // Read before the transaction opens so the append below can be gated.
        let capture =
            crate::database::repositories::setting::SettingsRepository::learning_capture_enabled(
                pool, ctx,
            )
            .await;

        // One transaction: theirs' compare-and-swap verdict and ours' record of it
        // land together, or not at all (ADR-0030 §2). Approval keeps theirs'
        // `EXISTS` predicate, which re-validates an active same-meeting source at
        // write time (ADR-0019) instead of a separate pre-check.
        let mut transaction = pool.begin().await?;
        let result = if new == BlockStatus::Approved {
            sqlx::query(
                "UPDATE action_items SET status = ?, updated_at = ?, updated_by = ?, \
                 rev = rev + 1 \
                 WHERE id = ? AND workspace_id = ? AND deleted_at IS NULL \
                   AND status = ? AND rev = ? \
                   AND EXISTS ( \
                       SELECT 1 FROM transcripts t \
                       INNER JOIN meetings m \
                          ON m.id = t.meeting_id AND m.workspace_id = t.workspace_id \
                       WHERE t.id = action_items.source_chunk_id \
                         AND t.meeting_id = action_items.meeting_id \
                         AND t.workspace_id = action_items.workspace_id \
                         AND t.workspace_id = ? AND m.workspace_id = ? \
                         AND t.deleted_at IS NULL AND m.deleted_at IS NULL \
                   )",
            )
            .bind(block_status_to_db(new))
            .bind(Utc::now())
            .bind(ctx.user_id.as_str())
            .bind(item_id)
            .bind(ctx.tenant_id.as_str())
            .bind(block_status_to_db(current))
            .bind(current_rev)
            .bind(ctx.tenant_id.as_str())
            .bind(ctx.tenant_id.as_str())
            .execute(&mut *transaction)
            .await?
        } else {
            sqlx::query(
                "UPDATE action_items SET status = ?, updated_at = ?, updated_by = ?, \
                 rev = rev + 1 \
                 WHERE id = ? AND workspace_id = ? AND deleted_at IS NULL \
                   AND status = ? AND rev = ?",
            )
            .bind(block_status_to_db(new))
            .bind(Utc::now())
            .bind(ctx.user_id.as_str())
            .bind(item_id)
            .bind(ctx.tenant_id.as_str())
            .bind(block_status_to_db(current))
            .bind(current_rev)
            .execute(&mut *transaction)
            .await?
        };

        if result.rows_affected() != 1 {
            warn!(
                item_id = %item_id,
                reason_code = "concurrent_change_or_unresolvable_source",
                "set_status refused at atomic write gate"
            );
            return Ok(false);
        }

        // The verdict landed in this transaction; record it in the same one (when
        // learning is on) so the signal can never diverge from the state it
        // describes.
        if capture {
            CorrectionEventsRepository::append_tx(&mut transaction, ctx, &event).await?;
        }
        transaction.commit().await?;
        Ok(true)
    }

    /// Human edit of one item — patch semantics: `text = Some(new)` replaces
    /// the text; `assignee`/`due` use `None` = leave unchanged,
    /// `Some(None)` = clear, `Some(Some(v))` = set. NEVER touches
    /// `source_chunk_id` (immutable evidence anchor, §4). On the first text
    /// edit the pre-edit text is preserved in `original_text`; the status
    /// becomes [`BlockStatus::Edited`].
    ///
    /// `Ok(false)` when: nothing to change (all three patches are `None`), the
    /// item is not visible in this workspace, the item is rejected (no
    /// `rejected → edited` arc — restore to draft first), or the row changed
    /// concurrently before the compare-and-swap write.
    pub async fn edit(
        pool: &SqlitePool,
        ctx: &AuthContext,
        item_id: &str,
        text: Option<&str>,
        assignee: Option<Option<&str>>,
        due: Option<Option<&str>>,
    ) -> Result<bool, SummaryDraftError> {
        if text.is_none() && assignee.is_none() && due.is_none() {
            info!(
                item_id = %item_id,
                reason_code = "empty_edit",
                "edit refused: no fields to change"
            );
            return Ok(false);
        }

        let row = sqlx::query(
            "SELECT ai.meeting_id AS meeting_id, ai.text AS text, ai.assignee AS assignee, \
                    ai.due AS due, ai.status AS status, ai.original_text AS original_text, \
                    ai.rev AS rev, ai.source_chunk_id AS source_chunk_id, \
                    s.model AS model, s.template_id AS template_id \
             FROM action_items ai \
             LEFT JOIN summaries s \
                    ON s.meeting_id = ai.meeting_id AND s.workspace_id = ai.workspace_id \
             WHERE ai.id = ? AND ai.workspace_id = ? AND ai.deleted_at IS NULL",
        )
        .bind(item_id)
        .bind(ctx.tenant_id.as_str())
        .fetch_optional(pool)
        .await?;
        let Some(row) = row else {
            info!(
                item_id = %item_id,
                reason_code = "item_not_found",
                "edit refused"
            );
            return Ok(false);
        };

        let current = block_status_from_db(&row.get::<String, _>("status"))?;
        let current_rev: i64 = row.get("rev");
        if current == BlockStatus::Rejected {
            info!(
                item_id = %item_id,
                reason_code = "illegal_transition",
                "edit refused: rejected items must be restored to draft first"
            );
            return Ok(false);
        }

        let current_text: String = row.get("text");
        let original_text: Option<String> = row.get("original_text");
        // What the MODEL wrote — captured before `original_text` is consumed
        // below (ADR-0030 §2: the delta is always model→human, never
        // human→human, or a re-edit would understate the burden).
        let model_text = original_text
            .clone()
            .unwrap_or_else(|| current_text.clone());
        let new_original = if text.is_some() && original_text.is_none() {
            Some(current_text.clone())
        } else {
            original_text
        };
        let new_text = text.map_or_else(|| current_text.clone(), str::to_string);
        let new_assignee: Option<String> = match assignee {
            None => row.get("assignee"),
            Some(patch) => patch.map(str::to_string),
        };
        let new_due: Option<String> = match due {
            None => row.get("due"),
            Some(patch) => patch.map(str::to_string),
        };

        let meeting_id: String = row.get("meeting_id");
        let source_chunk_id: Option<String> = row.get("source_chunk_id");
        let model: Option<String> = row.get("model");
        let template_id: Option<String> = row.get("template_id");

        // ONLY a text change is a language correction, which is what the miner
        // learns from. An assignee/due-only patch is a genuine correction of the
        // model's extraction, but it produces no model→human TEXT delta: recorded
        // as an edit it would enter the burden metric (E1) as a zero-cost one and
        // dilute it. Capturing structured-field corrections needs its own event
        // shape — owed, and recorded in ADR-0030 rather than faked here.
        let event = (new_text != current_text).then(|| NewCorrectionEvent {
            meeting_id: &meeting_id,
            subject_kind: CorrectionSubject::ActionItem,
            subject_id: item_id,
            action: CorrectionAction::Edit,
            original_text: Some(model_text.as_str()),
            final_text: Some(new_text.as_str()),
            reason: None,
            block_type: None,
            section_title: None,
            template_id: template_id.as_deref(),
            model: model.as_deref(),
            source_chunk_id: source_chunk_id.as_deref(),
        });

        // ADR-0030 §7: only record when learning is ON (off suppresses capture,
        // not the edit). Read before the transaction so the append can be gated.
        let capture =
            crate::database::repositories::setting::SettingsRepository::learning_capture_enabled(
                pool, ctx,
            )
            .await;

        // One transaction: theirs' compare-and-swap edit and ours' correction
        // record (only for a real text change) land together (ADR-0030 §2).
        let mut transaction = pool.begin().await?;
        let result = sqlx::query(
            "UPDATE action_items SET text = ?, assignee = ?, due = ?, original_text = ?, \
             status = 'edited', updated_at = ?, updated_by = ?, rev = rev + 1 \
             WHERE id = ? AND workspace_id = ? AND deleted_at IS NULL \
               AND status = ? AND rev = ?",
        )
        .bind(&new_text)
        .bind(&new_assignee)
        .bind(&new_due)
        .bind(&new_original)
        .bind(Utc::now())
        .bind(ctx.user_id.as_str())
        .bind(item_id)
        .bind(ctx.tenant_id.as_str())
        .bind(block_status_to_db(current))
        .bind(current_rev)
        .execute(&mut *transaction)
        .await?;
        if result.rows_affected() != 1 {
            warn!(
                item_id = %item_id,
                reason_code = "concurrent_change",
                "edit refused at atomic write gate"
            );
            return Ok(false);
        }
        if capture {
            if let Some(event) = &event {
                CorrectionEventsRepository::append_tx(&mut transaction, ctx, event).await?;
            }
        }
        transaction.commit().await?;
        Ok(true)
    }

    /// ADR-0019 decision 2 (retranscription downgrade): every live
    /// `approved`/`edited` item of the meeting drops back to `draft`
    /// (`original_text` KEPT), `rev` bumps. Rejected and draft items are left
    /// untouched. `Ok(false)` when no item needed downgrading.
    pub async fn downgrade_to_draft(
        pool: &SqlitePool,
        ctx: &AuthContext,
        meeting_id: &str,
    ) -> Result<bool, SummaryDraftError> {
        let result = sqlx::query(
            "UPDATE action_items SET status = 'draft', updated_at = ?, updated_by = ?, \
             rev = rev + 1 \
             WHERE meeting_id = ? AND workspace_id = ? AND deleted_at IS NULL \
               AND status IN ('approved', 'edited')",
        )
        .bind(Utc::now())
        .bind(ctx.user_id.as_str())
        .bind(meeting_id)
        .bind(ctx.tenant_id.as_str())
        .execute(pool)
        .await?;
        let downgraded = result.rows_affected();
        if downgraded > 0 {
            info!(
                meeting_id = %meeting_id,
                downgraded,
                "action items downgraded to draft (evidence must be re-reviewed)"
            );
        }
        Ok(downgraded > 0)
    }

    /// Soft-deletes one action item (sets `deleted_at`, bumps `rev`).
    /// `Ok(false)` when no live row with this id exists in this workspace.
    pub async fn soft_delete(
        pool: &SqlitePool,
        ctx: &AuthContext,
        item_id: &str,
    ) -> Result<bool, SummaryDraftError> {
        let now = Utc::now();
        let result = sqlx::query(
            "UPDATE action_items SET deleted_at = ?, updated_at = ?, updated_by = ?, \
             rev = rev + 1 WHERE id = ? AND workspace_id = ? AND deleted_at IS NULL",
        )
        .bind(now)
        .bind(now)
        .bind(ctx.user_id.as_str())
        .bind(item_id)
        .bind(ctx.tenant_id.as_str())
        .execute(pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }
}
