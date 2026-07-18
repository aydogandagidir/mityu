//! Integration tests for the applied-rules snapshot (ADR-0024 §5, phase B2).
//!
//! What this proves: **a summary carries a record of the learned rules that
//! shaped it, that record is a copy rather than a reference, and "we checked and
//! applied nothing" stays distinguishable from "we never recorded anything".**
//!
//! Why it matters more than it looks: the snapshot is the EU-AI-Act Art.50
//! reproducibility requirement, and it is the PRECONDITION that makes
//! auto-activation (§7) defensible. Without it, a rule the user never explicitly
//! approved could shape a summary and leave no trace of having done so — and six
//! months later "why does this read like this?" would have no answer. So these
//! tests gate a feature that does not exist yet, deliberately.
//!
//! Same harness as `correction_capture_a2.rs`: the app's real, compile-time
//! embedded migration set, driven at the repository layer.

use sqlx::migrate::Migrator;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;

use app_lib::api::TranscriptSegment;
use app_lib::context::AuthContext;
use app_lib::database::repositories::{
    summary_draft::SummariesRepository, transcript::TranscriptsRepository,
};
use app_lib::learning::rule::{
    applicable_rules, AppliedRule, LearnedRule, RuleKind, RuleOrigin, RuleScope, RuleStatus,
};
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

async fn seed_meeting(pool: &SqlitePool, ctx: &AuthContext) -> String {
    let segments = vec![TranscriptSegment {
        id: "c-1".to_string(),
        text: "segment c-1".to_string(),
        timestamp: "2026-07-17T10:00:00.000Z".to_string(),
        audio_start_time: Some(0.0),
        audio_end_time: Some(1.0),
        duration: Some(1.0),
    }];
    TranscriptsRepository::create_meeting_with_segments(pool, ctx, "Saha", &segments, None)
        .await
        .expect("create_meeting_with_segments")
}

fn notes(meeting_id: &str, content: &str) -> MeetingNotesDraft {
    MeetingNotesDraft {
        meeting_id: meeting_id.to_string(),
        status: SummaryStatus::Draft,
        sections: vec![DraftSection {
            title: "Kararlar".to_string(),
            blocks: vec![DraftBlock {
                id: "b1".to_string(),
                block_type: BlockType::Bullet,
                content: content.to_string(),
                source_chunk_id: "c-1".to_string(),
                status: BlockStatus::Draft,
                original_content: None,
            }],
        }],
    }
}

fn rule(id: &str, scope: RuleScope, text: &str) -> LearnedRule {
    LearnedRule {
        id: id.to_string(),
        scope,
        kind: RuleKind::TermSubstitution,
        rule_text: text.to_string(),
        status: RuleStatus::Active,
        origin: RuleOrigin::MinedDeterministic,
        support_count: 3,
    }
}

// ---------------------------------------------------------------------------
// The snapshot is a COPY, not a reference. That is the whole reason the column
// stores text instead of a foreign key: rules are editable and deletable, so a
// summary that merely pointed at one would become unexplainable the moment the
// user tidied their rule list — exactly when an audit is most likely to be asked
// for.
//
// Scope of this test, stated plainly: it proves the text is FROZEN INTO the
// summaries row and read back without consulting any rule (the rules are dropped
// before the read, so a lazy reference could not survive). It does NOT exercise
// editing or deleting a stored rule — `learned_rules` has no repository yet
// (§4, phase C3). That case is owed once C3 lands.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn the_snapshot_is_a_copy_not_a_reference() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let pool = open_migrated_temp_db(&tmp.path().join("b2_copy.sqlite")).await;
    let ctx = AuthContext::local();
    let meeting = seed_meeting(&pool, &ctx).await;

    let rules = [rule(
        "r1",
        RuleScope::Global,
        "Call follow-ups \"takip\", never \"aksiyon\".",
    )];
    let applied: Vec<AppliedRule> = applicable_rules(&rules, Some("daily_standup"))
        .into_iter()
        .map(AppliedRule::from_rule)
        .collect();

    SummariesRepository::upsert_draft(
        &pool,
        &ctx,
        &notes(&meeting, "3 takip maddesi çıktı"),
        Some("llama3.2"),
        Some("daily_standup"),
        &applied,
    )
    .await
    .expect("upsert_draft");

    // Nothing may be consulted at read time except the summaries row itself.
    drop(rules);

    let row = SummariesRepository::get_by_meeting(&pool, &ctx, &meeting)
        .await
        .expect("get_by_meeting")
        .expect("summary row");
    let snapshot = row.applied_rules.expect("the snapshot was recorded");
    assert_eq!(snapshot.len(), 1);
    assert_eq!(snapshot[0].rule_id, "r1");
    assert_eq!(
        snapshot[0].rule_text, "Call follow-ups \"takip\", never \"aksiyon\".",
        "the TEXT must be frozen into the summary, not fetched from the rule",
    );
    assert_eq!(snapshot[0].scope, "global");
}

