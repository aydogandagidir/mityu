//! Integration tests for the C1 tenant-scoped summaries / action-items
//! repositories (BACKLOG C1.3 — docs/CONTRACTS.md §2 + §4, ADR-0019), against a
//! temp database migrated with the app's real migration set (harness cloned
//! from `repository_tenant_scoping.rs`).
//!
//! Proves:
//!   (a) upserts/reads are workspace-scoped in BOTH directions, and the
//!       summary-level status is FORCED to draft on write (generation may
//!       never produce approved);
//!   (b) every foreign-context write is refused (`Ok(false)` or a typed
//!       not-found error) and leaves the target rows byte-unchanged;
//!   (c) writes bump `rev` and stamp `updated_by` (from the AuthContext) plus
//!       an RFC 3339 `updated_at`;
//!   (d) cross-tenant/cross-meeting source injection: a draft citing a
//!       `transcripts` row of another workspace's meeting (or another meeting
//!       in the same workspace) is rejected at write time with a count-only
//!       typed error, persisting nothing;
//!   (e) approve paths re-validate evidence NOW: both `set_block_status ->
//!       Approved` and `approve_summary` refuse once the cited segment row is
//!       gone (simulated retranscription);
//!   (f) full happy path: upsert -> approve blocks -> `approve_summary` stamps
//!       `approved_at`/`approved_by` -> `edit_block` drops the summary back to
//!       draft, preserving `original_content`;
//!   (g) `replace_meeting_transcripts` triggers the ADR-0019 downgrade hook
//!       (approved summary + approved items revert to draft; edit provenance
//!       survives);
//!   (h) action items: insert/list scoping, and regeneration preserves
//!       approved/edited items while soft-deleting draft/rejected ones.

use sqlx::migrate::Migrator;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};
use std::collections::HashSet;

use app_lib::api::TranscriptSegment;
use app_lib::context::{AuthContext, RequestId, Role, TenantId, UserId, LOCAL_WORKSPACE_ID};
use app_lib::database::repositories::{
    action_item::{
        ActionItemsRepository, DEFAULT_ACTION_CENTER_PAGE_SIZE, MAX_ACTION_CENTER_PAGE_SIZE,
    },
    summary_draft::{SummariesRepository, SummaryDraftError},
    transcript::TranscriptsRepository,
};
use app_lib::summary::draft::{
    ActionItemDraft, BlockStatus, BlockType, DraftBlock, DraftSection, MeetingNotesDraft,
    SummaryStatus,
};

/// The app's real, compile-time-embedded migration set (same source as
/// `DatabaseManager::new`).
static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

const OTHER_WORKSPACE_ID: &str = "other-ws";

/// A second workspace's context, built directly (test-only): `context::current()`
/// deliberately resolves to the local workspace, and identity must never come
/// from anywhere else in production code.
fn other_ws_ctx() -> AuthContext {
    AuthContext {
        tenant_id: TenantId::new(OTHER_WORKSPACE_ID),
        user_id: UserId::new("other-user"),
        roles: vec![Role::Owner],
        request_id: RequestId::generate(),
    }
}

/// A SECOND user inside the local workspace (test-only), to prove that
/// `updated_by`/`approved_by` stamps come from the acting AuthContext, not a
/// constant.
fn reviewer_ctx() -> AuthContext {
    AuthContext {
        tenant_id: TenantId::new(LOCAL_WORKSPACE_ID),
        user_id: UserId::new("reviewer-user"),
        roles: vec![Role::Owner],
        request_id: RequestId::generate(),
    }
}

async fn open_migrated_temp_db(path: &std::path::Path) -> SqlitePool {
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .expect("failed to open temp sqlite database");
    MIGRATOR.run(&pool).await.expect("migrations must apply");
    pool
}

fn segment(id: &str, text: &str, start: f64, end: f64) -> TranscriptSegment {
    TranscriptSegment {
        id: id.to_string(),
        text: text.to_string(),
        timestamp: "2026-07-05T10:00:00.000Z".to_string(),
        audio_start_time: Some(start),
        audio_end_time: Some(end),
        duration: Some(end - start),
    }
}

/// Creates a meeting whose transcript segment ids are exactly `chunk_ids`
/// (preserved ids — they are the evidence anchors the drafts will cite).
async fn seed_meeting(
    pool: &SqlitePool,
    ctx: &AuthContext,
    title: &str,
    chunk_ids: &[&str],
) -> String {
    let segments: Vec<TranscriptSegment> = chunk_ids
        .iter()
        .enumerate()
        .map(|(i, id)| segment(id, &format!("segment {id}"), i as f64, i as f64 + 1.0))
        .collect();
    TranscriptsRepository::create_meeting_with_segments(pool, ctx, title, &segments, None)
        .await
        .expect("create_meeting_with_segments")
}

fn block(id: &str, chunk: &str, status: BlockStatus) -> DraftBlock {
    DraftBlock {
        id: id.to_string(),
        block_type: BlockType::Bullet,
        content: format!("generated content of {id}"),
        source_chunk_id: chunk.to_string(),
        status,
        original_content: None,
    }
}

fn notes(meeting_id: &str, blocks: Vec<DraftBlock>) -> MeetingNotesDraft {
    MeetingNotesDraft {
        meeting_id: meeting_id.to_string(),
        status: SummaryStatus::Draft,
        sections: vec![DraftSection {
            title: "Decisions".to_string(),
            blocks,
        }],
    }
}

fn action(id: &str, chunk: &str) -> ActionItemDraft {
    ActionItemDraft {
        id: id.to_string(),
        text: format!("do {id}"),
        assignee: None,
        due: None,
        status: BlockStatus::Draft,
        source_chunk_id: chunk.to_string(),
    }
}

fn assert_rfc3339(value: &Option<String>, what: &str) {
    let raw = value
        .as_deref()
        .unwrap_or_else(|| panic!("{what} must not be NULL"));
    chrono::DateTime::parse_from_rfc3339(raw)
        .unwrap_or_else(|e| panic!("{what} must be RFC 3339, got {raw:?}: {e}"));
}

/// Fetch (workspace_id, rev, updated_by, created_at, updated_at) for one row.
async fn sync_columns(
    pool: &SqlitePool,
    table: &str,
    key_col: &str,
    key: &str,
) -> (String, i64, Option<String>, Option<String>, Option<String>) {
    let row = sqlx::query(&format!(
        "SELECT workspace_id, rev, updated_by, created_at, updated_at \
         FROM {table} WHERE {key_col} = ?"
    ))
    .bind(key)
    .fetch_one(pool)
    .await
    .unwrap_or_else(|e| panic!("sync-column fetch on {table} failed: {e}"));
    (
        row.get::<String, _>("workspace_id"),
        row.get::<i64, _>("rev"),
        row.get::<Option<String>, _>("updated_by"),
        row.get::<Option<String>, _>("created_at"),
        row.get::<Option<String>, _>("updated_at"),
    )
}

