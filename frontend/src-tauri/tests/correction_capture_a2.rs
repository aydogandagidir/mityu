//! Integration tests for the append-only correction log (ADR-0030 §2, phase A2).
//!
//! What these prove, in one line: **the HITL correction signal is now captured,
//! is model→human, and survives the regeneration that destroys the inline copy.**
//!
//! The app has always kept the last correction inline
//! (`summaries.sections[].original_content`, `action_items.original_text`), but
//! that is state, not a log. `upsert_draft` rewrites the whole `sections` JSON on
//! every regeneration, so the inline delta evaporates exactly when the user
//! iterates hardest. [`regeneration_destroys_the_inline_delta_but_not_the_log`]
//! asserts BOTH halves of that sentence in one test: the inline copy is gone (the
//! defect is real, not theoretical) and the log is intact (A2 fixes it).
//!
//! Same harness as `hitl_commands_c1.rs`: the real compile-time-embedded
//! migration set, driven at the repository layer under the same local
//! `AuthContext` that `context::current()` resolves to, because the
//! `#[tauri::command]` wrappers need a live `AppHandle` no headless test can
//! build.

use sqlx::migrate::Migrator;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;

use app_lib::api::TranscriptSegment;
use app_lib::context::{AuthContext, RequestId, Role, TenantId, UserId};
use app_lib::database::repositories::{
    action_item::ActionItemsRepository,
    correction_event::{
        CorrectionAction, CorrectionEventRow, CorrectionEventsRepository, CorrectionSubject,
    },
    summary_draft::SummariesRepository,
    transcript::TranscriptsRepository,
};
use app_lib::summary::draft::{
    ActionItemDraft, BlockStatus, BlockType, DraftBlock, DraftSection, MeetingNotesDraft,
    SummaryStatus,
};

static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

fn other_ws_ctx() -> AuthContext {
    AuthContext {
        tenant_id: TenantId::new("other-ws"),
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

fn segment(id: &str, start: f64) -> TranscriptSegment {
    TranscriptSegment {
        id: id.to_string(),
        text: format!("segment {id}"),
        timestamp: "2026-07-16T10:00:00.000Z".to_string(),
        audio_start_time: Some(start),
        audio_end_time: Some(start + 1.0),
        duration: Some(1.0),
    }
}

async fn seed_meeting(pool: &SqlitePool, ctx: &AuthContext, chunk_ids: &[&str]) -> String {
    let segments: Vec<TranscriptSegment> = chunk_ids
        .iter()
        .enumerate()
        .map(|(i, id)| segment(id, i as f64))
        .collect();
    TranscriptsRepository::create_meeting_with_segments(
        pool,
        ctx,
        "Saha görüşmesi",
        &segments,
        None,
    )
    .await
    .expect("create_meeting_with_segments")
}

fn block(id: &str, chunk: &str, content: &str) -> DraftBlock {
    DraftBlock {
        id: id.to_string(),
        block_type: BlockType::Bullet,
        content: content.to_string(),
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
            title: "Kararlar".to_string(),
            blocks,
        }],
    }
}

async fn seed_summary(
    pool: &SqlitePool,
    ctx: &AuthContext,
    meeting: &str,
    blocks: Vec<DraftBlock>,
) {
    SummariesRepository::upsert_draft(
        pool,
        ctx,
        &notes(meeting, blocks),
        Some("llama3.2"),
        Some("daily_standup"),
        &[],
    )
    .await
    .expect("upsert_draft");
}

async fn events(pool: &SqlitePool, ctx: &AuthContext, meeting: &str) -> Vec<CorrectionEventRow> {
    CorrectionEventsRepository::list_for_meeting(pool, ctx, meeting)
        .await
        .expect("list_for_meeting")
}

