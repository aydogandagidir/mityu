//! Integration tests for the C1.5 HITL Tauri commands (source-linked summary
//! draft review/approval — BACKLOG C1.5, docs/CONTRACTS.md §4, ADR-0019).
//!
//! The C1.5 commands (`api_get_summary_draft`, the block-level
//! approve/reject/edit/restore, `api_approve_summary`, and the action-item
//! mirror in `src/summary/commands.rs`) are THIN wrappers: each resolves
//! identity via `crate::context::current()`, calls exactly one C1.3 repository
//! method (`SummariesRepository` / `ActionItemsRepository`), and maps the typed
//! error to a content-free `String`. They carry NO business logic of their own.
//!
//! Driving the `#[tauri::command]` fns directly needs a live `tauri::State<'_,
//! AppState>` (a full `AppHandle` + initialized DB manager), which is not
//! constructible in a headless integration test. So — per the C1.5 test
//! guidance — the approval STATE MACHINE these commands expose is proven
//! end-to-end at the repository layer (the exact code path each wrapper
//! delegates to, under the same local `AuthContext` that `context::current()`
//! resolves to), and the command-only surface that does NOT touch a repository
//! — the request/response wire types, incl. the documented double-`Option`
//! deviation (`FieldPatch`) — is unit-tested directly against the public types.
//!
//! Scenario map (task acceptance list a–e):
//!   (a) approve blocks -> approve_summary succeeds and stamps
//!       `approved_at`/`approved_by`;
//!   (b) approve_summary refused (`Ok(false)`) while any non-rejected block is
//!       unapproved;
//!   (c) edit after approve drops the summary back to draft;
//!   (d) reject then restore returns a block / an item to draft (the
//!       `rejected -> draft` arc, exercised here end-to-end);
//!   (e) every mutation under a FOREIGN workspace context is a no-op.
//!
//! Overlap note: `tests/repository_tenant_scoping_c1.rs` already proves (a)/(b)/
//! (c) as part of its happy-path test and (e) as part of its foreign-context
//! test. This file re-asserts them SPECIFICALLY as the C1.5 command contract
//! (one focused test per acceptance bullet, the `SummaryStatus`/`approved_by`
//! surface the `api_get_summary_draft` response exposes), and adds (d) — the
//! reject-then-restore arc for BOTH a block and an action item — which the C1.3
//! suite did not exercise directly.

use sqlx::migrate::Migrator;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;

use app_lib::api::TranscriptSegment;
use app_lib::context::{AuthContext, RequestId, Role, TenantId, UserId};
use app_lib::database::repositories::{
    action_item::ActionItemsRepository,
    summary_draft::{SummariesRepository, SummaryDraftError},
    transcript::TranscriptsRepository,
};
use app_lib::summary::commands::{EditActionItemRequest, FieldPatch};
use app_lib::summary::draft::{
    ActionItemDraft, BlockStatus, BlockType, DraftBlock, DraftSection, MeetingNotesDraft,
    SummaryStatus,
};

/// The app's real, compile-time-embedded migration set (same source as
/// `DatabaseManager::new`, matching the C1.3 harness).
static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

const OTHER_WORKSPACE_ID: &str = "other-ws";