/// Byte-level snapshot of a `summaries` row: every column concatenated into a
/// single string (NULLs encoded), so before/after comparison proves a refused
/// write touched NOTHING.
async fn summary_snapshot(pool: &SqlitePool, meeting_id: &str) -> String {
    sqlx::query_scalar::<_, String>(
        "SELECT id || '|' || meeting_id || '|' || workspace_id || '|' || status || '|' || \
         ifnull(model, '<null>') || '|' || ifnull(template_id, '<null>') || '|' || sections \
         || '|' || ifnull(generated_at, '<null>') || '|' || ifnull(approved_at, '<null>') \
         || '|' || ifnull(approved_by, '<null>') || '|' || created_at || '|' || updated_at \
         || '|' || ifnull(updated_by, '<null>') || '|' || CAST(rev AS TEXT) || '|' || \
         ifnull(deleted_at, '<null>') FROM summaries WHERE meeting_id = ?",
    )
    .bind(meeting_id)
    .fetch_one(pool)
    .await
    .expect("summaries snapshot row must exist")
}

/// Byte-level snapshot of an `action_items` row (same technique).
async fn item_snapshot(pool: &SqlitePool, item_id: &str) -> String {
    sqlx::query_scalar::<_, String>(
        "SELECT id || '|' || meeting_id || '|' || workspace_id || '|' || text || '|' || \
         ifnull(assignee, '<null>') || '|' || ifnull(due, '<null>') || '|' || status || '|' || \
         source_chunk_id || '|' || CAST(position AS TEXT) || '|' || \
         ifnull(original_text, '<null>') || '|' || created_at || '|' || updated_at || '|' || \
         ifnull(updated_by, '<null>') || '|' || CAST(rev AS TEXT) || '|' || \
         ifnull(deleted_at, '<null>') FROM action_items WHERE id = ?",
    )
    .bind(item_id)
    .fetch_one(pool)
    .await
    .expect("action_items snapshot row must exist")
}

/// Collect every block of a hydrated draft as (id, status, original_content).
fn block_states(draft: &MeetingNotesDraft) -> Vec<(String, BlockStatus, Option<String>)> {
    draft
        .sections
        .iter()
        .flat_map(|s| s.blocks.iter())
        .map(|b| (b.id.clone(), b.status, b.original_content.clone()))
        .collect()
}

/// (a) Upsert/insert under one workspace is invisible to the other, in both
/// directions — and the persisted summary-level status is FORCED to draft even
/// when the incoming draft (maliciously) claims approved.
#[tokio::test]
async fn upserts_and_reads_are_workspace_scoped_both_directions() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let pool = open_migrated_temp_db(&tmp.path().join("c1_reads.sqlite")).await;
    let local = AuthContext::local();
    let other = other_ws_ctx();

    let local_meeting = seed_meeting(&pool, &local, "Local standup", &["c-l1"]).await;
    let other_meeting = seed_meeting(&pool, &other, "Other tenant sync", &["c-o1"]).await;

    // Local draft claims summary-level Approved — the repository must force it
    // back to Draft (generation may never produce approved).
    let local_draft = MeetingNotesDraft {
        status: SummaryStatus::Approved,
        ..notes(
            &local_meeting,
            vec![block("b1", "c-l1", BlockStatus::Draft)],
        )
    };
    SummariesRepository::upsert_draft(
        &pool,
        &local,
        &local_draft,
        Some("llama3.2"),
        Some("std"),
        &[],
    )
    .await
    .expect("local upsert_draft");
    let other_draft = notes(
        &other_meeting,
        vec![block("b-o1", "c-o1", BlockStatus::Draft)],
    );
    SummariesRepository::upsert_draft(&pool, &other, &other_draft, None, None, &[])
        .await
        .expect("other upsert_draft");

    // Own-workspace read round-trips sections verbatim, with forced Draft status.
    let row = SummariesRepository::get_by_meeting(&pool, &local, &local_meeting)
        .await
        .expect("local get_by_meeting")
        .expect("local summary must exist");
    assert_eq!(
        row.status,
        SummaryStatus::Draft,
        "status must be FORCED to draft"
    );
    assert_eq!(row.draft.status, SummaryStatus::Draft);
    assert_eq!(
        row.draft.sections, local_draft.sections,
        "sections round-trip"
    );
    assert_eq!(row.model.as_deref(), Some("llama3.2"));
    assert_eq!(row.template_id.as_deref(), Some("std"));
    assert_eq!(row.approved_at, None);
    assert_eq!(row.rev, 1);

    // Cross-workspace reads: invisible in BOTH directions.
    assert!(
        SummariesRepository::get_by_meeting(&pool, &other, &local_meeting)
            .await
            .expect("scoped read")
            .is_none(),
        "other-ws must not see the local summary"
    );
    assert!(
        SummariesRepository::get_by_meeting(&pool, &local, &other_meeting)
            .await
            .expect("scoped read")
            .is_none(),
        "local must not see the other-ws summary"
    );

    // Action items: same isolation, both directions.
    ActionItemsRepository::insert_drafts(&pool, &local, &local_meeting, &[action("a1", "c-l1")])
        .await
        .expect("local insert_drafts");
    ActionItemsRepository::insert_drafts(&pool, &other, &other_meeting, &[action("a-o1", "c-o1")])
        .await
        .expect("other insert_drafts");

    let local_items = ActionItemsRepository::list_by_meeting(&pool, &local, &local_meeting)
        .await
        .expect("local list");
    assert_eq!(local_items.len(), 1);
    assert_eq!(local_items[0].id, "a1");
    assert_eq!(local_items[0].status, BlockStatus::Draft);
    assert_eq!(local_items[0].position, 0);
    assert!(
        ActionItemsRepository::list_by_meeting(&pool, &other, &local_meeting)
            .await
            .expect("scoped list")
            .is_empty(),
        "other-ws must not list local items"
    );
    assert!(
        ActionItemsRepository::list_by_meeting(&pool, &local, &other_meeting)
            .await
            .expect("scoped list")
            .is_empty(),
        "local must not list other-ws items"
    );

    pool.close().await;
}

