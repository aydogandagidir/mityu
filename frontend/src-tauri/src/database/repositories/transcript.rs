//! Tenant-scoped transcripts repository (docs/CONTRACTS.md §2, BACKLOG B2 phase 2).
//!
//! Every method takes [`AuthContext`] and scopes each statement with
//! `workspace_id = ctx.tenant_id`. `transcripts.created_at`/`updated_at` are
//! nullable at the SQL level (added by migration 20260702000000), so every
//! INSERT here MUST populate them; writes also stamp `rev`/`updated_by`.

use crate::api::{TranscriptSearchResult, TranscriptSegment};
use crate::context::AuthContext;
use chrono::Utc;
use sqlx::{Connection, Error as SqlxError, SqlitePool};
use tracing::{error, info};
use uuid::Uuid;

pub struct TranscriptsRepository;

impl TranscriptsRepository {
    /// Saves a new meeting and its associated transcript segments.
    /// This function uses a transaction to ensure that either both the meeting
    /// and all its transcripts are saved, or none of them are.
    ///
    /// Segment ids are re-minted here (`transcript-{uuid}`); use
    /// [`Self::create_meeting_with_segments`] when caller-supplied segment ids
    /// must be preserved (e.g. audio import).
    pub async fn save_transcript(
        pool: &SqlitePool,
        ctx: &AuthContext,
        meeting_title: &str,
        transcripts: &[TranscriptSegment],
        folder_path: Option<String>,
    ) -> Result<String, SqlxError> {
        let meeting_id = format!("meeting-{}", Uuid::new_v4());

        let mut conn = pool.acquire().await?;
        let mut transaction = conn.begin().await?;

        let now = Utc::now();

        // 1. Create the new meeting
        let result = sqlx::query(
            "INSERT INTO meetings \
             (id, workspace_id, title, created_at, updated_at, updated_by, rev, folder_path) \
             VALUES (?, ?, ?, ?, ?, ?, 1, ?)",
        )
        .bind(&meeting_id)
        .bind(ctx.tenant_id.as_str())
        .bind(meeting_title)
        .bind(now)
        .bind(now)
        .bind(ctx.user_id.as_str())
        .bind(&folder_path)
        .execute(&mut *transaction)
        .await;

        if let Err(e) = result {
            error!("Failed to create meeting '{}': {}", meeting_title, e);
            transaction.rollback().await?;
            return Err(e);
        }

        info!("Successfully created meeting with id: {}", meeting_id);

        // 2. Save each transcript segment with audio timing fields
        for segment in transcripts {
            let transcript_id = format!("transcript-{}", Uuid::new_v4());
            let result = insert_transcript_segment(
                &mut transaction,
                ctx,
                &transcript_id,
                &meeting_id,
                segment,
                now,
            )
            .await;

            if let Err(e) = result {
                error!(
                    "Failed to save transcript segment for meeting {}: {}",
                    meeting_id, e
                );
                transaction.rollback().await?;
                return Err(e);
            }
        }

        info!(
            "Successfully saved {} transcript segments for meeting {}",
            transcripts.len(),
            meeting_id
        );

        // Commit the transaction
        transaction.commit().await?;

        Ok(meeting_id)
    }

    /// Creates a new meeting and inserts the given segments **preserving their
    /// ids** (audio import flow: ids are already referenced by the on-disk
    /// `transcripts.json`). Same transactional guarantee as
    /// [`Self::save_transcript`].
    pub async fn create_meeting_with_segments(
        pool: &SqlitePool,
        ctx: &AuthContext,
        meeting_title: &str,
        segments: &[TranscriptSegment],
        folder_path: Option<String>,
    ) -> Result<String, SqlxError> {
        let meeting_id = format!("meeting-{}", Uuid::new_v4());

        let mut conn = pool.acquire().await?;
        let mut transaction = conn.begin().await?;

        let now = Utc::now();

        sqlx::query(
            "INSERT INTO meetings \
             (id, workspace_id, title, created_at, updated_at, updated_by, rev, folder_path) \
             VALUES (?, ?, ?, ?, ?, ?, 1, ?)",
        )
        .bind(&meeting_id)
        .bind(ctx.tenant_id.as_str())
        .bind(meeting_title)
        .bind(now)
        .bind(now)
        .bind(ctx.user_id.as_str())
        .bind(&folder_path)
        .execute(&mut *transaction)
        .await?;

        for segment in segments {
            insert_transcript_segment(
                &mut transaction,
                ctx,
                &segment.id,
                &meeting_id,
                segment,
                now,
            )
            .await?;
        }

        transaction.commit().await?;

        info!(
            "Created meeting '{}' with {} transcripts",
            meeting_id,
            segments.len()
        );

        Ok(meeting_id)
    }

