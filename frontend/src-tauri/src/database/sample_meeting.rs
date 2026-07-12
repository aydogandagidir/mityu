//! First-launch sample meeting seeder (v1 onboarding polish).
//!
//! On a brand-new install the app has an empty database and the user sees a
//! blank product. This module seeds ONE high-quality, fully source-linked
//! example meeting so the first launch demonstrates the core loop —
//! transcript segments, a structured HITL summary whose every block cites a
//! real transcript chunk, extracted action items, and the legacy markdown view
//! used by export — without any network, model, or recording.
//!
//! Called exactly once from
//! [`crate::database::commands::initialize_fresh_database`], AFTER the default
//! config rows are written. It is:
//!   * **Idempotent** — gated on the specific sample id, so a user who deletes
//!     the sample never gets it re-created, and a second launch is a no-op.
//!   * **Local-first** — pure local INSERTs; never fatal (the caller logs and
//!     continues if seeding fails, so a seeding hiccup cannot block first run).
//!   * **Tenant-aware** — every row carries `workspace_id`/`updated_by` from the
//!     [`AuthContext`] and `rev = 1` (docs/DATA_MODEL.md common columns).
//!
//! HITL invariant (CLAUDE.md §0.5, docs/CONTRACTS.md §4): the sample is authored
//! DIRECTLY (bypassing the repository upsert paths) so per-block statuses can be
//! a realistic mix of `draft`/`approved`/`edited`, but the summary-level status
//! is `draft` — nothing here is born approved, and EVERY block/action-item
//! `source_chunk_id` equals a real inserted `transcripts.id` for this
//! meeting+workspace (the same evidence gate the repositories enforce).

use crate::context::AuthContext;
use crate::database::repositories::summary_draft::block_status_to_db;
use crate::summary::draft::{BlockStatus, BlockType, DraftBlock, DraftSection};
use anyhow::Context;
use chrono::Utc;
use sqlx::SqlitePool;
use tracing::{debug, info};

/// Stable id of the seeded sample meeting. Idempotency and the "user deleted it,
/// never re-add" rule both key off this exact value.
pub const SAMPLE_MEETING_ID: &str = "meeting-sample-0001";

/// Stable id of the sample's structured summary row (the `summaries` table PK).
const SAMPLE_SUMMARY_ID: &str = "summary-sample-0001";

/// One transcript segment of the sample: `(id, audio_start_s, audio_end_s, text)`.
type Segment = (&'static str, f64, f64, &'static str);

/// The 14 sample transcript segments (~16 min). The two long gaps (300→360 and
/// 600→720) give the chapter strip real chapters.
const SEGMENTS: [Segment; 14] = [
    (
        "sample-seg-01",
        0.0,
        18.0,
        "Okay, let's lock the Q3 launch. I'd rather slip a week than ship the old onboarding.",
    ),
    (
        "sample-seg-02",
        20.0,
        52.0,
        "Design can't finish until the final copy lands — Friday is the real deadline for that.",
    ),
    (
        "sample-seg-03",
        55.0,
        95.0,
        "Amira will send the revised budget to finance before Thursday's review.",
    ),
    (
        "sample-seg-04",
        100.0,
        140.0,
        "Then we confirm the new date with vendors and move the weekly check-in to Thursday.",
    ),
    (
        "sample-seg-05",
        145.0,
        190.0,
        "Support is fine as long as the docs land two days before the public date.",
    ),
    (
        "sample-seg-06",
        200.0,
        240.0,
        "Let's not move the date twice — I'd rather cut scope than lose the launch momentum.",
    ),
    (
        "sample-seg-07",
        250.0,
        300.0,
        "Agreed. The revised budget is the gating item for finance this week.",
    ),
    (
        "sample-seg-08",
        360.0,
        410.0,
        "On vendors: they confirmed capacity but want the date in writing before Thursday.",
    ),
    (
        "sample-seg-09",
        415.0,
        470.0,
        "Marketing needs the final copy to lock the hero and the pricing page.",
    ),
    (
        "sample-seg-10",
        480.0,
        540.0,
        "Who owns the vendor confirmation so it doesn't fall through the cracks?",
    ),
    (
        "sample-seg-11",
        545.0,
        600.0,
        "Deniz can own it. I'll take the copy hand-off with design.",
    ),
    (
        "sample-seg-12",
        720.0,
        770.0,
        "Good. So March 21, pending vendors, budget to finance Thursday, copy by Friday.",
    ),
    (
        "sample-seg-13",
        775.0,
        830.0,
        "One risk: the docs owner is still open — support flagged that.",
    ),
    (
        "sample-seg-14",
        835.0,
        900.0,
        "Let's assign that after the vendor call. Thanks everyone.",
    ),
];