/// (b) Every foreign-context write path is refused (`Ok(false)` or typed
/// not-found) and the target rows stay byte-identical.
#[tokio::test]
async fn foreign_context_writes_are_refused_and_rows_stay_byte_identical() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let pool = open_migrated_temp_db(&tmp.path().join("c1_foreign.sqlite")).await;
    let local = AuthContext::local();
    let other = other_ws_ctx();

    let meeting = seed_meeting(&pool, &local, "Target", &["c-1"]).await;
    SummariesRepository::upsert_draft(
        &pool,
        &local,
        &notes(&meeting, vec![block("b1", "c-1", BlockStatus::Draft)]),
        None,
        None,
        &[],
    )
    .await
    .expect("local upsert_draft");
    ActionItemsRepository::insert_drafts(&pool, &local, &meeting, &[action("a1", "c-1")])
        .await
        .expect("local insert_drafts");

    let summary_before = summary_snapshot(&pool, &meeting).await;
    let item_before = item_snapshot(&pool, "a1").await;

    // Creation paths: the meeting is invisible to the foreign workspace.
    let upsert = SummariesRepository::upsert_draft(
        &pool,
        &other,
        &notes(&meeting, vec![block("b-x", "c-1", BlockStatus::Draft)]),
        None,
        None,
        &[],
    )
    .await;
    assert!(
        matches!(upsert, Err(SummaryDraftError::MeetingNotFound)),
        "foreign upsert must fail closed, got {upsert:?}"
    );
    let insert =
        ActionItemsRepository::insert_drafts(&pool, &other, &meeting, &[action("a-x", "c-1")])
            .await;
    assert!(
        matches!(insert, Err(SummaryDraftError::MeetingNotFound)),
        "foreign insert_drafts must fail closed, got {insert:?}"
    );

    // Mutation paths: all silent no-ops (B2/ADR-0010 convention -> Phase-2 404).
    assert!(!SummariesRepository::set_block_status(
        &pool,
        &other,
        &meeting,
        "b1",
        BlockStatus::Approved,
        None
    )
    .await
    .expect("foreign set_block_status runs"));
    assert!(
        !SummariesRepository::edit_block(&pool, &other, &meeting, "b1", "hacked")
            .await
            .expect("foreign edit_block runs")
    );
    assert!(
        !SummariesRepository::approve_summary(&pool, &other, &meeting)
            .await
            .expect("foreign approve_summary runs")
    );
    assert!(
        !SummariesRepository::downgrade_to_draft(&pool, &other, &meeting)
            .await
            .expect("foreign summary downgrade runs")
    );
    assert!(!SummariesRepository::soft_delete(&pool, &other, &meeting)
        .await
        .expect("foreign summary soft_delete runs"));
    assert!(
        !ActionItemsRepository::set_status(&pool, &other, "a1", BlockStatus::Approved, None)
            .await
            .expect("foreign set_status runs")
    );
    assert!(
        !ActionItemsRepository::edit(&pool, &other, "a1", Some("hacked"), None, None)
            .await
            .expect("foreign edit runs")
    );
    assert!(
        !ActionItemsRepository::downgrade_to_draft(&pool, &other, &meeting)
            .await
            .expect("foreign items downgrade runs")
    );
    assert!(!ActionItemsRepository::soft_delete(&pool, &other, "a1")
        .await
        .expect("foreign item soft_delete runs"));

    // Byte-identical rows, and no stray rows were created.
    assert_eq!(
        summary_snapshot(&pool, &meeting).await,
        summary_before,
        "summary row must be byte-unchanged after foreign write attempts"
    );
    assert_eq!(
        item_snapshot(&pool, "a1").await,
        item_before,
        "action item row must be byte-unchanged after foreign write attempts"
    );
    let summary_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM summaries")
        .fetch_one(&pool)
        .await
        .expect("count summaries");
    let item_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM action_items")
        .fetch_one(&pool)
        .await
        .expect("count action_items");
    assert_eq!(summary_count, 1);
    assert_eq!(item_count, 1);

    pool.close().await;
}

/// (c) Writes bump `rev` and stamp `updated_by` from the ACTING context plus an
/// RFC 3339 `updated_at`; regeneration re-forces draft status.
#[tokio::test]
async fn writes_bump_rev_and_stamp_updated_by_and_rfc3339_timestamps() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let pool = open_migrated_temp_db(&tmp.path().join("c1_stamps.sqlite")).await;
    let local = AuthContext::local();
    let reviewer = reviewer_ctx();

    let meeting = seed_meeting(&pool, &local, "Stamped", &["c-1"]).await;
    let summary_id = SummariesRepository::upsert_draft(
        &pool,
        &local,
        &notes(&meeting, vec![block("b1", "c-1", BlockStatus::Draft)]),
        None,
        None,
        &[],
    )
    .await
    .expect("upsert_draft");

    let (ws, rev, updated_by, created_at, updated_at) =
        sync_columns(&pool, "summaries", "id", &summary_id).await;
    assert_eq!(ws, LOCAL_WORKSPACE_ID);
    assert_eq!(rev, 1, "fresh insert must carry rev = 1");
    assert_eq!(updated_by.as_deref(), Some("local-user"));
    assert_rfc3339(&created_at, "summaries.created_at");
    assert_rfc3339(&updated_at, "summaries.updated_at");
    let generated_at: Option<String> =
        sqlx::query_scalar("SELECT generated_at FROM summaries WHERE id = ?")
            .bind(&summary_id)
            .fetch_one(&pool)
            .await
            .expect("generated_at");
    assert_rfc3339(&generated_at, "summaries.generated_at");

    // A block write by a DIFFERENT local user stamps THAT user (identity comes
    // from the AuthContext, never a constant).
    assert!(SummariesRepository::set_block_status(
        &pool,
        &reviewer,
        &meeting,
        "b1",
        BlockStatus::Approved,
        None
    )
    .await
    .expect("reviewer set_block_status"));
    let (_, rev, updated_by, _, updated_at) =
        sync_columns(&pool, "summaries", "id", &summary_id).await;
    assert_eq!(rev, 2, "block write must bump rev");
    assert_eq!(updated_by.as_deref(), Some("reviewer-user"));
    assert_rfc3339(&updated_at, "summaries.updated_at after block write");
    assert!(
        updated_at.as_deref().unwrap().contains('T'),
        "updated_at must use the RFC 3339 'T' separator, got {updated_at:?}"
    );

    // Regeneration: same row (UNIQUE meeting_id), bumped rev, re-forced draft.
    let regenerated_id = SummariesRepository::upsert_draft(
        &pool,
        &local,
        &notes(&meeting, vec![block("b2", "c-1", BlockStatus::Draft)]),
        Some("claude-3-5"),
        None,
        &[],
    )
    .await
    .expect("regeneration upsert");
    assert_eq!(
        regenerated_id, summary_id,
        "regeneration must reuse the row"
    );
    let (_, rev, _, _, _) = sync_columns(&pool, "summaries", "id", &summary_id).await;
    assert_eq!(rev, 3, "regeneration must bump rev");
    let status: String = sqlx::query_scalar("SELECT status FROM summaries WHERE id = ?")
        .bind(&summary_id)
        .fetch_one(&pool)
        .await
        .expect("status");
    assert_eq!(status, "draft");

    // Action items: insert stamps rev = 1; an edit by the reviewer bumps rev
    // and re-stamps updated_by.
    ActionItemsRepository::insert_drafts(&pool, &local, &meeting, &[action("a1", "c-1")])
        .await
        .expect("insert_drafts");
    let (ws, rev, updated_by, created_at, updated_at) =
        sync_columns(&pool, "action_items", "id", "a1").await;
    assert_eq!(ws, LOCAL_WORKSPACE_ID);
    assert_eq!(rev, 1);
    assert_eq!(updated_by.as_deref(), Some("local-user"));
    assert_rfc3339(&created_at, "action_items.created_at");
    assert_rfc3339(&updated_at, "action_items.updated_at");

    assert!(
        ActionItemsRepository::edit(&pool, &reviewer, "a1", Some("refined"), None, None)
            .await
            .expect("reviewer edit")
    );
    let (_, rev, updated_by, _, updated_at) = sync_columns(&pool, "action_items", "id", "a1").await;
    assert_eq!(rev, 2, "item edit must bump rev");
    assert_eq!(updated_by.as_deref(), Some("reviewer-user"));
    assert_rfc3339(&updated_at, "action_items.updated_at after edit");

    pool.close().await;
}