// ---------------------------------------------------------------------------
// `[]` and NULL are different claims. `[]` says "the learning system ran and
// applied nothing"; NULL says "nothing was ever recorded here". An audit needs
// to tell "no" from "we don't know", so the read path must not collapse them.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn an_empty_snapshot_is_a_claim_and_null_is_not() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let pool = open_migrated_temp_db(&tmp.path().join("b2_empty.sqlite")).await;
    let ctx = AuthContext::local();
    let meeting = seed_meeting(&pool, &ctx).await;

    SummariesRepository::upsert_draft(
        &pool,
        &ctx,
        &notes(&meeting, "model metni"),
        Some("llama3.2"),
        Some("daily_standup"),
        &[],
    )
    .await
    .expect("upsert_draft");

    let row = SummariesRepository::get_by_meeting(&pool, &ctx, &meeting)
        .await
        .expect("get_by_meeting")
        .expect("summary row");
    assert_eq!(
        row.applied_rules,
        Some(Vec::new()),
        "generating with no rules must RECORD that, not leave it unknown",
    );

    // A row whose column was never written — a pre-migration summary, the
    // first-run sample, or the legacy markdown path — reads as "unknown".
    sqlx::query("UPDATE summaries SET applied_rules = NULL WHERE meeting_id = ?")
        .bind(&meeting)
        .execute(&pool)
        .await
        .expect("simulate an unrecorded row");

    let row = SummariesRepository::get_by_meeting(&pool, &ctx, &meeting)
        .await
        .expect("get_by_meeting")
        .expect("summary row");
    assert_eq!(row.applied_rules, None, "NULL is not an empty snapshot");
}

// ---------------------------------------------------------------------------
// The snapshot describes the draft CURRENTLY in the row, not the meeting's
// history — so regeneration replaces it. A stale snapshot would be worse than
// none: it would confidently explain a summary that no longer exists.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn regeneration_replaces_the_snapshot_rather_than_accumulating_it() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let pool = open_migrated_temp_db(&tmp.path().join("b2_regen.sqlite")).await;
    let ctx = AuthContext::local();
    let meeting = seed_meeting(&pool, &ctx).await;

    let first = [rule("r1", RuleScope::Global, "First rule.")];
    let applied: Vec<AppliedRule> = first.iter().map(AppliedRule::from_rule).collect();
    SummariesRepository::upsert_draft(
        &pool,
        &ctx,
        &notes(&meeting, "ilk üretim"),
        Some("llama3.2"),
        Some("daily_standup"),
        &applied,
    )
    .await
    .expect("first upsert");

    // Regenerate after the user dismissed that rule and activated another.
    let second = [rule(
        "r2",
        RuleScope::Template("daily_standup".to_string()),
        "Second rule.",
    )];
    let applied: Vec<AppliedRule> = second.iter().map(AppliedRule::from_rule).collect();
    SummariesRepository::upsert_draft(
        &pool,
        &ctx,
        &notes(&meeting, "ikinci üretim"),
        Some("llama3.2"),
        Some("daily_standup"),
        &applied,
    )
    .await
    .expect("regenerate");

    let row = SummariesRepository::get_by_meeting(&pool, &ctx, &meeting)
        .await
        .expect("get_by_meeting")
        .expect("summary row");
    let snapshot = row.applied_rules.expect("recorded");
    assert_eq!(
        snapshot.len(),
        1,
        "the snapshot is replaced, not appended to"
    );
    assert_eq!(snapshot[0].rule_id, "r2");
    assert_eq!(snapshot[0].scope, "template:daily_standup");
}

// ---------------------------------------------------------------------------
// The order in the snapshot must be the order that went into the prompt: the
// snapshot's job is to reproduce the generation, and rule order changes the
// prompt. `applicable_rules` sorts by id; this asserts the snapshot inherits it
// rather than whatever order the rules were handed over in.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn the_snapshot_preserves_the_prompt_order() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let pool = open_migrated_temp_db(&tmp.path().join("b2_order.sqlite")).await;
    let ctx = AuthContext::local();
    let meeting = seed_meeting(&pool, &ctx).await;

    // Handed over out of order, and with one rule that must NOT apply.
    let rules = [
        rule("r3", RuleScope::Global, "Third."),
        rule("r1", RuleScope::Global, "First."),
        rule(
            "r9",
            RuleScope::Template("other_template".to_string()),
            "Must not apply.",
        ),
        rule("r2", RuleScope::Section("Kararlar".to_string()), "Second."),
    ];
    let applied: Vec<AppliedRule> = applicable_rules(&rules, Some("daily_standup"))
        .into_iter()
        .map(AppliedRule::from_rule)
        .collect();

    SummariesRepository::upsert_draft(
        &pool,
        &ctx,
        &notes(&meeting, "model metni"),
        Some("llama3.2"),
        Some("daily_standup"),
        &applied,
    )
    .await
    .expect("upsert_draft");

    let row = SummariesRepository::get_by_meeting(&pool, &ctx, &meeting)
        .await
        .expect("get_by_meeting")
        .expect("summary row");
    let ids: Vec<String> = row
        .applied_rules
        .expect("recorded")
        .into_iter()
        .map(|r| r.rule_id)
        .collect();
    assert_eq!(
        ids,
        ["r1", "r2", "r3"],
        "id order, and the foreign-template rule is absent",
    );
}
