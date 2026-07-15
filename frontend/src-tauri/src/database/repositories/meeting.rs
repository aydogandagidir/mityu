//! Tenant-scoped meetings repository (docs/CONTRACTS.md §2, BACKLOG B2 phase 2).
//!
//! Every method takes [`AuthContext`] and scopes each statement with
//! `workspace_id = ctx.tenant_id`; writes to this synced table also bump `rev`
//! and stamp `updated_by`/`updated_at` (docs/DATA_MODEL.md common columns).

use crate::api::{MeetingDetails, MeetingTranscript};
use crate::context::AuthContext;
use crate::database::models::{MeetingModel, Transcript};
use chrono::Utc;
use sqlx::{Connection, Error as SqlxError, SqliteConnection, SqlitePool};
use tracing::info;

pub struct MeetingsRepository;

impl MeetingsRepository {
    /// Return whether `folder_path` is referenced by another active meeting in
    /// the caller's workspace. Cross-workspace collision safety is enforced by
    /// the recording folder's trusted `metadata.json.workspace_id` marker; this
    /// repository never reads a foreign workspace, even for an existence check.
    pub async fn recording_folder_has_other_same_workspace_reference(
        pool: &SqlitePool,
        ctx: &AuthContext,
        meeting_id: &str,
        folder_path: &str,
    ) -> Result<bool, SqlxError> {
        let exists: i64 = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM meetings \
             WHERE workspace_id = ? AND folder_path = ? AND deleted_at IS NULL \
             AND id <> ?)",
        )
        .bind(ctx.tenant_id.as_str())
        .bind(folder_path)
        .bind(meeting_id)
        .fetch_one(pool)
        .await?;
        Ok(exists == 1)
    }

    pub async fn get_meetings(
        pool: &SqlitePool,
        ctx: &AuthContext,
    ) -> Result<Vec<MeetingModel>, sqlx::Error> {
        let meetings = sqlx::query_as::<_, MeetingModel>(
            "SELECT * FROM meetings WHERE workspace_id = ? ORDER BY created_at DESC",
        )
        .bind(ctx.tenant_id.as_str())
        .fetch_all(pool)
        .await?;
        Ok(meetings)
    }

    pub async fn delete_meeting(
        pool: &SqlitePool,
        ctx: &AuthContext,
        meeting_id: &str,
    ) -> Result<bool, SqlxError> {
        let deleted = crate::database::deletion::delete_database_records(pool, ctx, meeting_id)
            .await
            .map_err(|error| SqlxError::Protocol(format!("verified deletion failed: {error:#}")))?;
        if deleted {
            crate::database::deletion::complete_privacy_maintenance(pool)
                .await
                .map_err(|error| {
                    SqlxError::Protocol(format!("privacy maintenance remains pending: {error:#}"))
                })?;
            info!("Meeting records deleted and local privacy maintenance verified");
        }
        Ok(deleted)
    }

    pub async fn get_meeting(
        pool: &SqlitePool,
        ctx: &AuthContext,
        meeting_id: &str,
    ) -> Result<Option<MeetingDetails>, SqlxError> {
        if meeting_id.trim().is_empty() {
            return Err(SqlxError::Protocol(
                "meeting_id cannot be empty".to_string(),
            ));
        }

        let mut conn = pool.acquire().await?;
        let mut transaction = conn.begin().await?;

        // Get meeting details
        let meeting: Option<MeetingModel> = sqlx::query_as(
            "SELECT id, title, created_at, updated_at, folder_path FROM meetings \
             WHERE id = ? AND workspace_id = ?",
        )
        .bind(meeting_id)
        .bind(ctx.tenant_id.as_str())
        .fetch_optional(&mut *transaction)
        .await?;

        if meeting.is_none() {
            transaction.rollback().await?;
            return Err(SqlxError::RowNotFound);
        }

        if let Some(meeting) = meeting {
            // Get all transcripts for this meeting
            let transcripts = sqlx::query_as::<_, Transcript>(
                "SELECT * FROM transcripts WHERE meeting_id = ? AND workspace_id = ?",
            )
            .bind(meeting_id)
            .bind(ctx.tenant_id.as_str())
            .fetch_all(&mut *transaction)
            .await?;

            transaction.commit().await?;

            // Convert Transcript to MeetingTranscript
            let meeting_transcripts = transcripts
                .into_iter()
                .map(|t| MeetingTranscript {
                    id: t.id,
                    text: t.transcript,
                    timestamp: t.timestamp,
                    audio_start_time: t.audio_start_time,
                    audio_end_time: t.audio_end_time,
                    duration: t.duration,
                })
                .collect::<Vec<_>>();

            Ok(Some(MeetingDetails {
                id: meeting.id,
                title: meeting.title,
                created_at: meeting.created_at.0.to_rfc3339(),
                updated_at: meeting.updated_at.0.to_rfc3339(),
                transcripts: meeting_transcripts,
            }))
        } else {
            transaction.rollback().await?;
            Ok(None)
        }
    }

    /// Get meeting metadata without transcripts (for pagination)
    pub async fn get_meeting_metadata(
        pool: &SqlitePool,
        ctx: &AuthContext,
        meeting_id: &str,
    ) -> Result<Option<MeetingModel>, SqlxError> {
        if meeting_id.trim().is_empty() {
            return Err(SqlxError::Protocol(
                "meeting_id cannot be empty".to_string(),
            ));
        }

        let meeting: Option<MeetingModel> = sqlx::query_as(
            "SELECT id, title, created_at, updated_at, folder_path FROM meetings \
             WHERE id = ? AND workspace_id = ?",
        )
        .bind(meeting_id)
        .bind(ctx.tenant_id.as_str())
        .fetch_optional(pool)
        .await?;

        Ok(meeting)
    }

    /// Get meeting transcripts with pagination support
    pub async fn get_meeting_transcripts_paginated(
        pool: &SqlitePool,
        ctx: &AuthContext,
        meeting_id: &str,
        limit: i64,
        offset: i64,
    ) -> Result<(Vec<Transcript>, i64), SqlxError> {
        if meeting_id.trim().is_empty() {
            return Err(SqlxError::Protocol(
                "meeting_id cannot be empty".to_string(),
            ));
        }

        // Get total count of transcripts for this meeting
        let total: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM transcripts WHERE meeting_id = ? AND workspace_id = ?",
        )
        .bind(meeting_id)
        .bind(ctx.tenant_id.as_str())
        .fetch_one(pool)
        .await?;

        // Get paginated transcripts ordered by audio_start_time
        let transcripts = sqlx::query_as::<_, Transcript>(
            "SELECT * FROM transcripts
             WHERE meeting_id = ? AND workspace_id = ?
             ORDER BY audio_start_time ASC
             LIMIT ? OFFSET ?",
        )
        .bind(meeting_id)
        .bind(ctx.tenant_id.as_str())
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;

        Ok((transcripts, total.0))
    }

    pub async fn update_meeting_title(
        pool: &SqlitePool,
        ctx: &AuthContext,
        meeting_id: &str,
        new_title: &str,
    ) -> Result<bool, SqlxError> {
        if meeting_id.trim().is_empty() {
            return Err(SqlxError::Protocol(
                "meeting_id cannot be empty".to_string(),
            ));
        }

        let mut conn = pool.acquire().await?;
        let mut transaction = conn.begin().await?;

        // Bind DateTime<Utc> (RFC 3339 text), matching every other writer in this
        // layer; this method previously bound `naive_utc()` (space-separated, no
        // offset) — a pre-existing format inconsistency fixed in B2 phase 2.
        let now = Utc::now();

        let rows_affected = sqlx::query(
            "UPDATE meetings SET title = ?, updated_at = ?, updated_by = ?, rev = rev + 1 \
             WHERE id = ? AND workspace_id = ?",
        )
        .bind(new_title)
        .bind(now)
        .bind(ctx.user_id.as_str())
        .bind(meeting_id)
        .bind(ctx.tenant_id.as_str())
        .execute(&mut *transaction)
        .await?;
        if rows_affected.rows_affected() == 0 {
            transaction.rollback().await?;
            return Ok(false);
        }
        transaction.commit().await?;
        Ok(true)
    }

    pub async fn update_meeting_name(
        pool: &SqlitePool,
        ctx: &AuthContext,
        meeting_id: &str,
        new_title: &str,
    ) -> Result<bool, SqlxError> {
        let mut transaction = pool.begin().await?;
        let now = Utc::now();

        // Update meetings table
        let meeting_update = sqlx::query(
            "UPDATE meetings SET title = ?, updated_at = ?, updated_by = ?, rev = rev + 1 \
             WHERE id = ? AND workspace_id = ?",
        )
        .bind(new_title)
        .bind(now)
        .bind(ctx.user_id.as_str())
        .bind(meeting_id)
        .bind(ctx.tenant_id.as_str())
        .execute(&mut *transaction)
        .await?;

        if meeting_update.rows_affected() == 0 {
            transaction.rollback().await?;
            return Ok(false); // Meeting not found
        }

        // Update transcript_chunks table (synced table: stamp sync columns too)
        sqlx::query(
            "UPDATE transcript_chunks SET meeting_name = ?, updated_at = ?, updated_by = ?, \
             rev = rev + 1 WHERE meeting_id = ? AND workspace_id = ?",
        )
        .bind(new_title)
        .bind(now)
        .bind(ctx.user_id.as_str())
        .bind(meeting_id)
        .bind(ctx.tenant_id.as_str())
        .execute(&mut *transaction)
        .await?;

        transaction.commit().await?;
        Ok(true)
    }
}

