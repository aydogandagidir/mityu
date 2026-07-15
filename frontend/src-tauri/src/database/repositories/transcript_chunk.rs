//! Tenant-scoped transcript-chunks repository (docs/CONTRACTS.md §2, BACKLOG B2
//! phase 2). `transcript_chunks.updated_at` is nullable at the SQL level (added
//! by migration 20260702000000), so every write here MUST populate it; writes
//! also stamp `rev`/`updated_by`. Child writes atomically require a live parent
//! meeting in the caller's workspace, and the upsert also guards its update path
//! with the same workspace so a conflicting row never crosses tenant boundaries.

use crate::context::AuthContext;
use chrono::Utc;
use log::info as log_info;
use sqlx::SqlitePool;

/// One logical `transcript_chunks` record: the full transcript text plus the
/// processing parameters it was produced with.
#[derive(Debug, Clone, Copy)]
pub struct TranscriptChunkData<'a> {
    pub text: &'a str,
    pub model: &'a str,
    pub model_name: &'a str,
    pub chunk_size: i32,
    pub overlap: i32,
}

pub struct TranscriptChunksRepository;

impl TranscriptChunksRepository {
    /// Saves the full transcript text and processing parameters.
    pub async fn save_transcript_data(
        pool: &SqlitePool,
        ctx: &AuthContext,
        meeting_id: &str,
        data: TranscriptChunkData<'_>,
    ) -> Result<(), sqlx::Error> {
        log_info!(
            "Saving transcript data to transcript_chunks for meeting_id: {}",
            meeting_id
        );
        let now = Utc::now();
        let result = sqlx::query(
            r#"
            INSERT INTO transcript_chunks (meeting_id, workspace_id, transcript_text, model, model_name, chunk_size, overlap, created_at, updated_at, updated_by, rev)
            SELECT ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 1
            WHERE EXISTS (
                SELECT 1
                FROM meetings
                WHERE id = ? AND workspace_id = ? AND deleted_at IS NULL
            )
            ON CONFLICT(meeting_id) DO UPDATE SET
                transcript_text = excluded.transcript_text,
                model = excluded.model,
                model_name = excluded.model_name,
                chunk_size = excluded.chunk_size,
                overlap = excluded.overlap,
                created_at = excluded.created_at,
                updated_at = excluded.updated_at,
                updated_by = excluded.updated_by,
                rev = rev + 1
            WHERE workspace_id = excluded.workspace_id
            "#,
        )
        .bind(meeting_id)
        .bind(ctx.tenant_id.as_str())
        .bind(data.text)
        .bind(data.model)
        .bind(data.model_name)
        .bind(data.chunk_size)
        .bind(data.overlap)
        .bind(now)
        .bind(now)
        .bind(ctx.user_id.as_str())
        .bind(meeting_id)
        .bind(ctx.tenant_id.as_str())
        .execute(pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(sqlx::Error::Protocol(
                "Parent meeting is unavailable in this workspace".to_string(),
            ));
        }

        Ok(())
    }
}
