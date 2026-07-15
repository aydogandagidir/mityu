//! Tenant-scoped transcripts repository (docs/CONTRACTS.md §2, BACKLOG B2 phase 2).
//!
//! Every method takes [`AuthContext`] and scopes each statement with
//! `workspace_id = ctx.tenant_id`. `transcripts.created_at`/`updated_at` are
//! nullable at the SQL level (added by migration 20260702000000), so every
//! INSERT here MUST populate them; writes also stamp `rev`/`updated_by`.

use crate::api::{
    EvidenceSearchResult, SearchMatchField, TranscriptSearchResult, TranscriptSegment,
};
use crate::context::AuthContext;
use chrono::Utc;
use sqlx::{Connection, Error as SqlxError, SqlitePool};
use std::collections::HashSet;
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
            error!("Failed to create meeting {}: {}", meeting_id, e);
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
    ///
    /// ADR-0019 decision 2: after a successful replace, the meeting's
    /// structured summary and action items are downgraded to `draft` — the
    /// segment rows their approved evidence links (`source_chunk_id`) point at
    /// were just deleted and re-inserted, so a human must re-review them
    /// against the changed segments.
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

        // Return the checked-out connection to the pool BEFORE the downgrade
        // hook below re-acquires from the same pool (a 1-connection pool would
        // otherwise deadlock into PoolTimedOut).
        drop(conn);

        // ADR-0019 decision 2 (retranscription downgrade): approved evidence
        // links must be re-reviewed against the changed segments, so any
        // approved/edited HITL state for this meeting drops back to draft
        // (original content/text preserved). `Ok(false)` — no summary row / no
        // touched items — is the normal no-op case. The replace above is
        // already committed; an error here means only the downgrade could not
        // be applied and is surfaced to the caller.
        //
        // The two downgrades are not atomic with each other (gate-review W1): if
        // the second errors, the whole call returns `Err` (the caller sees the
        // retranscription as failed) and a retry re-runs both — `downgrade_to_draft`
        // is idempotent (draft→draft is a no-op). Any item left stale-approved is
        // also refused by approve-time source re-resolution on the next approve.
        // Make these one transaction when the repo executor is generalized (Phase 2).
        use super::action_item::ActionItemsRepository;
        use super::summary_draft::{SummariesRepository, SummaryDraftError};
        SummariesRepository::downgrade_to_draft(pool, ctx, meeting_id)
            .await
            .map_err(SummaryDraftError::into_sqlx)?;
        ActionItemsRepository::downgrade_to_draft(pool, ctx, meeting_id)
            .await
            .map_err(SummaryDraftError::into_sqlx)?;

        Ok(())
    }

    /// Ranked local evidence search over persisted transcript segments.
    ///
    /// Product Intelligence v1 intentionally searches transcript evidence only:
    /// every returned row has a resolvable `source_chunk_id`, so the UI can jump
    /// straight to the supporting segment. Legacy/unreviewed summary JSON is not
    /// part of this trusted retrieval surface. The older [`Self::search_transcripts`]
    /// command remains available for backwards compatibility.
    ///
    /// The FTS table is a derived index, never an authority. Every candidate is
    /// joined back to both source tables and all three workspace predicates are
    /// bound from `AuthContext`; soft-deleted source rows cannot be returned even
    /// if a stale derived row were ever present.
    pub async fn search_evidence(
        pool: &SqlitePool,
        ctx: &AuthContext,
        query: &str,
    ) -> Result<Vec<EvidenceSearchResult>, SqlxError> {
        let Some(match_query) = build_evidence_match_query(query) else {
            return Ok(Vec::new());
        };

        // Rank every matching segment, then select the best evidence segment per
        // meeting before applying the public result cap. A long meeting can have
        // hundreds of strong matches; limiting segments first would let that one
        // meeting crowd every other matching meeting out of the result set.
        const RESULT_LIMIT: i64 = 50;

        let rows = sqlx::query_as::<_, (String, String, String, String, Option<f64>, String, f64)>(
            "WITH matching_segments AS MATERIALIZED (
                 SELECT m.id AS result_id,
                        m.title,
                        t.id AS source_chunk_id,
                        t.timestamp,
                        t.audio_start_time,
                        t.transcript,
                        bm25(transcript_search_fts) AS rank
                   FROM transcript_search_fts
                   JOIN transcript_search_documents AS d
                     ON d.id = transcript_search_fts.rowid
                   JOIN transcripts AS t
                     ON t.id = d.source_chunk_id
                    AND t.meeting_id = d.meeting_id
                    AND t.workspace_id = d.workspace_id
                   JOIN meetings AS m
                     ON m.id = t.meeting_id
                    AND m.workspace_id = t.workspace_id
                  WHERE transcript_search_fts MATCH ?
                    AND d.workspace_id = ?
                    AND t.workspace_id = ?
                    AND m.workspace_id = ?
                    AND t.deleted_at IS NULL
                    AND m.deleted_at IS NULL
             ),
             ranked_meetings AS (
                 SELECT result_id,
                        title,
                        source_chunk_id,
                        timestamp,
                        audio_start_time,
                        transcript,
                        rank,
                        ROW_NUMBER() OVER (
                            PARTITION BY result_id
                            ORDER BY rank ASC,
                                     COALESCE(audio_start_time, 1.0e30) ASC,
                                     source_chunk_id ASC
                        ) AS meeting_row
                   FROM matching_segments
             )
             SELECT result_id,
                    title,
                    source_chunk_id,
                    timestamp,
                    audio_start_time,
                    transcript,
                    rank
               FROM ranked_meetings
              WHERE meeting_row = 1
              ORDER BY rank ASC,
                       COALESCE(audio_start_time, 1.0e30) ASC,
                       source_chunk_id ASC
              LIMIT ?",
        )
        .bind(&match_query)
        .bind(ctx.tenant_id.as_str())
        .bind(ctx.tenant_id.as_str())
        .bind(ctx.tenant_id.as_str())
        .bind(RESULT_LIMIT)
        .fetch_all(pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(
                |(
                    id,
                    title,
                    source_chunk_id,
                    timestamp,
                    audio_start_time,
                    transcript_text,
                    _rank,
                )| EvidenceSearchResult {
                    id,
                    title,
                    match_context: evidence_match_context(&transcript_text, query),
                    timestamp,
                    matched_in: SearchMatchField::Transcript,
                    source_chunk_id,
                    audio_start_time,
                },
            )
            .collect())
    }

    /// Searches, within the caller's workspace, for a query string in either a
    /// meeting's transcript text OR its generated summary (BACKLOG C3). Returns
    /// one row per meeting; `matched_in` labels where the hit came from and
    /// `match_context` is a snippet from that same field. When a meeting matches
    /// in both a transcript segment and its summary, the transcript hit wins
    /// (deterministic dedup by meeting id).
    ///
    /// Tenant isolation (docs/MULTITENANCY.md rule 3): every joined table is
    /// scoped by `workspace_id = ctx.tenant_id` — `meetings`, `transcripts`, and
    /// `summary_processes` each get their own bound predicate; there is no
    /// unscoped join. The summary side is a LEFT JOIN so a summary-only match
    /// (no matching transcript segment) is still found.
    ///
    /// Matching is LIKE substring (case-insensitive) against the raw text; the
    /// summary text is `summary_processes.result`, a JSON string holding the
    /// generated summary, so a query term matches the summary content directly.
    /// `meeting_notes` is intentionally not searched: it is not wired to any
    /// writer/reader in the app today (migration-only), so it holds no
    /// user-facing content to match.
    ///
    /// FUTURE (ranking/tokenization): migrate this to a SQLite FTS5 virtual
    /// table (per-workspace, indexing transcript + summary text) for ranked,
    /// tokenized matching instead of unranked LIKE substring scans. That is a
    /// schema change and out of scope for this additive step.
    pub async fn search_transcripts(
        pool: &SqlitePool,
        ctx: &AuthContext,
        query: &str,
    ) -> Result<Vec<TranscriptSearchResult>, SqlxError> {
        if query.trim().is_empty() {
            return Ok(Vec::new());
        }

        let search_query = format!("%{}%", query.to_lowercase());

        // One scoped query path. Row shape:
        //   (meeting_id, title, transcript_text, timestamp, summary_result)
        // transcript_text/timestamp are NULL when only the summary matched;
        // summary_result is NULL when only a transcript segment matched. Every
        // table in the FROM/JOIN list carries its own `workspace_id = ?` guard,
        // each bound to `ctx.tenant_id`, so nothing outside the workspace can
        // ever join in.
        let rows = sqlx::query_as::<
            _,
            (
                String,
                String,
                Option<String>,
                Option<String>,
                Option<String>,
            ),
        >(
            "SELECT m.id, m.title, t.transcript, t.timestamp, s.result
             FROM meetings m
             LEFT JOIN transcripts t
               ON m.id = t.meeting_id
              AND t.workspace_id = ?
              AND LOWER(t.transcript) LIKE ?
             LEFT JOIN summary_processes s
               ON m.id = s.meeting_id
              AND s.workspace_id = ?
              AND s.result IS NOT NULL
              AND LOWER(s.result) LIKE ?
             WHERE m.workspace_id = ?
               AND (t.transcript IS NOT NULL OR s.result IS NOT NULL)",
        )
        .bind(ctx.tenant_id.as_str())
        .bind(&search_query)
        .bind(ctx.tenant_id.as_str())
        .bind(&search_query)
        .bind(ctx.tenant_id.as_str())
        .fetch_all(pool)
        .await?;

        // Dedup to one row per meeting. A meeting can yield several rows (one per
        // matching transcript segment, each optionally paired with the summary
        // match). Prefer a transcript hit deterministically; fall back to the
        // summary hit for summary-only meetings.
        let mut seen: HashSet<String> = HashSet::new();
        let mut results: Vec<TranscriptSearchResult> = Vec::new();

        // First pass: transcript matches win.
        for (id, title, transcript, timestamp, _summary) in &rows {
            if let Some(text) = transcript {
                if seen.insert(id.clone()) {
                    results.push(TranscriptSearchResult {
                        id: id.clone(),
                        title: title.clone(),
                        match_context: Self::get_match_context(text, query),
                        timestamp: timestamp.clone().unwrap_or_default(),
                        matched_in: SearchMatchField::Transcript,
                    });
                }
            }
        }

        // Second pass: summary-only matches (meetings not already claimed above).
        for (id, title, _transcript, _timestamp, summary) in &rows {
            if let Some(text) = summary {
                if seen.insert(id.clone()) {
                    results.push(TranscriptSearchResult {
                        id: id.clone(),
                        title: title.clone(),
                        match_context: Self::get_match_context(text, query),
                        // Summaries are not tied to a single segment timestamp;
                        // the UI keys off the meeting, so an empty timestamp is
                        // the honest value here.
                        timestamp: String::new(),
                        matched_in: SearchMatchField::Summary,
                    });
                }
            }
        }

        Ok(results)
    }

    /// Extracts a snippet of `text` around the first case-insensitive match of
    /// `query` (±100 chars, ellipsized). Generalized from transcript-only to any
    /// matched field so a summary hit is snippeted from the summary text, not an
    /// unmatched transcript.
    fn get_match_context(text: &str, query: &str) -> String {
        let text_lower = text.to_lowercase();
        let query_lower = query.to_lowercase();

        match text_lower.find(&query_lower) {
            Some(match_index) => {
                let start_index = match_index.saturating_sub(100);
                let end_index = (match_index + query.len() + 100).min(text.len());

                // Snap to char boundaries: `text` may be UTF-8 multibyte and the
                // ±100/query.len() arithmetic works on byte offsets.
                let start_index = floor_char_boundary(text, start_index);
                let end_index = ceil_char_boundary(text, end_index);

                let mut context = String::new();
                if start_index > 0 {
                    context.push_str("...");
                }
                context.push_str(&text[start_index..end_index]);
                if end_index < text.len() {
                    context.push_str("...");
                }
                context
            }
            None => text.chars().take(200).collect(), // Fallback to the start of the text
        }
    }
}