/// (d) Cross-tenant (and cross-meeting) source injection is rejected at write
/// time: the count-only typed error names how many DISTINCT cited ids failed
/// to resolve, and nothing is persisted.
#[tokio::test]
async fn cross_tenant_source_injection_is_rejected_at_write_time() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let pool = open_migrated_temp_db(&tmp.path().join("c1_injection.sqlite")).await;
    let local = AuthContext::local();
    let other = other_ws_ctx();

    let local_meeting = seed_meeting(&pool, &local, "Local", &["c-local"]).await;
    let local_meeting_2 = seed_meeting(&pool, &local, "Local 2", &["c-local2"]).await;
    let _other_meeting = seed_meeting(&pool, &other, "Foreign", &["c-foreign"]).await;

    // One valid + one foreign-workspace citation -> exactly 1 unresolvable.
    let injected = notes(
        &local_meeting,
        vec![
            block("b1", "c-local", BlockStatus::Draft),
            block("b2", "c-foreign", BlockStatus::Draft),
        ],
    );
    match SummariesRepository::upsert_draft(&pool, &local, &injected, None, None, &[]).await {
        Err(SummaryDraftError::UnresolvableSources { count }) => assert_eq!(count, 1),
        other => panic!("expected UnresolvableSources, got {other:?}"),
    }

    // Foreign + nonexistent citations -> 2 distinct unresolvable ids.
    let doubly_bad = notes(
        &local_meeting,
        vec![
            block("b1", "c-foreign", BlockStatus::Draft),
            block("b2", "c-nonexistent", BlockStatus::Draft),
        ],
    );
    match SummariesRepository::upsert_draft(&pool, &local, &doubly_bad, None, None, &[]).await {
        Err(SummaryDraftError::UnresolvableSources { count }) => assert_eq!(count, 2),
        other => panic!("expected UnresolvableSources, got {other:?}"),
    }

    // Same workspace but ANOTHER meeting's segment: also not resolvable
    // (resolvable = same meeting AND same workspace).
    let wrong_meeting = notes(
        &local_meeting,
        vec![block("b1", "c-local2", BlockStatus::Draft)],
    );
    match SummariesRepository::upsert_draft(&pool, &local, &wrong_meeting, None, None, &[]).await {
        Err(SummaryDraftError::UnresolvableSources { count }) => assert_eq!(count, 1),
        other => panic!("expected UnresolvableSources, got {other:?}"),
    }
    // local_meeting_2 exists only to own c-local2.
    assert_ne!(local_meeting, local_meeting_2);

    // Nothing was persisted by any rejected upsert.
    assert!(
        SummariesRepository::get_by_meeting(&pool, &local, &local_meeting)
            .await
            .expect("get_by_meeting")
            .is_none(),
        "a rejected upsert must persist nothing"
    );

    // Action items: same gate, same counting, nothing persisted.
    match ActionItemsRepository::insert_drafts(
        &pool,
        &local,
        &local_meeting,
        &[action("a1", "c-local"), action("a2", "c-foreign")],
    )
    .await
    {
        Err(SummaryDraftError::UnresolvableSources { count }) => assert_eq!(count, 1),
        other => panic!("expected UnresolvableSources, got {other:?}"),
    }
    let item_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM action_items")
        .fetch_one(&pool)
        .await
        .expect("count action_items");
    assert_eq!(item_count, 0, "a rejected insert must persist nothing");

    pool.close().await;
}

/// (e) Approve paths re-validate evidence NOW: once the cited segment row is
/// gone (simulated retranscription deletion), `set_block_status -> Approved`,
/// `approve_summary`, and item `set_status -> Approved` all refuse.
#[tokio::test]
async fn approvals_refuse_when_cited_segment_is_gone() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let pool = open_migrated_temp_db(&tmp.path().join("c1_stale.sqlite")).await;
    let local = AuthContext::local();

    let meeting = seed_meeting(&pool, &local, "Stale evidence", &["c-1", "c-2"]).await;
    SummariesRepository::upsert_draft(
        &pool,
        &local,
        &notes(
            &meeting,
            vec![
                block("b1", "c-1", BlockStatus::Draft),
                block("b2", "c-2", BlockStatus::Draft),
            ],
        ),
        None,
        None,
        &[],
    )
    .await
    .expect("upsert_draft");
    ActionItemsRepository::insert_drafts(&pool, &local, &meeting, &[action("a1", "c-2")])
        .await
        .expect("insert_drafts");

    // While evidence exists, approval works.
    assert!(SummariesRepository::set_block_status(
        &pool,
        &local,
        &meeting,
        "b1",
        BlockStatus::Approved,
        None
    )
    .await
    .expect("approve b1"));

    // Simulate retranscription having dropped c-2.
    sqlx::query("DELETE FROM transcripts WHERE id = 'c-2'")
        .execute(&pool)
        .await
        .expect("delete c-2");

    assert!(
        !SummariesRepository::set_block_status(
            &pool,
            &local,
            &meeting,
            "b2",
            BlockStatus::Approved,
            None
        )
        .await
        .expect("stale block approve runs"),
        "approving a block whose segment is gone must refuse"
    );
    assert!(
        !ActionItemsRepository::set_status(&pool, &local, "a1", BlockStatus::Approved, None)
            .await
            .expect("stale item approve runs"),
        "approving an item whose segment is gone must refuse"
    );

    // Take b2 out of the gate via rejection, then break b1's evidence too:
    // approve_summary must re-validate AT THIS MOMENT and refuse.
    assert!(SummariesRepository::set_block_status(
        &pool,
        &local,
        &meeting,
        "b2",
        BlockStatus::Rejected,
        None
    )
    .await
    .expect("reject b2"));
    sqlx::query("DELETE FROM transcripts WHERE id = 'c-1'")
        .execute(&pool)
        .await
        .expect("delete c-1");
    assert!(
        !SummariesRepository::approve_summary(&pool, &local, &meeting)
            .await
            .expect("stale approve_summary runs"),
        "approve_summary must refuse when an approved block's segment is gone"
    );

    // The refusals changed nothing: summary still draft, b2 rejected, b1 approved.
    let row = SummariesRepository::get_by_meeting(&pool, &local, &meeting)
        .await
        .expect("get_by_meeting")
        .expect("summary row");
    assert_eq!(row.status, SummaryStatus::Draft);
    assert_eq!(row.approved_at, None);
    assert_eq!(
        block_states(&row.draft),
        vec![
            ("b1".to_string(), BlockStatus::Approved, None),
            ("b2".to_string(), BlockStatus::Rejected, None),
        ]
    );

    pool.close().await;
}