/// A second workspace's context, built directly (test-only). Production code
/// NEVER constructs this — `context::current()` resolves to the local
/// workspace, and the C1.5 commands take identity from there alone (ADR-0010);
/// this stands in for "a request that arrived under a different tenant".
fn other_ws_ctx() -> AuthContext {
    AuthContext {
        tenant_id: TenantId::new(OTHER_WORKSPACE_ID),
        user_id: UserId::new("other-user"),
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

/// Seeds a meeting whose transcript segment ids are exactly `chunk_ids` (the
/// evidence anchors the drafts cite).
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

fn block(id: &str, chunk: &str) -> DraftBlock {
    DraftBlock {
        id: id.to_string(),
        block_type: BlockType::Bullet,
        content: format!("generated content of {id}"),
        source_chunk_id: chunk.to_string(),
        status: BlockStatus::Draft,
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

/// Finds one block's status in a hydrated draft.
fn block_status(draft: &MeetingNotesDraft, block_id: &str) -> BlockStatus {
    draft
        .sections
        .iter()
        .flat_map(|s| s.blocks.iter())
        .find(|b| b.id == block_id)
        .unwrap_or_else(|| panic!("block {block_id} must be present"))
        .status
}

// ---------------------------------------------------------------------------
// (a) approve-block(s) then approve-summary succeeds and stamps approved_*.
// The `SummaryDraftResponse` fields the command exposes are read via the same
// `SummariesRepository::get_by_meeting` hydration the command uses.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn approve_blocks_then_approve_summary_succeeds_and_stamps_approver() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let pool = open_migrated_temp_db(&tmp.path().join("c15_approve.sqlite")).await;
    let ctx = AuthContext::local();

    let meeting = seed_meeting(&pool, &ctx, "Approve path", &["c-1", "c-2"]).await;
    SummariesRepository::upsert_draft(
        &pool,
        &ctx,
        &notes(&meeting, vec![block("b1", "c-1"), block("b2", "c-2")]),
        Some("llama3.2"),
        Some("daily_standup"),
    )
    .await
    .expect("upsert_draft");

    // api_approve_summary_block -> set_block_status(Approved) for each block.
    for id in ["b1", "b2"] {
        assert!(
            SummariesRepository::set_block_status(&pool, &ctx, &meeting, id, BlockStatus::Approved)
                .await
                .expect("approve block runs"),
            "approving a draft block with resolvable evidence must succeed"
        );
    }

    // api_approve_summary -> approve_summary (gate satisfied).
    assert!(
        SummariesRepository::approve_summary(&pool, &ctx, &meeting)
            .await
            .expect("approve_summary runs"),
        "approve_summary must succeed once every non-rejected block is approved"
    );

    // The fields api_get_summary_draft surfaces (status/approved_at/approved_by).
    let row = SummariesRepository::get_by_meeting(&pool, &ctx, &meeting)
        .await
        .expect("get_by_meeting")
        .expect("summary row exists");
    assert_eq!(row.status, SummaryStatus::Approved);
    assert_eq!(row.approved_by.as_deref(), Some("local-user"));
    let approved_at = row.approved_at.expect("approved_at must be stamped");
    chrono::DateTime::parse_from_rfc3339(&approved_at)
        .unwrap_or_else(|e| panic!("approved_at must be RFC 3339, got {approved_at:?}: {e}"));

    pool.close().await;
}

// ---------------------------------------------------------------------------
// (b) approve-summary refused (Ok(false)) while ANY non-rejected block is
// unapproved — the command returns the repository's Ok(false) verbatim.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn approve_summary_refused_while_a_nonrejected_block_is_unapproved() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let pool = open_migrated_temp_db(&tmp.path().join("c15_gate.sqlite")).await;
    let ctx = AuthContext::local();

    let meeting = seed_meeting(&pool, &ctx, "Gated", &["c-1", "c-2"]).await;
    SummariesRepository::upsert_draft(
        &pool,
        &ctx,
        &notes(&meeting, vec![block("b1", "c-1"), block("b2", "c-2")]),
        None,
        None,
    )
    .await
    .expect("upsert_draft");

    // Nothing approved yet -> refused.
    assert!(
        !SummariesRepository::approve_summary(&pool, &ctx, &meeting)
            .await
            .expect("approve_summary runs"),
        "approve_summary must refuse with all blocks in draft"
    );

    // Approve only b1; b2 still draft -> still refused.
    assert!(SummariesRepository::set_block_status(
        &pool,
        &ctx,
        &meeting,
        "b1",
        BlockStatus::Approved
    )
    .await
    .expect("approve b1"));
    assert!(
        !SummariesRepository::approve_summary(&pool, &ctx, &meeting)
            .await
            .expect("approve_summary runs"),
        "approve_summary must refuse while b2 (non-rejected) is unapproved"
    );

    // The refusal changed nothing: still a draft, no approver stamped.
    let row = SummariesRepository::get_by_meeting(&pool, &ctx, &meeting)
        .await
        .expect("get_by_meeting")
        .expect("summary row");
    assert_eq!(row.status, SummaryStatus::Draft);
    assert_eq!(row.approved_at, None);
    assert_eq!(row.approved_by, None);

    pool.close().await;
}

// ---------------------------------------------------------------------------
// (c) edit-after-approve drops the summary back to draft (api_edit_summary_block
// -> edit_block invariant), preserving the generated original.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn edit_after_approve_drops_summary_back_to_draft() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let pool = open_migrated_temp_db(&tmp.path().join("c15_edit.sqlite")).await;
    let ctx = AuthContext::local();

    let meeting = seed_meeting(&pool, &ctx, "Edit reopens", &["c-1"]).await;
    SummariesRepository::upsert_draft(
        &pool,
        &ctx,
        &notes(&meeting, vec![block("b1", "c-1")]),
        None,
        None,
    )
    .await
    .expect("upsert_draft");
    assert!(SummariesRepository::set_block_status(
        &pool,
        &ctx,
        &meeting,
        "b1",
        BlockStatus::Approved
    )
    .await
    .expect("approve b1"));
    assert!(SummariesRepository::approve_summary(&pool, &ctx, &meeting)
        .await
        .expect("approve_summary"));

    // Edit re-opens the summary.
    assert!(
        SummariesRepository::edit_block(&pool, &ctx, &meeting, "b1", "human-improved text")
            .await
            .expect("edit_block")
    );
    let row = SummariesRepository::get_by_meeting(&pool, &ctx, &meeting)
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
    let b1 = row
        .draft
        .sections
        .iter()
        .flat_map(|s| s.blocks.iter())
        .find(|b| b.id == "b1")
        .expect("b1 present");
    assert_eq!(b1.status, BlockStatus::Edited);
    assert_eq!(b1.content, "human-improved text");
    assert_eq!(
        b1.original_content.as_deref(),
        Some("generated content of b1"),
        "the first edit must preserve the generated text (§4 auditability)"
    );
    assert_eq!(
        b1.source_chunk_id, "c-1",
        "edit must NEVER touch the evidence anchor"
    );

    pool.close().await;
}

// ---------------------------------------------------------------------------
// (d) reject-then-restore returns a block AND an action item to draft
// (api_reject_* -> Rejected, api_restore_* -> Draft; the `rejected -> draft`
// arc). This is the C1.5-specific gap not exercised directly by the C1.3 suite.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn reject_then_restore_returns_block_and_item_to_draft() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let pool = open_migrated_temp_db(&tmp.path().join("c15_restore.sqlite")).await;
    let ctx = AuthContext::local();

    let meeting = seed_meeting(&pool, &ctx, "Restore", &["c-1"]).await;
    SummariesRepository::upsert_draft(
        &pool,
        &ctx,
        &notes(&meeting, vec![block("b1", "c-1")]),
        None,
        None,
    )
    .await
    .expect("upsert_draft");
    ActionItemsRepository::insert_drafts(&pool, &ctx, &meeting, &[action("a1", "c-1")])
        .await
        .expect("insert_drafts");

    // Block: draft -> rejected -> draft.
    assert!(SummariesRepository::set_block_status(
        &pool,
        &ctx,
        &meeting,
        "b1",
        BlockStatus::Rejected
    )
    .await
    .expect("reject b1"));
    let row = SummariesRepository::get_by_meeting(&pool, &ctx, &meeting)
        .await
        .expect("get")
        .expect("row");
    assert_eq!(block_status(&row.draft, "b1"), BlockStatus::Rejected);

    // api_restore_summary_block -> set_block_status(Draft).
    assert!(
        SummariesRepository::set_block_status(&pool, &ctx, &meeting, "b1", BlockStatus::Draft)
            .await
            .expect("restore b1 runs"),
        "rejected -> draft (restore) must be legal for a block"
    );
    let row = SummariesRepository::get_by_meeting(&pool, &ctx, &meeting)
        .await
        .expect("get")
        .expect("row");
    assert_eq!(
        block_status(&row.draft, "b1"),
        BlockStatus::Draft,
        "restore must return the block to draft"
    );

    // A restored (draft) block is approvable again -> full arc round-trips.
    assert!(SummariesRepository::set_block_status(
        &pool,
        &ctx,
        &meeting,
        "b1",
        BlockStatus::Approved
    )
    .await
    .expect("re-approve restored b1"));

    // Item: draft -> rejected -> draft (api_reject_action_item /
    // api_restore_action_item -> set_status).
    assert!(
        ActionItemsRepository::set_status(&pool, &ctx, "a1", BlockStatus::Rejected)
            .await
            .expect("reject a1")
    );
    assert_eq!(
        ActionItemsRepository::list_by_meeting(&pool, &ctx, &meeting)
            .await
            .expect("list")[0]
            .status,
        BlockStatus::Rejected
    );
    assert!(
        ActionItemsRepository::set_status(&pool, &ctx, "a1", BlockStatus::Draft)
            .await
            .expect("restore a1 runs"),
        "rejected -> draft (restore) must be legal for an action item"
    );
    assert_eq!(
        ActionItemsRepository::list_by_meeting(&pool, &ctx, &meeting)
            .await
            .expect("list")[0]
            .status,
        BlockStatus::Draft,
        "restore must return the item to draft"
    );

    pool.close().await;
}