/// Convert free-form user input into a bounded, literal FTS5 prefix query.
///
/// Only Unicode alphanumeric runs become tokens, so FTS operators supplied by
/// the user (`OR`, `NEAR`, quotes, parentheses, `*`) can never become syntax.
/// Each token is still quoted defensively and joined with AND. One-character
/// tokens are discarded and two-character tokens match exactly; only longer
/// tokens get prefix expansion. An interactive query therefore cannot fan a
/// one/two-character prefix across almost the entire corpus. A future hybrid
/// retriever can improve morphology without weakening that bound.
fn build_evidence_match_query(query: &str) -> Option<String> {
    let tokens = evidence_query_tokens(query);
    if tokens.is_empty() {
        return None;
    }

    Some(
        tokens
            .into_iter()
            .map(|token| {
                let literal = token.replace('"', "\"\"");
                if token.chars().count() == 2 {
                    // Preserve useful acronyms such as AI/HR without expanding
                    // a two-character prefix over a large vocabulary.
                    format!("\"{literal}\"")
                } else {
                    format!("\"{literal}\"*")
                }
            })
            .collect::<Vec<_>>()
            .join(" AND "),
    )
}

/// Bounded Unicode tokenization shared by the FTS expression and snippet
/// locator. Turkish I/İ/ı are intentionally folded together for forgiving
/// TR/EN/code-switch search; common Turkish diacritics are transliterated so
/// `toplanti` can find `toplantı` and the behavior does not depend on the host
/// SQLite build's Unicode table.
fn evidence_query_tokens(query: &str) -> Vec<String> {
    const MAX_QUERY_CHARS: usize = 256;
    const MAX_TOKEN_CHARS: usize = 64;
    const MAX_TOKENS: usize = 16;
    const MIN_TOKEN_CHARS: usize = 2;

    let mut raw_tokens: Vec<String> = Vec::new();
    let mut current = String::new();

    for ch in query.chars().take(MAX_QUERY_CHARS) {
        if ch.is_alphanumeric() {
            if current.chars().count() < MAX_TOKEN_CHARS {
                current.push(ch);
            }
        } else if !current.is_empty() {
            raw_tokens.push(std::mem::take(&mut current));
            if raw_tokens.len() == MAX_TOKENS {
                break;
            }
        }
    }
    if !current.is_empty() && raw_tokens.len() < MAX_TOKENS {
        raw_tokens.push(current);
    }

    let mut seen = HashSet::new();
    raw_tokens
        .into_iter()
        .map(|token| normalize_evidence_token(&token))
        .filter(|token| token.chars().count() >= MIN_TOKEN_CHARS && seen.insert(token.clone()))
        .take(MAX_TOKENS)
        .collect()
}

