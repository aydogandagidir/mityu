//! End-to-end mining (ADR-0024 §8, phase C1).
//!
//! `learning::miner`'s own unit tests pin the mining LOGIC over synthetic events.
//! This file proves the part they cannot: that real corrections, written by the
//! real HITL commands into the real log, come back out as a persisted rule — and
//! that the signature really does stop the second pass re-proposing it.
//!
//! In other words: the loop, closed, on a database.

use sqlx::migrate::Migrator;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;

use app_lib::api::TranscriptSegment;
use app_lib::context::AuthContext;
use app_lib::database::repositories::{
    learned_rule::LearnedRulesRepository, setting::SettingsRepository,
    summary_draft::SummariesRepository, transcript::TranscriptsRepository,
};
use app_lib::learning::config::LearningConfig;
use app_lib::learning::rule::{applicable_rules, RuleKind, RuleOrigin, RuleStatus};
use app_lib::summary::draft::{
    BlockStatus, BlockType, DraftBlock, DraftSection, MeetingNotesDraft, SummaryStatus,
};

static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

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

/// Seeds a meeting with a one-block summary saying `content`.
async fn seed(pool: &SqlitePool, ctx: &AuthContext, n: usize, content: &str) -> String {
    let segments = vec![TranscriptSegment {
        id: format!("c-{n}"),
        text: "segment".to_string(),
        timestamp: "2026-07-17T10:00:00.000Z".to_string(),
        audio_start_time: Some(0.0),
        audio_end_time: Some(1.0),
        duration: Some(1.0),
    }];
    let meeting = TranscriptsRepository::create_meeting_with_segments(
        pool,
        ctx,
        &format!("Toplantı {n}"),
        &segments,
        None,
    )
    .await
    .expect("meeting");

    SummariesRepository::upsert_draft(
        pool,
        ctx,
        &MeetingNotesDraft {
            meeting_id: meeting.clone(),
            status: SummaryStatus::Draft,
            sections: vec![DraftSection {
                title: "Kararlar".to_string(),
                blocks: vec![DraftBlock {
                    id: format!("b-{n}"),
                    block_type: BlockType::Bullet,
                    content: content.to_string(),
                    source_chunk_id: format!("c-{n}"),
                    status: BlockStatus::Draft,
                    original_content: None,
                }],
            }],
        },
        Some("llama3.2"),
        Some("daily_standup"),
        &[],
    )
    .await
    .expect("upsert_draft");
    meeting
}

/// Runs the same pass `api_mine_learned_rules` and the approve hook both call.
async fn mine(pool: &SqlitePool, ctx: &AuthContext) -> usize {
    app_lib::learning::commands::mine_and_persist(pool, ctx)
        .await
        .expect("mine_and_persist")
}

// ---------------------------------------------------------------------------
// THE test for this phase: three real corrections, made through the real HITL
// path, become a real rule that a real generation would inject.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn three_real_corrections_become_a_rule_that_reaches_the_prompt() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let pool = open_migrated_temp_db(&tmp.path().join("c1_loop.sqlite")).await;
    let ctx = AuthContext::local();

    // The user fixes the same word in three different meetings.
    for n in 1..=3 {
        let meeting = seed(&pool, &ctx, n, "3 aksiyon çıktı").await;
        SummariesRepository::edit_block(&pool, &ctx, &meeting, &format!("b-{n}"), "3 takip çıktı")
            .await
            .expect("edit_block");
    }

    assert_eq!(mine(&pool, &ctx).await, 1, "one rule mined");

    let rules = LearnedRulesRepository::list_all(&pool, &ctx)
        .await
        .expect("list_all");
    assert_eq!(rules.len(), 1);
    let rule = &rules[0];
    assert_eq!(rule.rule_text, "Say \"takip\" rather than \"aksiyon\".");
    assert_eq!(rule.kind, RuleKind::TermSubstitution);
    assert_eq!(rule.origin, RuleOrigin::MinedDeterministic);
    assert_eq!(rule.support_count, 3);
    assert_eq!(
        rule.status,
        RuleStatus::Active,
        "3 = the default threshold and auto-activation is on, so it is in force",
    );

    // And the generation path would inject exactly this.
    let active = LearnedRulesRepository::list_active(&pool, &ctx, &LearningConfig::default())
        .await
        .expect("list_active");
    let applicable = applicable_rules(&active, Some("daily_standup"));
    assert_eq!(applicable.len(), 1);
    assert_eq!(
        applicable[0].rule_text,
        "Say \"takip\" rather than \"aksiyon\"."
    );
}

/// Mining twice must not produce the rule twice — the signature is on record.
#[tokio::test]
async fn a_second_pass_proposes_nothing_new() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let pool = open_migrated_temp_db(&tmp.path().join("c1_idem.sqlite")).await;
    let ctx = AuthContext::local();

    for n in 1..=3 {
        let meeting = seed(&pool, &ctx, n, "3 aksiyon çıktı").await;
        SummariesRepository::edit_block(&pool, &ctx, &meeting, &format!("b-{n}"), "3 takip çıktı")
            .await
            .expect("edit_block");
    }

    assert_eq!(mine(&pool, &ctx).await, 1);
    assert_eq!(mine(&pool, &ctx).await, 0, "nothing new the second time");
    assert_eq!(mine(&pool, &ctx).await, 0);
    assert_eq!(
        LearnedRulesRepository::list_all(&pool, &ctx)
            .await
            .expect("list_all")
            .len(),
        1,
    );
}