// ---------------------------------------------------------------------------
// (e) EVERY C1.5 mutation under a FOREIGN workspace context is a no-op
// (Ok(false)). The commands themselves can only ever pass the LOCAL context
// (context::current()); this proves the delegated repository refuses a
// mismatched tenant, i.e. cross-workspace calls cannot mutate.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn every_mutation_under_foreign_workspace_is_a_noop() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let pool = open_migrated_temp_db(&tmp.path().join("c15_foreign.sqlite")).await;
    let local = AuthContext::local();
    let other = other_ws_ctx();

    let meeting = seed_meeting(&pool, &local, "Local only", &["c-1"]).await;
    SummariesRepository::upsert_draft(
        &pool,
        &local,
        &notes(&meeting, vec![block("b1", "c-1")]),
        None,
        None,
    )
    .await
    .expect("local upsert_draft");
    ActionItemsRepository::insert_drafts(&pool, &local, &meeting, &[action("a1", "c-1")])
        .await
        .expect("local insert_drafts");

    // Every block-level command path, foreign context -> Ok(false).
    for new in [
        BlockStatus::Approved,
        BlockStatus::Rejected,
        BlockStatus::Draft,
    ] {
        assert!(
            !SummariesRepository::set_block_status(&pool, &other, &meeting, "b1", new)
                .await
                .expect("foreign set_block_status runs"),
            "foreign block status change ({new:?}) must be a no-op"
        );
    }
    assert!(
        !SummariesRepository::edit_block(&pool, &other, &meeting, "b1", "hacked")
            .await
            .expect("foreign edit_block runs"),
        "foreign edit_block must be a no-op"
    );
    assert!(
        !SummariesRepository::approve_summary(&pool, &other, &meeting)
            .await
            .expect("foreign approve_summary runs"),
        "foreign approve_summary must be a no-op"
    );

    // Every action-item command path, foreign context -> Ok(false).
    for new in [
        BlockStatus::Approved,
        BlockStatus::Rejected,
        BlockStatus::Draft,
    ] {
        assert!(
            !ActionItemsRepository::set_status(&pool, &other, "a1", new)
                .await
                .expect("foreign set_status runs"),
            "foreign item status change ({new:?}) must be a no-op"
        );
    }
    assert!(
        !ActionItemsRepository::edit(&pool, &other, "a1", Some("hacked"), None, None)
            .await
            .expect("foreign item edit runs"),
        "foreign item edit must be a no-op"
    );

    // The local rows are untouched: still a fresh draft with rev == 1.
    let row = SummariesRepository::get_by_meeting(&pool, &local, &meeting)
        .await
        .expect("local get")
        .expect("local row");
    assert_eq!(row.status, SummaryStatus::Draft);
    assert_eq!(block_status(&row.draft, "b1"), BlockStatus::Draft);
    assert_eq!(row.rev, 1, "no foreign write may bump the local row's rev");
    let items = ActionItemsRepository::list_by_meeting(&pool, &local, &meeting)
        .await
        .expect("local list");
    assert_eq!(items[0].status, BlockStatus::Draft);
    assert_eq!(items[0].rev, 1, "no foreign write may bump the item's rev");

    // The foreign workspace also reads nothing (get_summary_draft would return
    // draft: None + empty action_items for `other`).
    assert!(SummariesRepository::get_by_meeting(&pool, &other, &meeting)
        .await
        .expect("foreign get")
        .is_none());
    assert!(
        ActionItemsRepository::list_by_meeting(&pool, &other, &meeting)
            .await
            .expect("foreign list")
            .is_empty()
    );

    pool.close().await;
}

