//! Tenant-scoped `speaker_turns` storage (ADR-0034).
//!
//! Every statement scopes on `workspace_id = ctx.tenant_id`, like every other
//! repository here (`docs/CONTRACTS.md` §2).
//!
//! Turns are LOCAL-DERIVED and not synced: ADR-0012 pins the synced entity set,
//! and a peer can regenerate turns from audio. So the table carries no
//! `rev`/`updated_by`/`deleted_at`, and neither does this module.

use anyhow::{bail, Result};
use chrono::Utc;
use sqlx::{Row, SqlitePool};

use crate::context::AuthContext;

/// One anonymous speaker turn, in the units the schema stores.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SpeakerTurn {
    pub start_ms: i64,
    pub end_ms: i64,
    pub speaker_label: String,
    pub confidence: Option<f64>,
}

pub struct SpeakerTurnsRepository;

impl SpeakerTurnsRepository {
    /// Replace this meeting's turns and stamp `meetings.diarized_at`, in ONE
    /// transaction.
    ///
    /// Replace rather than append: a second diarization pass re-labels the whole
    /// recording, and leaving the previous pass behind would show one stretch of
    /// audio attributed to two different speakers at once.
    ///
    /// The stamp is written even when `turns` is empty, and that is the point:
    /// an empty turn list must mean "ran and found nothing separable", which is
    /// a different statement from "never ran" (the migration's own note). Only
    /// `diarized_at` can tell those apart.
    pub async fn replace_for_meeting(
        pool: &SqlitePool,
        ctx: &AuthContext,
        meeting_id: &str,
        turns: &[SpeakerTurn],
    ) -> Result<usize> {
        if meeting_id.trim().is_empty() {
            bail!("meeting_id cannot be empty");
        }
        for t in turns {
            if t.end_ms <= t.start_ms {
                bail!(
                    "refusing to store a turn that ends before it starts ({} -> {})",
                    t.start_ms,
                    t.end_ms
                );
            }
            if t.speaker_label.trim().is_empty() {
                bail!("refusing to store a turn with no speaker label");
            }
        }

        let now = Utc::now().to_rfc3339();
        let mut tx = pool.begin().await?;

        // Scoped to the caller's workspace: a meeting id from another tenant
        // must match nothing rather than delete anything.
        let owned: Option<String> =
            sqlx::query_scalar("SELECT id FROM meetings WHERE id = ? AND workspace_id = ?")
                .bind(meeting_id)
                .bind(ctx.tenant_id.as_str())
                .fetch_optional(&mut *tx)
                .await?;
        if owned.is_none() {
            tx.rollback().await?;
            bail!("meeting {meeting_id} is not in this workspace");
        }

        sqlx::query("DELETE FROM speaker_turns WHERE meeting_id = ? AND workspace_id = ?")
            .bind(meeting_id)
            .bind(ctx.tenant_id.as_str())
            .execute(&mut *tx)
            .await?;

        for t in turns {
            sqlx::query(
                "INSERT INTO speaker_turns \
                 (id, meeting_id, workspace_id, speaker_label, start_ms, end_ms, confidence, \
                  created_at, updated_at) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(uuid::Uuid::new_v4().to_string())
            .bind(meeting_id)
            .bind(ctx.tenant_id.as_str())
            .bind(&t.speaker_label)
            .bind(t.start_ms)
            .bind(t.end_ms)
            .bind(t.confidence)
            .bind(&now)
            .bind(&now)
            .execute(&mut *tx)
            .await?;
        }

        // No `rev` bump: `meetings` is synced, and marking every diarized
        // meeting as freshly modified would make a sync peer re-pull it for a
        // field that is local-derived anyway.
        sqlx::query("UPDATE meetings SET diarized_at = ? WHERE id = ? AND workspace_id = ?")
            .bind(&now)
            .bind(meeting_id)
            .bind(ctx.tenant_id.as_str())
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;
        Ok(turns.len())
    }

    /// This meeting's turns, earliest first.
    pub async fn list_for_meeting(
        pool: &SqlitePool,
        ctx: &AuthContext,
        meeting_id: &str,
    ) -> Result<Vec<SpeakerTurn>> {
        let rows = sqlx::query(
            "SELECT speaker_label, start_ms, end_ms, confidence FROM speaker_turns \
             WHERE meeting_id = ? AND workspace_id = ? ORDER BY start_ms, end_ms",
        )
        .bind(meeting_id)
        .bind(ctx.tenant_id.as_str())
        .fetch_all(pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| SpeakerTurn {
                speaker_label: r.get("speaker_label"),
                start_ms: r.get("start_ms"),
                end_ms: r.get("end_ms"),
                confidence: r.get("confidence"),
            })
            .collect())
    }

    /// When a diarization pass last completed, or `None` if none ever has.
    ///
    /// `None` is NOT "no speakers found" — see `replace_for_meeting`.
    pub async fn diarized_at(
        pool: &SqlitePool,
        ctx: &AuthContext,
        meeting_id: &str,
    ) -> Result<Option<String>> {
        let value: Option<Option<String>> = sqlx::query_scalar(
            "SELECT diarized_at FROM meetings WHERE id = ? AND workspace_id = ?",
        )
        .bind(meeting_id)
        .bind(ctx.tenant_id.as_str())
        .fetch_optional(pool)
        .await?;
        Ok(value.flatten())
    }
}