/// One sample action item authored directly into `action_items`.
struct SampleAction {
    id: &'static str,
    text: &'static str,
    assignee: Option<&'static str>,
    due: Option<&'static str>,
    source_chunk_id: &'static str,
    status: BlockStatus,
}

/// Builds the structured summary sections. Every block's `source_chunk_id`
/// references one of [`SEGMENTS`]; only the edited block carries
/// `original_content`.
fn sample_sections() -> Vec<DraftSection> {
    let text = |id: &str, content: &str, source: &str, status: BlockStatus| DraftBlock {
        id: id.to_string(),
        block_type: BlockType::Text,
        content: content.to_string(),
        source_chunk_id: source.to_string(),
        status,
        original_content: None,
    };
    let bullet = |id: &str, content: &str, source: &str, status: BlockStatus| DraftBlock {
        id: id.to_string(),
        block_type: BlockType::Bullet,
        content: content.to_string(),
        source_chunk_id: source.to_string(),
        status,
        original_content: None,
    };

    vec![
        DraftSection {
            title: "Overview".to_string(),
            blocks: vec![
                text(
                    "sample-b1",
                    "The team agreed to move the Q3 launch to March 21, pending vendor confirmation.",
                    "sample-seg-01",
                    BlockStatus::Approved,
                ),
                text(
                    "sample-b2",
                    "Design is blocked on the final launch copy until Friday, the real deadline.",
                    "sample-seg-02",
                    BlockStatus::Approved,
                ),
                DraftBlock {
                    id: "sample-b3".to_string(),
                    block_type: BlockType::Text,
                    content: "The revised budget goes to finance before Thursday's review."
                        .to_string(),
                    source_chunk_id: "sample-seg-03".to_string(),
                    status: BlockStatus::Edited,
                    original_content: Some("Budget to finance.".to_string()),
                },
                text(
                    "sample-b4",
                    "Onboarding will slip one week rather than ship the old flow to new users.",
                    "sample-seg-01",
                    BlockStatus::Draft,
                ),
            ],
        },
        DraftSection {
            title: "Decisions".to_string(),
            blocks: vec![
                bullet(
                    "sample-b5",
                    "Launch moved to March 21 (pending vendor sign-off).",
                    "sample-seg-12",
                    BlockStatus::Approved,
                ),
                bullet(
                    "sample-b6",
                    "Weekly check-in moved to Thursday.",
                    "sample-seg-04",
                    BlockStatus::Approved,
                ),
                bullet(
                    "sample-b7",
                    "Cut scope before moving the date a second time.",
                    "sample-seg-06",
                    BlockStatus::Approved,
                ),
            ],
        },
        DraftSection {
            title: "Risks & open questions".to_string(),
            blocks: vec![text(
                "sample-b8",
                "Docs must land two days before the public date — owner still open.",
                "sample-seg-13",
                BlockStatus::Draft,
            )],
        },
    ]
}