/// A dismissed rule stays on the record, so the miner stops offering it — the
/// whole reason dismissing keeps the row (§4).
#[tokio::test]
async fn a_dismissed_rule_is_never_mined_again_but_a_deleted_one_can_be() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let pool = open_migrated_temp_db(&tmp.path().join("c1_dismiss.sqlite")).await;
    let ctx = AuthContext::local();

    for n in 1..=3 {
        let meeting = seed(&pool, &ctx, n, "3 aksiyon çıktı").await;
        SummariesRepository::edit_block(&pool, &ctx, &meeting, &format!("b-{n}"), "3 takip çıktı")
            .await
            .expect("edit_block");
    }
    assert_eq!(mine(&pool, &ctx).await, 1);

    let id = LearnedRulesRepository::list_all(&pool, &ctx).await.unwrap()[0]
        .id
        .clone();

    // Dismissed: the human said no, and the miner respects it.
    LearnedRulesRepository::set_status(&pool, &ctx, &id, RuleStatus::Dismissed)
        .await
        .expect("dismiss");
    assert_eq!(
        mine(&pool, &ctx).await,
        0,
        "the human already refused this — asking again is nagging",
    );

    // Deleted: no record of refusal, so the same behaviour can be learned again.
    LearnedRulesRepository::soft_delete(&pool, &ctx, &id)
        .await
        .expect("delete");
    assert_eq!(
        mine(&pool, &ctx).await,
        1,
        "deleting is how a user says 'offer it again if I keep doing it'",
    );
}

/// The master switch beats the log: learning off means nothing is mined at all.
#[tokio::test]
async fn nothing_is_mined_when_learning_is_switched_off() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let pool = open_migrated_temp_db(&tmp.path().join("c1_off.sqlite")).await;
    let ctx = AuthContext::local();

    for n in 1..=3 {
        let meeting = seed(&pool, &ctx, n, "3 aksiyon çıktı").await;
        SummariesRepository::edit_block(&pool, &ctx, &meeting, &format!("b-{n}"), "3 takip çıktı")
            .await
            .expect("edit_block");
    }

    SettingsRepository::set_learning_config(
        &pool,
        &ctx,
        &LearningConfig {
            enabled: false,
            ..LearningConfig::default()
        },
    )
    .await
    .expect("save config");

    assert_eq!(mine(&pool, &ctx).await, 0);
    assert!(LearnedRulesRepository::list_all(&pool, &ctx)
        .await
        .expect("list_all")
        .is_empty());
}

/// With auto-activation off, the same three corrections still produce the rule —
/// they just produce it as a PROPOSAL, waiting for the human.
#[tokio::test]
async fn with_auto_activation_off_the_rule_waits_for_the_human() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let pool = open_migrated_temp_db(&tmp.path().join("c1_manual.sqlite")).await;
    let ctx = AuthContext::local();

    SettingsRepository::set_learning_config(
        &pool,
        &ctx,
        &LearningConfig {
            auto_activate: false,
            ..LearningConfig::default()
        },
    )
    .await
    .expect("save config");

    for n in 1..=3 {
        let meeting = seed(&pool, &ctx, n, "3 aksiyon çıktı").await;
        SummariesRepository::edit_block(&pool, &ctx, &meeting, &format!("b-{n}"), "3 takip çıktı")
            .await
            .expect("edit_block");
    }
    assert_eq!(mine(&pool, &ctx).await, 1);

    let rules = LearnedRulesRepository::list_all(&pool, &ctx)
        .await
        .expect("list_all");
    assert_eq!(rules[0].status, RuleStatus::Proposed);
    assert!(
        LearnedRulesRepository::list_active(&pool, &ctx, &LearningConfig::default())
            .await
            .expect("list_active")
            .is_empty(),
        "a proposal must never reach a prompt",
    );
}

/// Deleting the meetings erases the corrections (§10) — so a rule already mined
/// survives, but the evidence behind it does not, and a fresh workspace could not
/// have mined it. The asymmetry, end to end.
#[tokio::test]
async fn erasing_the_meetings_leaves_the_rule_and_takes_its_evidence() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let pool = open_migrated_temp_db(&tmp.path().join("c1_erase.sqlite")).await;
    let ctx = AuthContext::local();

    let mut meetings = vec![];
    for n in 1..=3 {
        let meeting = seed(&pool, &ctx, n, "3 aksiyon çıktı").await;
        SummariesRepository::edit_block(&pool, &ctx, &meeting, &format!("b-{n}"), "3 takip çıktı")
            .await
            .expect("edit_block");
        meetings.push(meeting);
    }
    assert_eq!(mine(&pool, &ctx).await, 1);
    let id = LearnedRulesRepository::list_all(&pool, &ctx).await.unwrap()[0]
        .id
        .clone();
    let evidence_before = LearnedRulesRepository::evidence_ids(&pool, &ctx, &id)
        .await
        .expect("evidence")
        .expect("present");
    assert_eq!(evidence_before.len(), 3);

    for meeting in &meetings {
        sqlx::query("DELETE FROM meetings WHERE id = ?")
            .bind(meeting)
            .execute(&pool)
            .await
            .expect("delete meeting");
    }

    // The rule stands: it is an abstraction the human has, in effect, approved.
    let rules = LearnedRulesRepository::list_all(&pool, &ctx)
        .await
        .expect("list_all");
    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0].rule_text, "Say \"takip\" rather than \"aksiyon\".");
    // Its evidence list still NAMES three corrections, and all three are gone —
    // which is precisely the dangling state the rules screen reports.
    assert_eq!(
        LearnedRulesRepository::evidence_ids(&pool, &ctx, &id)
            .await
            .expect("evidence")
            .expect("present")
            .len(),
        3,
    );
    // And with the log erased, a second pass has nothing left to learn from.
    assert_eq!(mine(&pool, &ctx).await, 0);
}
