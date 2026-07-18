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
    resolve_source_chunk_ids, SummaryDraftError,
};
use crate::summary::draft::{ActionItemDraft, BlockStatus};
use chrono::Utc;
use serde::Serialize;
use sqlx::{Row, SqlitePool};
use std::collections::HashSet;
use tracing::{info, warn};

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
    /// Statuses are persisted exactly as given — the §4 wire default is
    /// `draft`, and nothing here can mint an approval (HITL: approval is only
    /// [`Self::set_status`] with an explicit human action).
    pub async fn insert_drafts(
        pool: &SqlitePool,
        ctx: &AuthContext,
        meeting_id: &str,
        items: &[ActionItemDraft],
    ) -> Result<usize, SummaryDraftError> {
        let meeting_exists: Option<(i64,)> =
            sqlx::query_as("SELECT 1 FROM meetings WHERE id = ? AND workspace_id = ?")
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
            .bind(block_status_to_db(item.status))
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

    /// Applies the shared status machine ([`block_transition_allowed`]) to one
    /// item. Approving RE-validates that the item's `source_chunk_id` resolves
    /// NOW ([`resolve_source_chunk_ids`] — ADR-0019). `Ok(false)` (with a
    /// content-free `tracing` reason) when the item is not visible in this
    /// workspace, the transition is illegal, or approve-time evidence no
    /// longer resolves.
    /// `reason` is the human's optional rationale, recorded on the correction
    /// event (ADR-0024 §3) and never a gate — see
    /// [`SummariesRepository::set_block_status`](super::summary_draft::SummariesRepository::set_block_status).
    ///
    /// `model`/`template_id` come from the meeting's `summaries` row via LEFT
    /// JOIN: action items carry no generation context of their own, but they are
    /// produced by the same `run_structured_generation` pass as the summary, so
    /// that row IS their context — and the miner needs it to scope a rule to a
    /// template instead of attributing one model's habits to another. The join is
    /// LEFT because a soft-deleted or absent summary must not hide the item.
    pub async fn set_status(
        pool: &SqlitePool,
        ctx: &AuthContext,
        item_id: &str,
        new: BlockStatus,
        reason: Option<&str>,
    ) -> Result<bool, SummaryDraftError> {
        let row = sqlx::query(
            "SELECT ai.meeting_id AS meeting_id, ai.status AS status, \
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

        if new == BlockStatus::Approved {
            let required = HashSet::from([source_chunk_id.clone()]);
            let resolved = resolve_source_chunk_ids(pool, ctx, &meeting_id, &required).await?;
            if resolved.len() != required.len() {
                warn!(
                    item_id = %item_id,
                    meeting_id = %meeting_id,
                    reason_code = "unresolvable_source",
                    "set_status refused: cited segment no longer resolves"
                );
                return Ok(false);
            }
        }

        // What the MODEL wrote: `original_text` once an edit has happened,
        // otherwise the text itself (ADR-0024 §2 — always model→human).
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

        // One transaction: the verdict and the record of it land together, or not
        // at all (ADR-0024 §2).
        let mut transaction = pool.begin().await?;
        sqlx::query(
            "UPDATE action_items SET status = ?, updated_at = ?, updated_by = ?, \
             rev = rev + 1 WHERE id = ? AND workspace_id = ?",
        )
        .bind(block_status_to_db(new))
        .bind(Utc::now())
        .bind(ctx.user_id.as_str())
        .bind(item_id)
        .bind(ctx.tenant_id.as_str())
        .execute(&mut *transaction)
        .await?;
        CorrectionEventsRepository::append_tx(&mut transaction, ctx, &event).await?;
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
    /// item is not visible in this workspace, or the item is rejected (no
    /// `rejected → edited` arc — restore to draft first).
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
                    ai.source_chunk_id AS source_chunk_id, \
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
        // below (ADR-0024 §2: the delta is always model→human, never
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
        // shape — owed, and recorded in ADR-0024 rather than faked here.
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

        let mut transaction = pool.begin().await?;
        sqlx::query(
            "UPDATE action_items SET text = ?, assignee = ?, due = ?, original_text = ?, \
             status = 'edited', updated_at = ?, updated_by = ?, rev = rev + 1 \
             WHERE id = ? AND workspace_id = ?",
        )
        .bind(&new_text)
        .bind(&new_assignee)
        .bind(&new_due)
        .bind(&new_original)
        .bind(Utc::now())
        .bind(ctx.user_id.as_str())
        .bind(item_id)
        .bind(ctx.tenant_id.as_str())
        .execute(&mut *transaction)
        .await?;
        if let Some(event) = &event {
            CorrectionEventsRepository::append_tx(&mut transaction, ctx, event).await?;
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