// ---------------------------------------------------------------------------
// An edit records the model→human delta, plus the generation context the miner
// needs to scope a rule (section title, template, model, block kind).
// ---------------------------------------------------------------------------
#[tokio::test]
async fn edit_captures_the_model_to_human_delta_and_its_context() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let pool = open_migrated_temp_db(&tmp.path().join("a2_edit.sqlite")).await;
    let ctx = AuthContext::local();

    let meeting = seed_meeting(&pool, &ctx, &["c-1"]).await;
    seed_summary(
        &pool,
        &ctx,
        &meeting,
        vec![block("b1", "c-1", "3 aksiyon çıktı")],
    )
    .await;

    assert!(
        SummariesRepository::edit_block(&pool, &ctx, &meeting, "b1", "3 takip maddesi çıktı")
            .await
            .expect("edit_block"),
    );

    let events = events(&pool, &ctx, &meeting).await;
    assert_eq!(events.len(), 1, "exactly one correction must be recorded");
    let e = &events[0];
    assert_eq!(e.action, CorrectionAction::Edit);
    assert_eq!(e.subject_kind, CorrectionSubject::SummaryBlock);
    assert_eq!(e.subject_id, "b1");
    assert_eq!(e.original_text.as_deref(), Some("3 aksiyon çıktı"));
    assert_eq!(e.final_text.as_deref(), Some("3 takip maddesi çıktı"));
    // The context the miner scopes rules by — captured here because
    // `summaries.sections` will not survive a regeneration to be asked later.
    assert_eq!(e.section_title.as_deref(), Some("Kararlar"));
    assert_eq!(e.template_id.as_deref(), Some("daily_standup"));
    assert_eq!(e.model.as_deref(), Some("llama3.2"));
    assert_eq!(e.block_type.as_deref(), Some("bullet"));
}

// ---------------------------------------------------------------------------
// The subtle one. On a RE-edit, `content` already holds a human revision — so
// reading it would record a human→human delta and understate the burden. Every
// event must compare against what the MODEL wrote.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn re_edit_records_the_model_text_not_the_previous_human_revision() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let pool = open_migrated_temp_db(&tmp.path().join("a2_reedit.sqlite")).await;
    let ctx = AuthContext::local();

    let meeting = seed_meeting(&pool, &ctx, &["c-1"]).await;
    seed_summary(
        &pool,
        &ctx,
        &meeting,
        vec![block("b1", "c-1", "model metni")],
    )
    .await;

    for revision in ["ilk düzeltme", "ikinci düzeltme"] {
        assert!(
            SummariesRepository::edit_block(&pool, &ctx, &meeting, "b1", revision)
                .await
                .expect("edit_block"),
        );
    }

    let events = events(&pool, &ctx, &meeting).await;
    assert_eq!(events.len(), 2);
    // BOTH events anchor on the model's text — not on each other.
    for e in &events {
        assert_eq!(e.original_text.as_deref(), Some("model metni"));
    }
    assert_eq!(events[0].final_text.as_deref(), Some("ilk düzeltme"));
    assert_eq!(events[1].final_text.as_deref(), Some("ikinci düzeltme"));
}

// ---------------------------------------------------------------------------
// THE test A2 exists for. The inline delta dies on regeneration; the log does
// not. Both halves asserted together so neither can rot silently.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn regeneration_destroys_the_inline_delta_but_not_the_log() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let pool = open_migrated_temp_db(&tmp.path().join("a2_regen.sqlite")).await;
    let ctx = AuthContext::local();

    let meeting = seed_meeting(&pool, &ctx, &["c-1"]).await;
    seed_summary(
        &pool,
        &ctx,
        &meeting,
        vec![block("b1", "c-1", "3 aksiyon çıktı")],
    )
    .await;
    SummariesRepository::edit_block(&pool, &ctx, &meeting, "b1", "3 takip maddesi çıktı")
        .await
        .expect("edit_block");

    // Precondition: the inline copy exists right now.
    let before = SummariesRepository::get_by_meeting(&pool, &ctx, &meeting)
        .await
        .expect("get_by_meeting")
        .expect("summary row");
    assert_eq!(
        before.draft.sections[0].blocks[0]
            .original_content
            .as_deref(),
        Some("3 aksiyon çıktı"),
    );

    // Regenerate — `upsert_draft` rewrites `sections` wholesale.
    seed_summary(
        &pool,
        &ctx,
        &meeting,
        vec![block("b1", "c-1", "yeniden üretilmiş metin")],
    )
    .await;

    // Half 1 — the defect is REAL: the inline delta is gone. If this ever starts
    // failing, `upsert_draft` learned to preserve `original_content` and the
    // motivation recorded in ADR-0030 §2 needs revisiting (the log stays correct
    // either way).
    let after = SummariesRepository::get_by_meeting(&pool, &ctx, &meeting)
        .await
        .expect("get_by_meeting")
        .expect("summary row");
    assert_eq!(
        after.draft.sections[0].blocks[0].original_content, None,
        "regeneration is expected to wipe the inline delta",
    );
    assert_eq!(
        after.draft.sections[0].blocks[0].content,
        "yeniden üretilmiş metin",
    );

    // Half 2 — the fix: the correction outlives the rewrite, intact.
    let events = events(&pool, &ctx, &meeting).await;
    assert_eq!(events.len(), 1, "the log must survive regeneration");
    assert_eq!(events[0].original_text.as_deref(), Some("3 aksiyon çıktı"));
    assert_eq!(
        events[0].final_text.as_deref(),
        Some("3 takip maddesi çıktı"),
    );
}