pub(crate) async fn delete_meeting_with_connection(
    transaction: &mut SqliteConnection,
    ctx: &AuthContext,
    meeting_id: &str,
) -> Result<bool, SqlxError> {
    // Check if meeting exists in this workspace
    let meeting_exists: Option<(i64,)> =
        sqlx::query_as("SELECT 1 FROM meetings WHERE id = ? AND workspace_id = ?")
            .bind(meeting_id)
            .bind(ctx.tenant_id.as_str())
            .fetch_optional(&mut *transaction)
            .await?;

    if meeting_exists.is_none() {
        return Ok(false);
    }

    // This connection has foreign-key actions disabled by `delete_database_records`
    // and is close-on-drop. Delete every caller-owned child explicitly so malformed
    // foreign-workspace rows are never read and never cascaded away.
    sqlx::query("DELETE FROM action_items WHERE meeting_id = ? AND workspace_id = ?")
        .bind(meeting_id)
        .bind(ctx.tenant_id.as_str())
        .execute(&mut *transaction)
        .await?;

    sqlx::query("DELETE FROM summaries WHERE meeting_id = ? AND workspace_id = ?")
        .bind(meeting_id)
        .bind(ctx.tenant_id.as_str())
        .execute(&mut *transaction)
        .await?;

    sqlx::query("DELETE FROM meeting_notes WHERE meeting_id = ? AND workspace_id = ?")
        .bind(meeting_id)
        .bind(ctx.tenant_id.as_str())
        .execute(&mut *transaction)
        .await?;

    sqlx::query("DELETE FROM transcript_chunks WHERE meeting_id = ? AND workspace_id = ?")
        .bind(meeting_id)
        .bind(ctx.tenant_id.as_str())
        .execute(&mut *transaction)
        .await?;

    sqlx::query("DELETE FROM summary_processes WHERE meeting_id = ? AND workspace_id = ?")
        .bind(meeting_id)
        .bind(ctx.tenant_id.as_str())
        .execute(&mut *transaction)
        .await?;

    sqlx::query("DELETE FROM transcripts WHERE meeting_id = ? AND workspace_id = ?")
        .bind(meeting_id)
        .bind(ctx.tenant_id.as_str())
        .execute(&mut *transaction)
        .await?;

    // Finally delete only the caller-owned parent. With cascades disabled on
    // this disposable connection, foreign malformed children remain untouched.
    let result = sqlx::query("DELETE FROM meetings WHERE id = ? AND workspace_id = ?")
        .bind(meeting_id)
        .bind(ctx.tenant_id.as_str())
        .execute(&mut *transaction)
        .await?;

    Ok(result.rows_affected() > 0)
}