/// (f) Full happy path: upsert -> approve blocks (rejected ones excluded from
/// the gate) -> approve_summary stamps approved_at/approved_by -> edit_block
/// drops the summary back to draft and preserves original_content.
#[tokio::test]
async fn happy_path_approve_summary_then_edit_drops_back_to_draft() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let pool = open_migrated_temp_db(&tmp.path().join("c1_happy.sqlite")).await;
    let local = AuthContext::local();

    let meeting = seed_meeting(&pool, &local, "Happy path", &["c-1", "c-2", "c-3"]).await;
    SummariesRepository::upsert_draft(
        &pool,
        &local,
        &notes(
            &meeting,
            vec![
                block("b1", "c-1", BlockStatus::Draft),
                block("b2", "c-2", BlockStatus::Draft),
                block("b3", "c-3", BlockStatus::Draft),
            ],
        ),
        Some("llama3.2"),
        None,
        &[],
    )
    .await
    .expect("upsert_draft");

    // Gate: unapproved blocks block the summary approval.
    assert!(
        !SummariesRepository::approve_summary(&pool, &local, &meeting)
            .await
            .expect("premature approve runs"),
        "approve_summary must refuse while non-rejected blocks are unapproved"
    );

    assert!(SummariesRepository::set_block_status(
        &pool,
        &local,
        &meeting,
        "b1",
        BlockStatus::Approved,
        None
    )
    .await
    .expect("approve b1"));
    assert!(SummariesRepository::set_block_status(
        &pool,
        &local,
        &meeting,
        "b2",
        BlockStatus::Approved,
        None
    )
    .await
    .expect("approve b2"));
    assert!(SummariesRepository::set_block_status(
        &pool,
        &local,
        &meeting,
        "b3",
        BlockStatus::Rejected,
        None
    )
    .await
    .expect("reject b3"));
    // Illegal self-transition: approved -> approved.
    assert!(
        !SummariesRepository::set_block_status(
            &pool,
            &local,
            &meeting,
            "b1",
            BlockStatus::Approved,
            None
        )
        .await
        .expect("re-approve runs"),
        "approved -> approved must be an illegal transition"
    );

    assert!(
        SummariesRepository::approve_summary(&pool, &local, &meeting)
            .await
            .expect("approve_summary")
    );
    let row = SummariesRepository::get_by_meeting(&pool, &local, &meeting)
        .await
        .expect("get_by_meeting")
        .expect("summary row");
    assert_eq!(row.status, SummaryStatus::Approved);
    assert_eq!(row.approved_by.as_deref(), Some("local-user"));
    assert_rfc3339(&row.approved_at, "summaries.approved_at");

    // A human edit re-opens the summary: status drops to draft, approval
    // stamps clear, the block turns Edited and keeps its generated original.
    assert!(
        SummariesRepository::edit_block(&pool, &local, &meeting, "b2", "human-improved text")
            .await
            .expect("edit b2")
    );
    let row = SummariesRepository::get_by_meeting(&pool, &local, &meeting)
        .await
        .expect("get_by_meeting")
        .expect("summary row");
    assert_eq!(
        row.status,
        SummaryStatus::Draft,
        "edit must re-open the summary"
    );
    assert_eq!(row.approved_at, None);
    assert_eq!(row.approved_by, None);
    let b2 = row
        .draft
        .sections
        .iter()
        .flat_map(|s| s.blocks.iter())
        .find(|b| b.id == "b2")
        .expect("b2 present");
    assert_eq!(b2.status, BlockStatus::Edited);
    assert_eq!(b2.content, "human-improved text");
    assert_eq!(
        b2.original_content.as_deref(),
        Some("generated content of b2"),
        "first edit must preserve the generated text"
    );
    assert_eq!(
        b2.source_chunk_id, "c-2",
        "edit must NEVER touch the evidence anchor"
    );

    // Second edit keeps the FIRST original; editing a rejected block refuses.
    assert!(
        SummariesRepository::edit_block(&pool, &local, &meeting, "b2", "second pass")
            .await
            .expect("re-edit b2")
    );
    let row = SummariesRepository::get_by_meeting(&pool, &local, &meeting)
        .await
        .expect("get_by_meeting")
        .expect("summary row");
    let b2 = row
        .draft
        .sections
        .iter()
        .flat_map(|s| s.blocks.iter())
        .find(|b| b.id == "b2")
        .expect("b2 present");
    assert_eq!(
        b2.original_content.as_deref(),
        Some("generated content of b2")
    );
    assert!(
        !SummariesRepository::edit_block(&pool, &local, &meeting, "b3", "necromancy")
            .await
            .expect("edit rejected runs"),
        "a rejected block must be restored to draft before editing"
    );

    // Re-approve after the edit round-trip: edited -> approved is legal.
    assert!(SummariesRepository::set_block_status(
        &pool,
        &local,
        &meeting,
        "b2",
        BlockStatus::Approved,
        None
    )
    .await
    .expect("approve edited b2"));
    assert!(
        SummariesRepository::approve_summary(&pool, &local, &meeting)
            .await
            .expect("re-approve summary")
    );

    pool.close().await;
}

/// (g) `replace_meeting_transcripts` triggers the ADR-0019 downgrade hook: the
/// approved summary and approved items revert to draft (approval stamps
/// cleared), and edit provenance (`original_content`/`original_text`) survives
/// the downgrade.
#[tokio::test]
async fn retranscription_triggers_downgrade_of_summary_and_items() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let pool = open_migrated_temp_db(&tmp.path().join("c1_retranscribe.sqlite")).await;
    let local = AuthContext::local();

    let meeting = seed_meeting(&pool, &local, "Retranscribed", &["c-1", "c-2"]).await;
    SummariesRepository::upsert_draft(
        &pool,
        &local,
        &notes(
            &meeting,
            vec![
                block("b1", "c-1", BlockStatus::Draft),
                block("b2", "c-2", BlockStatus::Draft),
            ],
        ),
        None,
        None,
        &[],
    )
    .await
    .expect("upsert_draft");
    for id in ["b1", "b2"] {
        assert!(SummariesRepository::set_block_status(
            &pool,
            &local,
            &meeting,
            id,
            BlockStatus::Approved,
            None
        )
        .await
        .expect("approve block"));
    }
    assert!(
        SummariesRepository::approve_summary(&pool, &local, &meeting)
            .await
            .expect("approve_summary")
    );
    ActionItemsRepository::insert_drafts(&pool, &local, &meeting, &[action("a1", "c-1")])
        .await
        .expect("insert_drafts");
    assert!(
        ActionItemsRepository::set_status(&pool, &local, "a1", BlockStatus::Approved, None)
            .await
            .expect("approve a1")
    );
    let action_center = ActionItemsRepository::list_approved_with_sources(&pool, &local, 100, 0)
        .await
        .expect("list approved actions before retranscription");
    assert_eq!(
        action_center.items.len(),
        1,
        "approved item must enter the Action Center"
    );

    // Retranscription: new segment rows, new ids.
    TranscriptsRepository::replace_meeting_transcripts(
        &pool,
        &local,
        &meeting,
        &[segment("c-9", "retranscribed", 0.0, 2.0)],
    )
    .await
    .expect("replace_meeting_transcripts");

    // Hook fired: summary back to draft, approval stamps cleared, blocks draft.
    let row = SummariesRepository::get_by_meeting(&pool, &local, &meeting)
        .await
        .expect("get_by_meeting")
        .expect("summary row");
    assert_eq!(
        row.status,
        SummaryStatus::Draft,
        "hook must downgrade the summary"
    );
    assert_eq!(row.approved_at, None);
    assert_eq!(row.approved_by, None);
    assert_eq!(
        block_states(&row.draft),
        vec![
            ("b1".to_string(), BlockStatus::Draft, None),
            ("b2".to_string(), BlockStatus::Draft, None),
        ]
    );
    let items = ActionItemsRepository::list_by_meeting(&pool, &local, &meeting)
        .await
        .expect("list items");
    assert_eq!(items.len(), 1);
    assert_eq!(
        items[0].status,
        BlockStatus::Draft,
        "hook must downgrade items"
    );
    assert!(
        ActionItemsRepository::list_approved_with_sources(&pool, &local, 100, 0)
            .await
            .expect("list approved actions after retranscription")
            .items
            .is_empty(),
        "retranscription must remove the downgraded item from the Action Center"
    );

    // Edit provenance survives a subsequent retranscription: Edited -> Draft
    // keeps original_content / original_text.
    assert!(
        SummariesRepository::edit_block(&pool, &local, &meeting, "b1", "edited after regen")
            .await
            .expect("edit b1")
    );
    assert!(
        ActionItemsRepository::edit(&pool, &local, "a1", Some("edited item"), None, None)
            .await
            .expect("edit a1")
    );
    TranscriptsRepository::replace_meeting_transcripts(
        &pool,
        &local,
        &meeting,
        &[segment("c-10", "retranscribed again", 0.0, 2.0)],
    )
    .await
    .expect("second replace");

    let row = SummariesRepository::get_by_meeting(&pool, &local, &meeting)
        .await
        .expect("get_by_meeting")
        .expect("summary row");
    let b1 = row
        .draft
        .sections
        .iter()
        .flat_map(|s| s.blocks.iter())
        .find(|b| b.id == "b1")
        .expect("b1 present");
    assert_eq!(b1.status, BlockStatus::Draft);
    assert_eq!(b1.content, "edited after regen", "edited content survives");
    assert_eq!(
        b1.original_content.as_deref(),
        Some("generated content of b1"),
        "downgrade must keep original_content"
    );
    let items = ActionItemsRepository::list_by_meeting(&pool, &local, &meeting)
        .await
        .expect("list items");
    assert_eq!(items[0].status, BlockStatus::Draft);
    assert_eq!(items[0].text, "edited item");
    assert_eq!(
        items[0].original_text.as_deref(),
        Some("do a1"),
        "downgrade must keep original_text"
    );

    pool.close().await;
}