// ---------------------------------------------------------------------------
// A reject carries the rationale and leaves no surviving text; an approve of an
// untouched block is the zero-burden positive signal (original == final).
// ---------------------------------------------------------------------------
#[tokio::test]
async fn reject_records_a_reason_and_approve_of_untouched_text_is_zero_burden() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let pool = open_migrated_temp_db(&tmp.path().join("a2_verdicts.sqlite")).await;
    let ctx = AuthContext::local();

    let meeting = seed_meeting(&pool, &ctx, &["c-1", "c-2"]).await;
    seed_summary(
        &pool,
        &ctx,
        &meeting,
        vec![
            block("b1", "c-1", "müşteri fiyat istedi"),
            block("b2", "c-2", "bu sadece sohbetti"),
        ],
    )
    .await;

    assert!(SummariesRepository::set_block_status(
        &pool,
        &ctx,
        &meeting,
        "b2",
        BlockStatus::Rejected,
        Some("bu bir karar değil, sohbetti"),
    )
    .await
    .expect("reject"));

    assert!(SummariesRepository::set_block_status(
        &pool,
        &ctx,
        &meeting,
        "b1",
        BlockStatus::Approved,
        None,
    )
    .await
    .expect("approve"));

    let events = events(&pool, &ctx, &meeting).await;
    assert_eq!(events.len(), 2);

    let reject = events
        .iter()
        .find(|e| e.subject_id == "b2")
        .expect("reject");
    assert_eq!(reject.action, CorrectionAction::Reject);
    assert_eq!(
        reject.reason.as_deref(),
        Some("bu bir karar değil, sohbetti")
    );
    assert_eq!(
        reject.final_text, None,
        "a reject leaves nothing standing to record",
    );

    let approve = events
        .iter()
        .find(|e| e.subject_id == "b1")
        .expect("approve");
    assert_eq!(approve.action, CorrectionAction::Approve);
    // The model got it exactly right — E1 reads this as zero correction burden.
    assert_eq!(approve.original_text, approve.final_text);
    assert_eq!(approve.reason, None);
}

// ---------------------------------------------------------------------------
// Retranscription is the APP resetting its own bookkeeping. Nobody said
// anything, so nothing may be learned from it.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn the_retranscription_downgrade_is_not_a_human_correction() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let pool = open_migrated_temp_db(&tmp.path().join("a2_downgrade.sqlite")).await;
    let ctx = AuthContext::local();

    let meeting = seed_meeting(&pool, &ctx, &["c-1"]).await;
    seed_summary(
        &pool,
        &ctx,
        &meeting,
        vec![block("b1", "c-1", "model metni")],
    )
    .await;
    SummariesRepository::set_block_status(&pool, &ctx, &meeting, "b1", BlockStatus::Approved, None)
        .await
        .expect("approve");

    let before = events(&pool, &ctx, &meeting).await.len();

    assert!(
        SummariesRepository::downgrade_to_draft(&pool, &ctx, &meeting)
            .await
            .expect("downgrade_to_draft")
    );

    assert_eq!(
        events(&pool, &ctx, &meeting).await.len(),
        before,
        "a system-driven downgrade must leave no correction event",
    );
}