// ---------------------------------------------------------------------------
// Command-only surface (no repository / no AppState needed): the action-item
// edit request wire types. This is the documented double-`Option` deviation —
// `api_edit_action_item` takes a typed `EditActionItemRequest` whose per-field
// `FieldPatch` distinguishes keep / clear / set unambiguously over JSON, then
// lowers to the C1.3 `edit(text, assignee, due)` `Option<Option<&str>>`.
// ---------------------------------------------------------------------------

/// The whole reason the typed request exists: over JSON, an omitted field and
/// an explicit "clear" must NOT collapse to the same repository intent (plain
/// `Option<Option<String>>` would). Absent -> keep (`None`); `clear` ->
/// `Some(None)`; `set` -> `Some(Some(v))`.
#[test]
fn edit_action_item_request_distinguishes_clear_from_omitted() {
    // Only text supplied: assignee/due omitted => keep (None), text sets.
    let omitted: EditActionItemRequest =
        serde_json::from_str(r#"{ "text": "refined" }"#).expect("omitted patches parse");
    assert_eq!(omitted.text.as_deref(), Some("refined"));
    assert!(
        matches!(omitted.assignee, FieldPatch::Keep),
        "omitted assignee must default to Keep"
    );
    assert!(
        matches!(omitted.due, FieldPatch::Keep),
        "omitted due must default to Keep"
    );

    // Explicit clear + set: distinct from omitted, and text stays None.
    let explicit: EditActionItemRequest = serde_json::from_str(
        r#"{ "assignee": { "op": "clear" }, "due": { "op": "set", "value": "Friday" } }"#,
    )
    .expect("explicit patches parse");
    assert_eq!(explicit.text, None);
    assert!(
        matches!(explicit.assignee, FieldPatch::Clear),
        "explicit clear must parse to Clear (NOT Keep) — the deviation's whole point"
    );
    match explicit.due {
        FieldPatch::Set { ref value } => assert_eq!(value, "Friday"),
        other => panic!("due must be Set, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Sanity: the typed error the commands map to a `String` stays content-free
// (ids/counts only). Guards against a future variant leaking meeting text into
// the Tauri error channel (CLAUDE.md §0.6).
// ---------------------------------------------------------------------------
#[test]
fn summary_draft_error_display_is_content_free() {
    let unresolvable = SummaryDraftError::UnresolvableSources { count: 2 };
    let msg = unresolvable.to_string();
    assert!(msg.contains('2'), "must name the count: {msg}");

    let not_found = SummaryDraftError::MeetingNotFound.to_string();
    assert!(
        not_found.contains("not found"),
        "not-found message stays generic: {not_found}"
    );
}