/// (h) Action-item regeneration: approved/edited items survive; draft/rejected
/// ones are soft-deleted and replaced by the new batch.
#[tokio::test]
async fn action_item_regeneration_preserves_human_touched_items() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let pool = open_migrated_temp_db(&tmp.path().join("c1_regen.sqlite")).await;
    let local = AuthContext::local();
    let other = other_ws_ctx();

    let meeting = seed_meeting(&pool, &local, "Regen", &["c-1"]).await;
    let generation_1 = [
        action("a1", "c-1"),
        action("a2", "c-1"),
        action("a3", "c-1"),
        action("a4", "c-1"),
    ];
    assert_eq!(
        ActionItemsRepository::insert_drafts(&pool, &local, &meeting, &generation_1)
            .await
            .expect("generation 1"),
        4
    );

    // Human touches: approve a1, edit a2 (with assignee patch), reject a4.
    assert!(
        ActionItemsRepository::set_status(&pool, &local, "a1", BlockStatus::Approved, None)
            .await
            .expect("approve a1")
    );
    assert!(ActionItemsRepository::edit(
        &pool,
        &local,
        "a2",
        Some("refined a2"),
        Some(Some("ayse")),
        None
    )
    .await
    .expect("edit a2"));
    assert!(
        ActionItemsRepository::set_status(&pool, &local, "a4", BlockStatus::Rejected, None)
            .await
            .expect("reject a4")
    );

    // Regeneration: a3 (draft) and a4 (rejected) are replaced; a1/a2 survive.
    let generation_2 = [action("b1", "c-1"), action("b2", "c-1")];
    assert_eq!(
        ActionItemsRepository::insert_drafts(&pool, &local, &meeting, &generation_2)
            .await
            .expect("generation 2"),
        2
    );

    let items = ActionItemsRepository::list_by_meeting(&pool, &local, &meeting)
        .await
        .expect("list");
    let ids: HashSet<&str> = items.iter().map(|item| item.id.as_str()).collect();
    assert_eq!(
        ids,
        HashSet::from(["a1", "a2", "b1", "b2"]),
        "approved/edited survive, draft/rejected are replaced"
    );
    let by_id = |id: &str| items.iter().find(|item| item.id == id).unwrap();
    assert_eq!(by_id("a1").status, BlockStatus::Approved);
    assert_eq!(by_id("a1").text, "do a1", "kept items are untouched");
    assert_eq!(by_id("a2").status, BlockStatus::Edited);
    assert_eq!(by_id("a2").text, "refined a2");
    assert_eq!(by_id("a2").assignee.as_deref(), Some("ayse"));
    assert_eq!(
        by_id("a2").original_text.as_deref(),
        Some("do a2"),
        "first edit must preserve the generated text"
    );
    assert_eq!(by_id("b1").status, BlockStatus::Draft);
    assert_eq!((by_id("b1").position, by_id("b2").position), (0, 1));

    // The replaced rows are SOFT-deleted (audit trail kept), not removed.
    for id in ["a3", "a4"] {
        let deleted_at: Option<String> =
            sqlx::query_scalar("SELECT deleted_at FROM action_items WHERE id = ?")
                .bind(id)
                .fetch_one(&pool)
                .await
                .expect("replaced row still exists");
        assert!(
            deleted_at.is_some(),
            "{id} must be soft-deleted by regeneration"
        );
    }

    // Scoping: the foreign workspace sees nothing and cannot regenerate.
    assert!(
        ActionItemsRepository::list_by_meeting(&pool, &other, &meeting)
            .await
            .expect("foreign list")
            .is_empty()
    );
    assert!(
        matches!(
            ActionItemsRepository::insert_drafts(&pool, &other, &meeting, &generation_2).await,
            Err(SummaryDraftError::MeetingNotFound)
        ),
        "foreign regeneration must fail closed"
    );

    // Per-item soft delete is scoped and reported honestly.
    assert!(ActionItemsRepository::soft_delete(&pool, &local, "b2")
        .await
        .expect("soft_delete b2"));
    assert!(
        !ActionItemsRepository::soft_delete(&pool, &local, "b2")
            .await
            .expect("second soft_delete runs"),
        "soft-deleting an already-deleted item must report no match"
    );
    let items = ActionItemsRepository::list_by_meeting(&pool, &local, &meeting)
        .await
        .expect("list after delete");
    assert!(
        items.iter().all(|item| item.id != "b2"),
        "soft-deleted item must vanish from listings"
    );

    pool.close().await;
}

/// Generation is an untrusted producer: repository persistence must never let
/// an incoming model payload mint any human-review state.
#[tokio::test]
async fn generation_forces_summary_blocks_and_action_items_to_draft() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let pool = open_migrated_temp_db(&tmp.path().join("action_status_gate.sqlite")).await;
    let local = AuthContext::local();
    let meeting = seed_meeting(&pool, &local, "Status gate", &["status-source"]).await;

    let mut approved_block = block(
        "incoming-approved-block",
        "status-source",
        BlockStatus::Approved,
    );
    approved_block.original_content = Some("forged edit provenance".to_string());
    let generated_summary = MeetingNotesDraft {
        status: SummaryStatus::Approved,
        ..notes(
            &meeting,
            vec![
                approved_block,
                block(
                    "incoming-edited-block",
                    "status-source",
                    BlockStatus::Edited,
                ),
                block(
                    "incoming-rejected-block",
                    "status-source",
                    BlockStatus::Rejected,
                ),
            ],
        )
    };
    SummariesRepository::upsert_draft(&pool, &local, &generated_summary, None, None, &[])
        .await
        .expect("insert untrusted summary generation");
    let summary = SummariesRepository::get_by_meeting(&pool, &local, &meeting)
        .await
        .expect("read forced summary draft")
        .expect("summary exists");
    assert_eq!(summary.status, SummaryStatus::Draft);
    assert!(
        summary
            .draft
            .sections
            .iter()
            .flat_map(|section| section.blocks.iter())
            .all(|block| {
                block.status == BlockStatus::Draft && block.original_content.is_none()
            }),
        "repository boundary must clear producer-supplied review state and edit provenance"
    );

    let mut approved = action("incoming-approved", "status-source");
    approved.status = BlockStatus::Approved;
    let mut edited = action("incoming-edited", "status-source");
    edited.status = BlockStatus::Edited;
    let mut rejected = action("incoming-rejected", "status-source");
    rejected.status = BlockStatus::Rejected;

    ActionItemsRepository::insert_drafts(&pool, &local, &meeting, &[approved, edited, rejected])
        .await
        .expect("insert untrusted generation batch");

    let items = ActionItemsRepository::list_by_meeting(&pool, &local, &meeting)
        .await
        .expect("list forced drafts");
    assert_eq!(items.len(), 3);
    assert!(
        items.iter().all(|item| item.status == BlockStatus::Draft),
        "repository boundary must force every generated item to draft"
    );
    assert!(
        ActionItemsRepository::list_approved_with_sources(&pool, &local, 100, 0)
            .await
            .expect("list approved actions")
            .items
            .is_empty(),
        "untrusted generation must not enter the Action Center"
    );

    pool.close().await;
}