/// The sample action items, in display order (`position` = index).
fn sample_actions() -> Vec<SampleAction> {
    vec![
        SampleAction {
            id: "sample-action-01",
            text: "Send the revised budget to finance.",
            assignee: Some("Amira"),
            due: Some("Thu"),
            source_chunk_id: "sample-seg-03",
            status: BlockStatus::Approved,
        },
        SampleAction {
            id: "sample-action-02",
            text: "Confirm the new launch date with vendors in writing.",
            assignee: Some("Deniz"),
            due: Some("This week"),
            source_chunk_id: "sample-seg-08",
            status: BlockStatus::Draft,
        },
        SampleAction {
            id: "sample-action-03",
            text: "Finalize the launch copy with design.",
            assignee: Some("Sam"),
            due: Some("Fri"),
            source_chunk_id: "sample-seg-02",
            status: BlockStatus::Draft,
        },
        SampleAction {
            id: "sample-action-04",
            text: "Assign an owner for the docs / support hand-off.",
            assignee: None,
            due: Some("After vendor call"),
            source_chunk_id: "sample-seg-13",
            status: BlockStatus::Draft,
        },
    ]
}

/// Renders the legacy `summary_processes.result` markdown (the read/export path
/// used by `api_get_summary`). Mirrors the `{ "markdown": ... }` shape and the
/// block formatting of `summary::service::render_draft_markdown`, but — honoring
/// HITL — shows only human-APPROVED blocks, then lists the action items.
fn sample_result_markdown(sections: &[DraftSection], actions: &[SampleAction]) -> String {
    let mut parts: Vec<String> = Vec::new();

    for section in sections {
        let approved: Vec<&DraftBlock> = section
            .blocks
            .iter()
            .filter(|b| b.status == BlockStatus::Approved)
            .collect();
        if approved.is_empty() {
            continue;
        }
        let mut part = format!("## {}\n", section.title);
        for block in approved {
            let rendered = match block.block_type {
                BlockType::Heading1 => format!("\n# {}\n", block.content),
                BlockType::Heading2 => format!("\n## {}\n", block.content),
                BlockType::Text => format!("\n{}\n", block.content),
                BlockType::Bullet => format!("- {}\n", block.content),
            };
            part.push_str(&rendered);
        }
        parts.push(part);
    }

    if !actions.is_empty() {
        let mut part = String::from("## Action items\n");
        for action in actions {
            let mut line = format!("- {}", action.text);
            if let Some(assignee) = action.assignee {
                line.push_str(&format!(" — {assignee}"));
            }
            if let Some(due) = action.due {
                line.push_str(&format!(" (due: {due})"));
            }
            line.push('\n');
            part.push_str(&line);
        }
        parts.push(part);
    }

    parts.join("\n").trim_end().to_string()
}