fn normalize_evidence_token(value: &str) -> String {
    let mut normalized = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            'I' | 'İ' | 'ı' => normalized.push('i'),
            'Ç' | 'ç' => normalized.push('c'),
            'Ğ' | 'ğ' => normalized.push('g'),
            'Ö' | 'ö' => normalized.push('o'),
            'Ş' | 'ş' => normalized.push('s'),
            'Ü' | 'ü' => normalized.push('u'),
            _ => normalized.extend(ch.to_lowercase()),
        }
    }
    normalized
}

/// Return an original-text snippet around the first word that satisfies the FTS
/// token semantics (two characters = exact; longer = prefix). The FTS table
/// stores normalized text for Turkish recall, but the UI must always show the
/// user's authoritative transcript.
fn evidence_match_context(text: &str, query: &str) -> String {
    let tokens = evidence_query_tokens(query);
    let mut word_start: Option<usize> = None;

    for (index, ch) in text
        .char_indices()
        .chain(std::iter::once((text.len(), ' ')))
    {
        if ch.is_alphanumeric() {
            word_start.get_or_insert(index);
            continue;
        }

        let Some(start) = word_start.take() else {
            continue;
        };
        let word = normalize_evidence_token(&text[start..index]);
        if tokens.iter().any(|token| {
            if token.chars().count() == 2 {
                word == token.as_str()
            } else {
                word.starts_with(token)
            }
        }) {
            let context_start = floor_char_boundary(text, start.saturating_sub(100));
            let context_end = ceil_char_boundary(text, (index + 100).min(text.len()));
            let mut context = String::new();
            if context_start > 0 {
                context.push_str("...");
            }
            context.push_str(&text[context_start..context_end]);
            if context_end < text.len() {
                context.push_str("...");
            }
            return context;
        }
    }

    text.chars().take(200).collect()
}

/// Largest char-boundary index `<= index` in `s` (stable stand-in for the
/// nightly `str::floor_char_boundary`).
fn floor_char_boundary(s: &str, index: usize) -> usize {
    if index >= s.len() {
        return s.len();
    }
    let mut i = index;
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

/// Smallest char-boundary index `>= index` in `s` (stable stand-in for the
/// nightly `str::ceil_char_boundary`).
fn ceil_char_boundary(s: &str, index: usize) -> usize {
    if index >= s.len() {
        return s.len();
    }
    let mut i = index;
    while i < s.len() && !s.is_char_boundary(i) {
        i += 1;
    }
    i
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

#[cfg(test)]
mod evidence_search_tests {
    use super::evidence_match_context;

    #[test]
    fn two_character_snippet_anchors_to_exact_token_not_an_earlier_prefix() {
        let transcript = format!("İstanbul {} is gerçek kanıt", "uzak bağlam ".repeat(30));

        let context = evidence_match_context(&transcript, "is");

        assert!(context.contains(" is gerçek kanıt"));
        assert!(
            !context.contains("İstanbul"),
            "snippet must not anchor the exact 'is' query to the earlier İstanbul prefix"
        );
    }
}