/// "Resolvable" means an active source in an active same-workspace meeting,
/// both at draft insertion and at the atomic human-approval write gate.
#[tokio::test]
async fn action_item_write_and_approval_gates_require_active_evidence() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let pool = open_migrated_temp_db(&tmp.path().join("action_active_source.sqlite")).await;
    let local = AuthContext::local();
    let meeting = seed_meeting(&pool, &local, "Active evidence", &["active-source"]).await;

    sqlx::query("UPDATE transcripts SET deleted_at = ? WHERE id = ?")
        .bind("2026-07-14T10:00:00Z")
        .bind("active-source")
        .execute(&pool)
        .await
        .expect("soft-delete source");
    let insert = ActionItemsRepository::insert_drafts(
        &pool,
        &local,
        &meeting,
        &[action("source-deleted-at-insert", "active-source")],
    )
    .await;
    assert!(
        matches!(
            insert,
            Err(SummaryDraftError::UnresolvableSources { count: 1 })
        ),
        "soft-deleted evidence must fail the write-time source gate: {insert:?}"
    );

    sqlx::query("UPDATE transcripts SET deleted_at = NULL WHERE id = ?")
        .bind("active-source")
        .execute(&pool)
        .await
        .expect("restore source");
    ActionItemsRepository::insert_drafts(
        &pool,
        &local,
        &meeting,
        &[action("approval-target", "active-source")],
    )
    .await
    .expect("insert approval target");

    sqlx::query("UPDATE transcripts SET deleted_at = ? WHERE id = ?")
        .bind("2026-07-14T10:01:00Z")
        .bind("active-source")
        .execute(&pool)
        .await
        .expect("soft-delete source before approval");
    assert!(
        !ActionItemsRepository::set_status(
            &pool,
            &local,
            "approval-target",
            BlockStatus::Approved,
            None,
        )
        .await
        .expect("approval with deleted source returns a decision"),
        "approval must fail atomically when the source is inactive"
    );

    sqlx::query("UPDATE transcripts SET deleted_at = NULL WHERE id = ?")
        .bind("active-source")
        .execute(&pool)
        .await
        .expect("restore source again");
    sqlx::query("UPDATE meetings SET deleted_at = ? WHERE id = ?")
        .bind("2026-07-14T10:02:00Z")
        .bind(&meeting)
        .execute(&pool)
        .await
        .expect("soft-delete meeting");
    assert!(
        !ActionItemsRepository::set_status(
            &pool,
            &local,
            "approval-target",
            BlockStatus::Approved,
            None,
        )
        .await
        .expect("approval with deleted meeting returns a decision"),
        "approval must fail atomically when the meeting is inactive"
    );
    let insert = ActionItemsRepository::insert_drafts(
        &pool,
        &local,
        &meeting,
        &[action("meeting-deleted-at-insert", "active-source")],
    )
    .await;
    assert!(
        matches!(insert, Err(SummaryDraftError::MeetingNotFound)),
        "soft-deleted meetings must reject new generation batches: {insert:?}"
    );

    pool.close().await;
}