/// Seeds the single sample meeting for a fresh install (see module docs).
///
/// Idempotent: if a meeting with [`SAMPLE_MEETING_ID`] already exists in the
/// caller's workspace this is a no-op (the user may have deleted it — we must
/// not re-create it). All rows are written in one transaction so a partial
/// sample can never be left behind.
pub async fn seed_sample_meeting(pool: &SqlitePool, ctx: &AuthContext) -> anyhow::Result<()> {
    let already_present: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM meetings WHERE workspace_id = ? AND id = ?")
            .bind(ctx.tenant_id.as_str())
            .bind(SAMPLE_MEETING_ID)
            .fetch_one(pool)
            .await
            .context("checking whether the sample meeting already exists")?;
    if already_present > 0 {
        debug!(
            meeting_id = SAMPLE_MEETING_ID,
            "sample meeting already present; skipping seed"
        );
        return Ok(());
    }

    let now = Utc::now();
    let sections = sample_sections();
    let actions = sample_actions();
    let sections_json =
        serde_json::to_string(&sections).context("serializing sample summary sections")?;
    let result_markdown = sample_result_markdown(&sections, &actions);
    let result_json = serde_json::json!({ "markdown": result_markdown }).to_string();

    let mut tx = pool
        .begin()
        .await
        .context("begin sample-seed transaction")?;

    // 1. Meeting (mirrors TranscriptsRepository column list; folder_path NULL).
    sqlx::query(
        "INSERT INTO meetings \
         (id, workspace_id, title, created_at, updated_at, updated_by, rev, folder_path) \
         VALUES (?, ?, ?, ?, ?, ?, 1, NULL)",
    )
    .bind(SAMPLE_MEETING_ID)
    .bind(ctx.tenant_id.as_str())
    .bind("Sample — Q3 launch planning")
    .bind(now)
    .bind(now)
    .bind(ctx.user_id.as_str())
    .execute(&mut *tx)
    .await
    .context("inserting sample meeting")?;

    // 2. Transcript segments FIRST — they are the evidence the summary and
    //    action items cite (the hard source-resolution gate).
    for (id, start, end, text) in SEGMENTS {
        // Wall-clock display string: the meeting start plus the segment offset.
        let wall_clock = (now + chrono::Duration::seconds(start as i64)).to_rfc3339();
        sqlx::query(
            "INSERT INTO transcripts \
             (id, workspace_id, meeting_id, transcript, timestamp, audio_start_time, \
              audio_end_time, duration, created_at, updated_at, updated_by, rev) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 1)",
        )
        .bind(id)
        .bind(ctx.tenant_id.as_str())
        .bind(SAMPLE_MEETING_ID)
        .bind(text)
        .bind(&wall_clock)
        .bind(start)
        .bind(end)
        .bind(end - start)
        .bind(now)
        .bind(now)
        .bind(ctx.user_id.as_str())
        .execute(&mut *tx)
        .await
        .with_context(|| format!("inserting sample transcript segment {id}"))?;
    }

    // 3. Structured summary — status forced to draft (HITL); per-block statuses
    //    live inside the sections JSON. Mirrors SummariesRepository::upsert_draft
    //    column list.
    sqlx::query(
        "INSERT INTO summaries \
         (id, meeting_id, workspace_id, status, model, template_id, sections, \
          generated_at, created_at, updated_at, updated_by, rev) \
         VALUES (?, ?, ?, 'draft', NULL, NULL, ?, ?, ?, ?, ?, 1)",
    )
    .bind(SAMPLE_SUMMARY_ID)
    .bind(SAMPLE_MEETING_ID)
    .bind(ctx.tenant_id.as_str())
    .bind(&sections_json)
    .bind(now)
    .bind(now)
    .bind(now)
    .bind(ctx.user_id.as_str())
    .execute(&mut *tx)
    .await
    .context("inserting sample structured summary")?;

    // 4. Action items (mirrors ActionItemsRepository::insert_drafts column list).
    for (position, action) in actions.iter().enumerate() {
        sqlx::query(
            "INSERT INTO action_items \
             (id, meeting_id, workspace_id, text, assignee, due, status, source_chunk_id, \
              position, created_at, updated_at, updated_by, rev) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 1)",
        )
        .bind(action.id)
        .bind(SAMPLE_MEETING_ID)
        .bind(ctx.tenant_id.as_str())
        .bind(action.text)
        .bind(action.assignee)
        .bind(action.due)
        .bind(block_status_to_db(action.status))
        .bind(action.source_chunk_id)
        .bind(position as i64)
        .bind(now)
        .bind(now)
        .bind(ctx.user_id.as_str())
        .execute(&mut *tx)
        .await
        .with_context(|| format!("inserting sample action item {}", action.id))?;
    }

    // 5. Legacy summary_processes row so api_get_summary / export work on the
    //    sample (mirrors summary::service's completed-process write shape).
    sqlx::query(
        "INSERT INTO summary_processes \
         (meeting_id, workspace_id, status, created_at, updated_at, updated_by, rev, \
          start_time, end_time, chunk_count, processing_time, result) \
         VALUES (?, ?, 'completed', ?, ?, ?, 1, ?, ?, 1, 0.0, ?)",
    )
    .bind(SAMPLE_MEETING_ID)
    .bind(ctx.tenant_id.as_str())
    .bind(now)
    .bind(now)
    .bind(ctx.user_id.as_str())
    .bind(now)
    .bind(now)
    .bind(&result_json)
    .execute(&mut *tx)
    .await
    .context("inserting sample summary process")?;

    tx.commit()
        .await
        .context("commit sample-seed transaction")?;

    info!(
        meeting_id = SAMPLE_MEETING_ID,
        segments = SEGMENTS.len(),
        actions = actions.len(),
        "seeded sample meeting for fresh install"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::AuthContext;
    use crate::database::repositories::action_item::ActionItemsRepository;
    use crate::database::repositories::meeting::MeetingsRepository;
    use crate::database::repositories::summary_draft::SummariesRepository;
    use crate::summary::draft::SummaryStatus;
    use sqlx::migrate::Migrator;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
    use std::collections::HashSet;

    /// The app's real, compile-time-embedded migration set (same source as
    /// `DatabaseManager::new`), with `foreign_keys` ON to match production so
    /// the meeting-FK cascade can be exercised.
    static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

    async fn open_migrated_pool() -> (SqlitePool, tempfile::TempDir) {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("sample-seed-test.sqlite");
        let options = SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(true)
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .expect("open temp sqlite db");
        MIGRATOR.run(&pool).await.expect("migrations must apply");
        (pool, dir)
    }

    async fn count(pool: &SqlitePool, sql: &str) -> i64 {
        sqlx::query_scalar(sql)
            .bind(SAMPLE_MEETING_ID)
            .fetch_one(pool)
            .await
            .expect("count query")
    }

    #[tokio::test]
    async fn seeds_one_source_linked_meeting_and_is_idempotent() {
        let (pool, _dir) = open_migrated_pool().await;
        let ctx = AuthContext::local();

        seed_sample_meeting(&pool, &ctx)
            .await
            .expect("seed must succeed");

        // Exactly one meeting, the sample.
        let meetings: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM meetings WHERE workspace_id = ?")
                .bind(ctx.tenant_id.as_str())
                .fetch_one(&pool)
                .await
                .expect("count meetings");
        assert_eq!(meetings, 1, "expected exactly one seeded meeting");

        // 14 transcript segments; collect their ids as the resolvable evidence set.
        let seg_ids: Vec<String> = sqlx::query_scalar(
            "SELECT id FROM transcripts WHERE meeting_id = ? AND workspace_id = ?",
        )
        .bind(SAMPLE_MEETING_ID)
        .bind(ctx.tenant_id.as_str())
        .fetch_all(&pool)
        .await
        .expect("fetch transcript ids");
        assert_eq!(seg_ids.len(), 14, "expected 14 transcript segments");
        let seg_set: HashSet<String> = seg_ids.into_iter().collect();

        // A draft structured summary whose EVERY block source resolves to a
        // seeded transcript (the api_get_summary_draft-equivalent read).
        let row = SummariesRepository::get_by_meeting(&pool, &ctx, SAMPLE_MEETING_ID)
            .await
            .expect("get_by_meeting ok")
            .expect("summary present");
        assert_eq!(row.status, SummaryStatus::Draft, "summary must be a draft");
        assert_eq!(row.draft.status, SummaryStatus::Draft);
        let mut block_count = 0usize;
        for section in &row.draft.sections {
            for block in &section.blocks {
                assert!(
                    seg_set.contains(&block.source_chunk_id),
                    "block {} cites unresolved source {}",
                    block.id,
                    block.source_chunk_id
                );
                block_count += 1;
            }
        }
        assert_eq!(block_count, 8, "expected 8 summary blocks");

        // The one edited block preserves its pre-edit text.
        let edited = row
            .draft
            .sections
            .iter()
            .flat_map(|s| &s.blocks)
            .find(|b| b.status == BlockStatus::Edited)
            .expect("one edited block");
        assert_eq!(
            edited.original_content.as_deref(),
            Some("Budget to finance.")
        );

        // 4 action items, all resolving to a seeded transcript.
        let items = ActionItemsRepository::list_by_meeting(&pool, &ctx, SAMPLE_MEETING_ID)
            .await
            .expect("list_by_meeting");
        assert_eq!(items.len(), 4, "expected 4 action items");
        for item in &items {
            assert!(
                seg_set.contains(&item.source_chunk_id),
                "action item {} cites unresolved source {}",
                item.id,
                item.source_chunk_id
            );
        }

        // Legacy summary_processes result carries non-empty markdown (export path).
        let result: String = sqlx::query_scalar(
            "SELECT result FROM summary_processes WHERE meeting_id = ? AND workspace_id = ?",
        )
        .bind(SAMPLE_MEETING_ID)
        .bind(ctx.tenant_id.as_str())
        .fetch_one(&pool)
        .await
        .expect("fetch summary_processes result");
        let parsed: serde_json::Value = serde_json::from_str(&result).expect("result must be JSON");
        let markdown = parsed
            .get("markdown")
            .and_then(|m| m.as_str())
            .expect("markdown field present");
        assert!(!markdown.is_empty(), "markdown must be non-empty");

        // Idempotency: re-seeding is a no-op (no dupes, no error).
        seed_sample_meeting(&pool, &ctx)
            .await
            .expect("second seed must be a no-op");
        let meetings_after: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM meetings WHERE workspace_id = ?")
                .bind(ctx.tenant_id.as_str())
                .fetch_one(&pool)
                .await
                .expect("count meetings again");
        assert_eq!(meetings_after, 1, "re-seed must not duplicate the meeting");
        assert_eq!(
            count(
                &pool,
                "SELECT COUNT(*) FROM transcripts WHERE meeting_id = ?"
            )
            .await,
            14
        );
        assert_eq!(
            count(
                &pool,
                "SELECT COUNT(*) FROM action_items WHERE meeting_id = ?"
            )
            .await,
            4
        );
    }

    /// The sample deletes cleanly through the normal `api_delete_meeting` path,
    /// and the `summaries` / `action_items` rows are removed by the meeting-FK
    /// ON DELETE CASCADE (foreign_keys ON), even though the repository's manual
    /// delete only touches transcript_chunks/summary_processes/transcripts.
    #[tokio::test]
    async fn sample_deletes_via_repository_with_fk_cascade() {
        let (pool, _dir) = open_migrated_pool().await;
        let ctx = AuthContext::local();
        seed_sample_meeting(&pool, &ctx)
            .await
            .expect("seed must succeed");

        let deleted = MeetingsRepository::delete_meeting(&pool, &ctx, SAMPLE_MEETING_ID)
            .await
            .expect("delete_meeting ok");
        assert!(deleted, "sample meeting must delete");

        assert_eq!(
            count(&pool, "SELECT COUNT(*) FROM meetings WHERE id = ?").await,
            0,
            "meeting row gone"
        );
        assert_eq!(
            count(
                &pool,
                "SELECT COUNT(*) FROM transcripts WHERE meeting_id = ?"
            )
            .await,
            0,
            "transcripts gone (manual delete)"
        );
        assert_eq!(
            count(&pool, "SELECT COUNT(*) FROM summaries WHERE meeting_id = ?").await,
            0,
            "summaries gone (FK cascade)"
        );
        assert_eq!(
            count(
                &pool,
                "SELECT COUNT(*) FROM action_items WHERE meeting_id = ?"
            )
            .await,
            0,
            "action_items gone (FK cascade)"
        );
        assert_eq!(
            count(
                &pool,
                "SELECT COUNT(*) FROM summary_processes WHERE meeting_id = ?"
            )
            .await,
            0,
            "summary_processes gone (manual delete)"
        );
    }
}