// ---------------------------------------------------------------------------
// Action items: a text edit is a language correction; an assignee-only patch is
// not one the burden metric can read, so it deliberately records nothing.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn action_item_text_edit_records_a_delta_but_an_assignee_only_patch_does_not() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let pool = open_migrated_temp_db(&tmp.path().join("a2_items.sqlite")).await;
    let ctx = AuthContext::local();

    let meeting = seed_meeting(&pool, &ctx, &["c-1"]).await;
    seed_summary(
        &pool,
        &ctx,
        &meeting,
        vec![block("b1", "c-1", "model metni")],
    )
    .await;
    ActionItemsRepository::insert_drafts(
        &pool,
        &ctx,
        &meeting,
        &[ActionItemDraft {
            id: "a1".to_string(),
            text: "Müşteriye aksiyon gönder".to_string(),
            assignee: None,
            due: None,
            status: BlockStatus::Draft,
            source_chunk_id: "c-1".to_string(),
        }],
    )
    .await
    .expect("insert_drafts");

    // Assignee-only patch: a real correction of the model's extraction, but no
    // model→human TEXT delta — recording it would enter E1 as a zero-cost edit.
    assert!(
        ActionItemsRepository::edit(&pool, &ctx, "a1", None, Some(Some("Ayşe")), None,)
            .await
            .expect("assignee patch")
    );
    assert!(
        events(&pool, &ctx, &meeting).await.is_empty(),
        "an assignee-only patch must not be recorded as a language correction",
    );

    // Text edit: the real signal.
    assert!(ActionItemsRepository::edit(
        &pool,
        &ctx,
        "a1",
        Some("Müşteriye takip maili gönder"),
        None,
        None,
    )
    .await
    .expect("text edit"));

    let events = events(&pool, &ctx, &meeting).await;
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].subject_kind, CorrectionSubject::ActionItem);
    assert_eq!(events[0].action, CorrectionAction::Edit);
    assert_eq!(
        events[0].original_text.as_deref(),
        Some("Müşteriye aksiyon gönder"),
    );
    assert_eq!(
        events[0].final_text.as_deref(),
        Some("Müşteriye takip maili gönder"),
    );
    // Action items have no section and no §4 block kind; the template/model come
    // from the meeting's summary row via LEFT JOIN.
    assert_eq!(events[0].section_title, None);
    assert_eq!(events[0].block_type, None);
    assert_eq!(events[0].template_id.as_deref(), Some("daily_standup"));
}

// ---------------------------------------------------------------------------
// Erasure (ADR-0030 §10): deleting the meeting takes its corrections with it, so
// "delete my data" stays a real DELETE — the promise a fine-tune cannot make.
// Proven here through the app's own sqlx pool, not a hand-rolled connection.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn deleting_the_meeting_erases_its_corrections() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let pool = open_migrated_temp_db(&tmp.path().join("a2_erasure.sqlite")).await;
    let ctx = AuthContext::local();

    let meeting = seed_meeting(&pool, &ctx, &["c-1"]).await;
    seed_summary(
        &pool,
        &ctx,
        &meeting,
        vec![block("b1", "c-1", "model metni")],
    )
    .await;
    SummariesRepository::edit_block(&pool, &ctx, &meeting, "b1", "insan metni")
        .await
        .expect("edit_block");
    assert_eq!(events(&pool, &ctx, &meeting).await.len(), 1);

    sqlx::query("DELETE FROM meetings WHERE id = ?")
        .bind(&meeting)
        .execute(&pool)
        .await
        .expect("delete meeting");

    assert!(
        events(&pool, &ctx, &meeting).await.is_empty(),
        "correction events must CASCADE with their meeting",
    );
    assert_eq!(
        CorrectionEventsRepository::count(&pool, &ctx)
            .await
            .expect("count"),
        0,
    );
}

// ---------------------------------------------------------------------------
// House rule: a foreign workspace context is a no-op everywhere, so it can
// neither mutate nor teach.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn a_foreign_workspace_context_records_nothing() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let pool = open_migrated_temp_db(&tmp.path().join("a2_foreign.sqlite")).await;
    let ctx = AuthContext::local();
    let other = other_ws_ctx();

    let meeting = seed_meeting(&pool, &ctx, &["c-1"]).await;
    seed_summary(
        &pool,
        &ctx,
        &meeting,
        vec![block("b1", "c-1", "model metni")],
    )
    .await;

    assert!(
        !SummariesRepository::edit_block(&pool, &other, &meeting, "b1", "sızdırılmış")
            .await
            .expect("edit_block under a foreign context"),
    );
    assert!(!SummariesRepository::set_block_status(
        &pool,
        &other,
        &meeting,
        "b1",
        BlockStatus::Rejected,
        Some("sızdırılmış gerekçe"),
    )
    .await
    .expect("reject under a foreign context"));

    assert!(events(&pool, &ctx, &meeting).await.is_empty());
    assert_eq!(
        CorrectionEventsRepository::count(&pool, &other)
            .await
            .expect("count"),
        0,
    );
}