/// The Action Center is a bounded, deterministic, tenant-scoped projection of
/// approved items whose meeting/source joins are still active and consistent.
#[tokio::test]
async fn approved_action_center_filters_untrusted_states_and_preserves_source_links() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let pool = open_migrated_temp_db(&tmp.path().join("approved_action_center.sqlite")).await;
    let local = AuthContext::local();
    let other = other_ws_ctx();

    let newer = seed_meeting(
        &pool,
        &local,
        "Newer planning",
        &["new-source", "soft-source"],
    )
    .await;
    let older = seed_meeting(&pool, &local, "Older planning", &["old-source"]).await;
    let deleted_meeting = seed_meeting(
        &pool,
        &local,
        "Deleted meeting",
        &["deleted-meeting-source"],
    )
    .await;
    let other_meeting = seed_meeting(&pool, &other, "Other tenant", &["other-source"]).await;

    for (meeting_id, created_at) in [
        (&newer, "2026-07-03T09:00:00Z"),
        (&deleted_meeting, "2026-07-02T09:00:00Z"),
        (&older, "2026-07-01T09:00:00Z"),
        (&other_meeting, "2026-07-04T09:00:00Z"),
    ] {
        sqlx::query("UPDATE meetings SET created_at = ? WHERE id = ?")
            .bind(created_at)
            .bind(meeting_id)
            .execute(&pool)
            .await
            .expect("set deterministic meeting date");
    }

    let mut primary = action("approved-primary", "new-source");
    primary.assignee = Some("Ayse".to_string());
    primary.due = Some("next Friday".to_string());
    let newer_batch = [
        primary,
        action("still-draft", "new-source"),
        action("human-edited", "new-source"),
        action("rejected", "new-source"),
        action("approved-secondary", "new-source"),
        action("soft-deleted-item", "new-source"),
        action("wrong-meeting-source", "new-source"),
        action("foreign-workspace-source", "new-source"),
        action("soft-deleted-source", "soft-source"),
    ];
    ActionItemsRepository::insert_drafts(&pool, &local, &newer, &newer_batch)
        .await
        .expect("insert newer batch");

    for id in [
        "approved-primary",
        "approved-secondary",
        "soft-deleted-item",
        "wrong-meeting-source",
        "foreign-workspace-source",
        "soft-deleted-source",
    ] {
        assert!(
            ActionItemsRepository::set_status(&pool, &local, id, BlockStatus::Approved, None)
                .await
                .unwrap_or_else(|error| panic!("approve {id}: {error}")),
            "{id} should approve while its original source is active"
        );
    }
    assert!(ActionItemsRepository::edit(
        &pool,
        &local,
        "human-edited",
        Some("human refined action"),
        None,
        None,
    )
    .await
    .expect("edit action"));
    assert!(ActionItemsRepository::set_status(
        &pool,
        &local,
        "rejected",
        BlockStatus::Rejected,
        None
    )
    .await
    .expect("reject action"));
    assert!(
        ActionItemsRepository::soft_delete(&pool, &local, "soft-deleted-item")
            .await
            .expect("soft-delete action")
    );

    ActionItemsRepository::insert_drafts(
        &pool,
        &local,
        &older,
        &[action("approved-older", "old-source")],
    )
    .await
    .expect("insert older action");
    assert!(ActionItemsRepository::set_status(
        &pool,
        &local,
        "approved-older",
        BlockStatus::Approved,
        None,
    )
    .await
    .expect("approve older action"));

    ActionItemsRepository::insert_drafts(
        &pool,
        &local,
        &deleted_meeting,
        &[action("approved-deleted-meeting", "deleted-meeting-source")],
    )
    .await
    .expect("insert deleted-meeting action");
    assert!(ActionItemsRepository::set_status(
        &pool,
        &local,
        "approved-deleted-meeting",
        BlockStatus::Approved,
        None,
    )
    .await
    .expect("approve deleted-meeting action"));

    ActionItemsRepository::insert_drafts(
        &pool,
        &other,
        &other_meeting,
        &[action("approved-other", "other-source")],
    )
    .await
    .expect("insert other-workspace action");
    assert!(ActionItemsRepository::set_status(
        &pool,
        &other,
        "approved-other",
        BlockStatus::Approved,
        None,
    )
    .await
    .expect("approve other-workspace action"));

    // Simulate corrupt/future-sync rows that violate immutable source rules.
    // The trusted read must fail closed even if storage is tampered with.
    sqlx::query("UPDATE action_items SET source_chunk_id = ? WHERE id = ?")
        .bind("old-source")
        .bind("wrong-meeting-source")
        .execute(&pool)
        .await
        .expect("inject wrong-meeting source");
    sqlx::query("UPDATE action_items SET source_chunk_id = ? WHERE id = ?")
        .bind("other-source")
        .bind("foreign-workspace-source")
        .execute(&pool)
        .await
        .expect("inject foreign-workspace source");
    sqlx::query("UPDATE transcripts SET deleted_at = ? WHERE id = ?")
        .bind("2026-07-14T11:00:00Z")
        .bind("soft-source")
        .execute(&pool)
        .await
        .expect("soft-delete cited source");
    sqlx::query("UPDATE meetings SET deleted_at = ? WHERE id = ?")
        .bind("2026-07-14T11:01:00Z")
        .bind(&deleted_meeting)
        .execute(&pool)
        .await
        .expect("soft-delete cited meeting");

    let first = ActionItemsRepository::list_approved_with_sources(&pool, &local, 2, 0)
        .await
        .expect("first Action Center page");
    assert_eq!(
        first
            .items
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        vec!["approved-primary", "approved-secondary"],
        "backend order must be preserved after every fail-closed filter"
    );
    assert!(first.has_more);
    assert_eq!(first.next_offset, Some(2));
    assert!(first
        .items
        .iter()
        .all(|item| item.review_status == BlockStatus::Approved));
    assert_eq!(first.items[0].meeting_id, newer);
    assert_eq!(first.items[0].meeting_title, "Newer planning");
    assert_eq!(first.items[0].meeting_created_at, "2026-07-03T09:00:00Z");
    assert_eq!(first.items[0].assignee.as_deref(), Some("Ayse"));
    assert_eq!(first.items[0].due.as_deref(), Some("next Friday"));
    assert_eq!(first.items[0].source_chunk_id, "new-source");
    assert_eq!(first.items[0].source_timestamp, "2026-07-05T10:00:00.000Z");
    assert_eq!(first.items[0].audio_start_time, Some(0.0));

    let second = ActionItemsRepository::list_approved_with_sources(
        &pool,
        &local,
        2,
        first.next_offset.expect("next page offset"),
    )
    .await
    .expect("second Action Center page");
    assert_eq!(
        second
            .items
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        vec!["approved-older"]
    );
    assert!(!second.has_more);
    assert_eq!(second.next_offset, None);

    let clamped = ActionItemsRepository::list_approved_with_sources(&pool, &local, 0, 0)
        .await
        .expect("zero limit is safely clamped");
    assert_eq!(clamped.items.len(), 1);
    assert!(clamped.has_more);
    assert_eq!(clamped.next_offset, Some(1));

    let other_page = ActionItemsRepository::list_approved_with_sources(&pool, &other, 100, 0)
        .await
        .expect("other-workspace page");
    assert_eq!(other_page.items.len(), 1);
    assert_eq!(other_page.items[0].id, "approved-other");
    assert_eq!(other_page.items[0].meeting_id, other_meeting);

    let wire = serde_json::to_value(&first).expect("serialize Action Center page");
    assert!(wire.get("hasMore").is_some());
    assert!(wire.get("nextOffset").is_some());
    assert!(wire["items"][0].get("meetingId").is_some());
    assert!(wire["items"][0].get("sourceChunkId").is_some());
    assert_eq!(wire["items"][0]["reviewStatus"], "approved");
    assert!(wire["items"][0].get("meeting_id").is_none());

    pool.close().await;
}

/// A caller cannot bypass the Action Center's 200-row safety cap, and the
/// visible continuation metadata makes the remaining rows retrievable.
#[tokio::test]
async fn approved_action_center_caps_large_pages_without_silent_truncation() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let pool = open_migrated_temp_db(&tmp.path().join("action_center_page_cap.sqlite")).await;
    let local = AuthContext::local();
    let meeting = seed_meeting(&pool, &local, "Large action set", &["cap-source"]).await;

    assert_eq!(DEFAULT_ACTION_CENTER_PAGE_SIZE, 100);
    assert_eq!(MAX_ACTION_CENTER_PAGE_SIZE, 200);

    let generated: Vec<ActionItemDraft> = (0..205)
        .map(|index| action(&format!("cap-{index:03}"), "cap-source"))
        .collect();
    ActionItemsRepository::insert_drafts(&pool, &local, &meeting, &generated)
        .await
        .expect("insert large generated batch");

    // Projection-only fixture setup: the approval transition itself is covered
    // elsewhere; this test needs enough approved rows to exercise the cap.
    sqlx::query(
        "UPDATE action_items SET status = 'approved' \
         WHERE meeting_id = ? AND workspace_id = ? AND deleted_at IS NULL",
    )
    .bind(&meeting)
    .bind(LOCAL_WORKSPACE_ID)
    .execute(&pool)
    .await
    .expect("mark page-cap fixture approved");

    let first = ActionItemsRepository::list_approved_with_sources(&pool, &local, u32::MAX, 0)
        .await
        .expect("capped first page");
    assert_eq!(first.items.len(), MAX_ACTION_CENTER_PAGE_SIZE as usize);
    assert!(first.has_more);
    assert_eq!(first.next_offset, Some(MAX_ACTION_CENTER_PAGE_SIZE));
    assert_eq!(first.items.first().unwrap().id, "cap-000");
    assert_eq!(first.items.last().unwrap().id, "cap-199");

    let second = ActionItemsRepository::list_approved_with_sources(
        &pool,
        &local,
        u32::MAX,
        first.next_offset.unwrap(),
    )
    .await
    .expect("capped second page");
    assert_eq!(second.items.len(), 5);
    assert!(!second.has_more);
    assert_eq!(second.next_offset, None);
    assert_eq!(second.items.first().unwrap().id, "cap-200");
    assert_eq!(second.items.last().unwrap().id, "cap-204");

    pool.close().await;
}