    /// Atomically replaces every transcript segment of a meeting (retranscription
    /// flow). Delete + inserts run in one transaction so a failure cannot lose
    /// the existing transcript. Segment ids are preserved.
    pub async fn replace_meeting_transcripts(
        pool: &SqlitePool,
        ctx: &AuthContext,
        meeting_id: &str,
        segments: &[TranscriptSegment],
    ) -> Result<(), SqlxError> {
        if meeting_id.trim().is_empty() {
            return Err(SqlxError::Protocol(
                "meeting_id cannot be empty".to_string(),
            ));
        }

        let mut conn = pool.acquire().await?;
        let mut transaction = conn.begin().await?;

        let now = Utc::now();

        // The meeting must exist in the caller's workspace: without this check a
        // foreign context could attach segments to another workspace's meeting.
        let meeting_exists: Option<(i64,)> =
            sqlx::query_as("SELECT 1 FROM meetings WHERE id = ? AND workspace_id = ?")
                .bind(meeting_id)
                .bind(ctx.tenant_id.as_str())
                .fetch_optional(&mut *transaction)
                .await?;
        if meeting_exists.is_none() {
            transaction.rollback().await?;
            return Err(SqlxError::RowNotFound);
        }

        sqlx::query("DELETE FROM transcripts WHERE meeting_id = ? AND workspace_id = ?")
            .bind(meeting_id)
            .bind(ctx.tenant_id.as_str())
            .execute(&mut *transaction)
            .await?;

        for segment in segments {
            insert_transcript_segment(&mut transaction, ctx, &segment.id, meeting_id, segment, now)
                .await?;
        }

        transaction.commit().await?;

        info!(
            "Replaced transcripts for meeting {} with {} segments",
            meeting_id,
            segments.len()
        );

        Ok(())
    }

    /// Searches for a query string within the transcripts.
    /// It returns a list of matching transcripts with context.
    pub async fn search_transcripts(
        pool: &SqlitePool,
        ctx: &AuthContext,
        query: &str,
    ) -> Result<Vec<TranscriptSearchResult>, SqlxError> {
        if query.trim().is_empty() {
            return Ok(Vec::new());
        }

        let search_query = format!("%{}%", query.to_lowercase());

        let rows = sqlx::query_as::<_, (String, String, String, String)>(
            "SELECT m.id, m.title, t.transcript, t.timestamp
             FROM meetings m
             JOIN transcripts t ON m.id = t.meeting_id
             WHERE m.workspace_id = ? AND t.workspace_id = ? AND LOWER(t.transcript) LIKE ?",
        )
        .bind(ctx.tenant_id.as_str())
        .bind(ctx.tenant_id.as_str())
        .bind(&search_query)
        .fetch_all(pool)
        .await?;

        let results = rows
            .into_iter()
            .map(|(id, title, transcript, timestamp)| {
                let match_context = Self::get_match_context(&transcript, query);
                TranscriptSearchResult {
                    id,
                    title,
                    match_context,
                    timestamp,
                }
            })
            .collect();

        Ok(results)
    }

    /// Helper function to extract a snippet of text around the first match of a query.
    fn get_match_context(transcript: &str, query: &str) -> String {
        let transcript_lower = transcript.to_lowercase();
        let query_lower = query.to_lowercase();

        match transcript_lower.find(&query_lower) {
            Some(match_index) => {
                let start_index = match_index.saturating_sub(100);
                let end_index = (match_index + query.len() + 100).min(transcript.len());

                let mut context = String::new();
                if start_index > 0 {
                    context.push_str("...");
                }
                context.push_str(&transcript[start_index..end_index]);
                if end_index < transcript.len() {
                    context.push_str("...");
                }
                context
            }
            None => transcript.chars().take(200).collect(), // Fallback to the start of the transcript
        }
    }
}

/// Single INSERT shape for a transcript segment row: sets `workspace_id` from the
/// context and always populates `created_at`/`updated_at` (nullable columns) plus
/// `rev = 1` / `updated_by` (sync columns).
async fn insert_transcript_segment(
    conn: &mut sqlx::SqliteConnection,
    ctx: &AuthContext,
    transcript_id: &str,
    meeting_id: &str,
    segment: &TranscriptSegment,
    now: chrono::DateTime<Utc>,
) -> Result<(), SqlxError> {
    sqlx::query(
        "INSERT INTO transcripts \
         (id, workspace_id, meeting_id, transcript, timestamp, audio_start_time, \
          audio_end_time, duration, created_at, updated_at, updated_by, rev) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 1)",
    )
    .bind(transcript_id)
    .bind(ctx.tenant_id.as_str())
    .bind(meeting_id)
    .bind(&segment.text)
    .bind(&segment.timestamp)
    .bind(segment.audio_start_time)
    .bind(segment.audio_end_time)
    .bind(segment.duration)
    .bind(now)
    .bind(now)
    .bind(ctx.user_id.as_str())
    .execute(&mut *conn)
    .await?;
    Ok(())
}
