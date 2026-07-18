//! Integration tests for the learned-rule lifecycle and policy (ADR-0024 §4/§7,
//! phase C3).
//!
//! What this proves: **rules persist, the status machine holds, the workspace's
//! policy — not the caller — decides whether a mined rule is born active, and the
//! B2 snapshot really does survive the rule being rewritten and deleted.**
//!
//! That last one settles a debt: `applied_rules_snapshot_b2.rs` could only show
//! the text was frozen into the summaries row, because `learned_rules` had no
//! repository to mutate. It does now, so the claim gets tested for real.

use sqlx::migrate::Migrator;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;

use app_lib::api::TranscriptSegment;
use app_lib::context::{AuthContext, RequestId, Role, TenantId, UserId};
use app_lib::database::repositories::{
    learned_rule::{LearnedRuleError, LearnedRulesRepository, NewLearnedRule},
    setting::SettingsRepository,
    summary_draft::SummariesRepository,
    transcript::TranscriptsRepository,
};
use app_lib::learning::config::LearningConfig;
use app_lib::learning::rule::{
    applicable_rules, AppliedRule, RuleKind, RuleOrigin, RuleScope, RuleStatus,
};
use app_lib::summary::draft::{
    BlockStatus, BlockType, DraftBlock, DraftSection, MeetingNotesDraft, SummaryStatus,
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

fn mined(text: &str, support: i64) -> NewLearnedRule {
    NewLearnedRule {
        scope: RuleScope::Global,
        kind: RuleKind::TermSubstitution,
        rule_text: text.to_string(),
        origin: RuleOrigin::MinedDeterministic,
        support_count: support,
        evidence: vec!["e1".to_string(), "e2".to_string()],
        signature: Some(format!("term_substitution|global|{text}")),
    }
}

fn authored(text: &str) -> NewLearnedRule {
    NewLearnedRule {
        scope: RuleScope::Global,
        kind: RuleKind::Freeform,
        rule_text: text.to_string(),
        origin: RuleOrigin::UserAuthored,
        support_count: 0,
        evidence: Vec::new(),
        signature: None,
    }
}

// ---------------------------------------------------------------------------
// The workspace's POLICY decides a mined rule's birth status — not the caller.
// That is why `create` computes it rather than accepting it: otherwise any future
// caller could mint an active rule just by asking, and auto-activation would stop
// being a thing the user controls.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn the_policy_decides_whether_a_mined_rule_is_born_active() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let pool = open_migrated_temp_db(&tmp.path().join("c3_birth.sqlite")).await;
    let ctx = AuthContext::local();
    let config = LearningConfig::default(); // auto_activate on, threshold 3

    let weak = LearnedRulesRepository::create(&pool, &ctx, &mined("Weak.", 2), &config)
        .await
        .expect("create");
    let strong = LearnedRulesRepository::create(&pool, &ctx, &mined("Strong.", 3), &config)
        .await
        .expect("create");

    let all = LearnedRulesRepository::list_all(&pool, &ctx)
        .await
        .expect("list_all");
    let status_of = |id: &str| {
        all.iter()
            .find(|r| r.id == id)
            .unwrap_or_else(|| panic!("rule {id} present"))
            .status
    };
    assert_eq!(status_of(&weak), RuleStatus::Proposed, "below threshold");
    assert_eq!(status_of(&strong), RuleStatus::Active, "at threshold");

    // With auto-activation off, even a well-supported rule waits for the human.
    let manual = LearningConfig {
        auto_activate: false,
        ..LearningConfig::default()
    };
    let id = LearnedRulesRepository::create(&pool, &ctx, &mined("Also strong.", 9), &manual)
        .await
        .expect("create");
    let all = LearnedRulesRepository::list_all(&pool, &ctx)
        .await
        .expect("list_all");
    assert_eq!(
        all.iter().find(|r| r.id == id).unwrap().status,
        RuleStatus::Proposed,
    );
}

/// A user who writes a rule has approved it by writing it. Parking their own
/// instruction in a queue for them to approve would be absurd.
#[tokio::test]
async fn a_user_authored_rule_is_born_active_even_with_auto_activation_off() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let pool = open_migrated_temp_db(&tmp.path().join("c3_authored.sqlite")).await;
    let ctx = AuthContext::local();
    let manual = LearningConfig {
        auto_activate: false,
        ..LearningConfig::default()
    };

    let id =
        LearnedRulesRepository::create(&pool, &ctx, &authored("Prefer active voice."), &manual)
            .await
            .expect("create");

    let all = LearnedRulesRepository::list_all(&pool, &ctx)
        .await
        .expect("list_all");
    assert_eq!(
        all.iter().find(|r| r.id == id).unwrap().status,
        RuleStatus::Active
    );
}

// ---------------------------------------------------------------------------
// The status machine. Nothing returns to `Proposed`: once the human has
// answered, the answer stands until they change it — re-proposing a settled
// question is the nagging this machine exists to prevent.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn the_status_machine_allows_only_the_four_legal_arcs() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let pool = open_migrated_temp_db(&tmp.path().join("c3_machine.sqlite")).await;
    let ctx = AuthContext::local();
    let manual = LearningConfig {
        auto_activate: false,
        ..LearningConfig::default()
    };

    let id = LearnedRulesRepository::create(&pool, &ctx, &mined("A rule.", 1), &manual)
        .await
        .expect("create");

    // proposed → active
    assert!(
        LearnedRulesRepository::set_status(&pool, &ctx, &id, RuleStatus::Active)
            .await
            .expect("activate")
    );
    // active → active is a self-transition, and illegal like every other
    assert!(
        !LearnedRulesRepository::set_status(&pool, &ctx, &id, RuleStatus::Active)
            .await
            .expect("self-transition"),
    );
    // active → proposed is not an arc: the human already decided
    assert!(
        !LearnedRulesRepository::set_status(&pool, &ctx, &id, RuleStatus::Proposed)
            .await
            .expect("no way back to proposed"),
    );
    // active → dismissed → active (the user changing their mind)
    assert!(
        LearnedRulesRepository::set_status(&pool, &ctx, &id, RuleStatus::Dismissed)
            .await
            .expect("dismiss"),
    );
    assert!(
        LearnedRulesRepository::set_status(&pool, &ctx, &id, RuleStatus::Active)
            .await
            .expect("revive")
    );

    assert!(!LearnedRulesRepository::set_status(
        &pool,
        &ctx,
        "no-such-rule",
        RuleStatus::Dismissed
    )
    .await
    .expect("unknown rule is a soft no-op"),);
}

// ---------------------------------------------------------------------------
// Only ACTIVE rules reach a prompt, and the master switch beats the table.
// `list_active` returns empty when learning is off rather than trusting every
// caller to remember — a caller who forgot would keep injecting rules into a
// workspace that turned the feature off, invisibly.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn only_active_rules_are_listed_and_the_master_switch_wins() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let pool = open_migrated_temp_db(&tmp.path().join("c3_active.sqlite")).await;
    let ctx = AuthContext::local();
    let config = LearningConfig::default();

    LearnedRulesRepository::create(&pool, &ctx, &mined("Active one.", 5), &config)
        .await
        .expect("create");
    let proposed = LearnedRulesRepository::create(&pool, &ctx, &mined("Proposed one.", 1), &config)
        .await
        .expect("create");
    let dismissed =
        LearnedRulesRepository::create(&pool, &ctx, &mined("Dismissed one.", 5), &config)
            .await
            .expect("create");
    LearnedRulesRepository::set_status(&pool, &ctx, &dismissed, RuleStatus::Dismissed)
        .await
        .expect("dismiss");

    let active = LearnedRulesRepository::list_active(&pool, &ctx, &config)
        .await
        .expect("list_active");
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].rule_text, "Active one.");
    assert!(!active.iter().any(|r| r.id == proposed));

    let off = LearningConfig {
        enabled: false,
        ..LearningConfig::default()
    };
    assert!(
        LearnedRulesRepository::list_active(&pool, &ctx, &off)
            .await
            .expect("list_active")
            .is_empty(),
        "the master switch must beat whatever is in the table",
    );
}

// ---------------------------------------------------------------------------
// Dismiss and delete are DIFFERENT. Dismiss keeps the row (the human's "no" is
// on the record, so the miner stops offering it); delete removes it from the
// list and carries no memory of refusal, so the behaviour can be re-learned.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn delete_removes_the_rule_while_dismiss_keeps_it_on_the_record() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let pool = open_migrated_temp_db(&tmp.path().join("c3_delete.sqlite")).await;
    let ctx = AuthContext::local();
    let config = LearningConfig::default();

    let dismissed = LearnedRulesRepository::create(&pool, &ctx, &mined("Dismissed.", 5), &config)
        .await
        .expect("create");
    let deleted = LearnedRulesRepository::create(&pool, &ctx, &mined("Deleted.", 5), &config)
        .await
        .expect("create");

    LearnedRulesRepository::set_status(&pool, &ctx, &dismissed, RuleStatus::Dismissed)
        .await
        .expect("dismiss");
    assert!(LearnedRulesRepository::soft_delete(&pool, &ctx, &deleted)
        .await
        .expect("delete"));

    let all = LearnedRulesRepository::list_all(&pool, &ctx)
        .await
        .expect("list_all");
    assert!(
        all.iter().any(|r| r.id == dismissed),
        "a dismissed rule stays visible — the refusal is the record",
    );
    assert!(
        !all.iter().any(|r| r.id == deleted),
        "a deleted rule leaves the list entirely",
    );

    // Deleting twice is a no-op, not an error.
    assert!(!LearnedRulesRepository::soft_delete(&pool, &ctx, &deleted)
        .await
        .expect("second delete"));
}

// ---------------------------------------------------------------------------
// The B2 debt, settled. The snapshot claimed to survive the rule being rewritten
// and deleted; back then `learned_rules` had no repository, so the test could
// only show the text was frozen into the row. Now it can be shown properly.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn the_b2_snapshot_survives_the_real_rule_being_rewritten_and_deleted() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let pool = open_migrated_temp_db(&tmp.path().join("c3_snapshot.sqlite")).await;
    let ctx = AuthContext::local();
    let config = LearningConfig::default();

    let segments = vec![TranscriptSegment {
        id: "c-1".to_string(),
        text: "segment".to_string(),
        timestamp: "2026-07-17T10:00:00.000Z".to_string(),
        audio_start_time: Some(0.0),
        audio_end_time: Some(1.0),
        duration: Some(1.0),
    }];
    let meeting =
        TranscriptsRepository::create_meeting_with_segments(&pool, &ctx, "Saha", &segments, None)
            .await
            .expect("meeting");

    let rule_id = LearnedRulesRepository::create(
        &pool,
        &ctx,
        &mined("Call follow-ups \"takip\", never \"aksiyon\".", 5),
        &config,
    )
    .await
    .expect("create");

    let rules = LearnedRulesRepository::list_active(&pool, &ctx, &config)
        .await
        .expect("list_active");
    let applied: Vec<AppliedRule> = applicable_rules(&rules, Some("daily_standup"))
        .into_iter()
        .map(AppliedRule::from_rule)
        .collect();
    assert_eq!(applied.len(), 1);

    SummariesRepository::upsert_draft(
        &pool,
        &ctx,
        &MeetingNotesDraft {
            meeting_id: meeting.clone(),
            status: SummaryStatus::Draft,
            sections: vec![DraftSection {
                title: "Kararlar".to_string(),
                blocks: vec![DraftBlock {
                    id: "b1".to_string(),
                    block_type: BlockType::Bullet,
                    content: "3 takip maddesi".to_string(),
                    source_chunk_id: "c-1".to_string(),
                    status: BlockStatus::Draft,
                    original_content: None,
                }],
            }],
        },
        Some("llama3.2"),
        Some("daily_standup"),
        &applied,
    )
    .await
    .expect("upsert_draft");

    // Now do the two things a live reference could not survive.
    assert!(
        LearnedRulesRepository::edit_text(&pool, &ctx, &rule_id, "Something else entirely.")
            .await
            .expect("edit"),
    );
    assert!(LearnedRulesRepository::soft_delete(&pool, &ctx, &rule_id)
        .await
        .expect("delete"));

    let row = SummariesRepository::get_by_meeting(&pool, &ctx, &meeting)
        .await
        .expect("get_by_meeting")
        .expect("summary row");
    let snapshot = row.applied_rules.expect("recorded");
    assert_eq!(snapshot.len(), 1);
    assert_eq!(snapshot[0].rule_id, rule_id);
    assert_eq!(
        snapshot[0].rule_text, "Call follow-ups \"takip\", never \"aksiyon\".",
        "the summary must still explain itself with the text that actually shaped it",
    );
}

/// Editing refines a rule the user already agreed to; it must not knock it back
/// into the approval queue, and it must not disown the corrections it was mined
/// from.
#[tokio::test]
async fn editing_a_rule_keeps_its_status_origin_and_evidence() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let pool = open_migrated_temp_db(&tmp.path().join("c3_edit.sqlite")).await;
    let ctx = AuthContext::local();
    let config = LearningConfig::default();

    let id = LearnedRulesRepository::create(&pool, &ctx, &mined("Rough wording.", 5), &config)
        .await
        .expect("create");
    assert!(
        LearnedRulesRepository::edit_text(&pool, &ctx, &id, "  Better wording.  ")
            .await
            .expect("edit"),
    );

    let all = LearnedRulesRepository::list_all(&pool, &ctx)
        .await
        .expect("list_all");
    let rule = all.iter().find(|r| r.id == id).expect("present");
    assert_eq!(rule.rule_text, "Better wording.", "trimmed");
    assert_eq!(rule.status, RuleStatus::Active, "still in force");
    assert_eq!(rule.origin, RuleOrigin::MinedDeterministic, "still mined");
    assert_eq!(
        LearnedRulesRepository::evidence_ids(&pool, &ctx, &id)
            .await
            .expect("evidence"),
        Some(vec!["e1".to_string(), "e2".to_string()]),
        "the corrections it came from are unchanged",
    );

    assert!(matches!(
        LearnedRulesRepository::edit_text(&pool, &ctx, &id, "   ").await,
        Err(LearnedRuleError::BlankText),
    ));
}

// ---------------------------------------------------------------------------
// The config round-trips, and — the part that matters — a CORRUPT blob falls
// back to disabled rather than to the default, so a preference we cannot read
// never becomes an implicit yes to auto-activation.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn an_unreadable_config_disables_rather_than_defaulting() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let pool = open_migrated_temp_db(&tmp.path().join("c3_config.sqlite")).await;
    let ctx = AuthContext::local();

    // Absent: no preference was ever expressed → the product default.
    assert_eq!(
        SettingsRepository::get_learning_config(&pool, &ctx)
            .await
            .expect("absent config"),
        LearningConfig::default(),
    );

    let chosen = LearningConfig {
        auto_activate: false,
        auto_activate_min_support: 5,
        ..LearningConfig::default()
    };
    SettingsRepository::set_learning_config(&pool, &ctx, &chosen)
        .await
        .expect("save");
    assert_eq!(
        SettingsRepository::get_learning_config(&pool, &ctx)
            .await
            .expect("round trip"),
        chosen,
    );

    // Corrupt: the user HAD preferences and we cannot read them.
    sqlx::query("UPDATE settings SET learningConfig = '{not json' WHERE id = '1'")
        .execute(&pool)
        .await
        .expect("corrupt the blob");
    let recovered = SettingsRepository::get_learning_config(&pool, &ctx)
        .await
        .expect("corrupt config still resolves");
    assert_eq!(recovered, LearningConfig::disabled());
    assert!(
        !recovered.auto_activate,
        "a lost preference must never become an implicit yes",
    );
    assert_ne!(recovered, LearningConfig::default());
}

// ---------------------------------------------------------------------------
// House rule: a foreign workspace can neither see nor touch these rules.
// ---------------------------------------------------------------------------
#[tokio::test]
async fn rules_are_invisible_and_immutable_across_workspaces() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let pool = open_migrated_temp_db(&tmp.path().join("c3_tenant.sqlite")).await;
    let ctx = AuthContext::local();
    let other = other_ws_ctx();
    let config = LearningConfig::default();

    let id = LearnedRulesRepository::create(&pool, &ctx, &mined("Local rule.", 5), &config)
        .await
        .expect("create");

    assert!(LearnedRulesRepository::list_all(&pool, &other)
        .await
        .expect("list_all")
        .is_empty());
    assert!(LearnedRulesRepository::list_active(&pool, &other, &config)
        .await
        .expect("list_active")
        .is_empty());
    assert_eq!(
        LearnedRulesRepository::evidence_ids(&pool, &other, &id)
            .await
            .expect("evidence"),
        None,
    );
    assert!(
        !LearnedRulesRepository::set_status(&pool, &other, &id, RuleStatus::Dismissed)
            .await
            .expect("foreign set_status"),
    );
    assert!(
        !LearnedRulesRepository::edit_text(&pool, &other, &id, "hijacked")
            .await
            .expect("foreign edit"),
    );
    assert!(!LearnedRulesRepository::soft_delete(&pool, &other, &id)
        .await
        .expect("foreign delete"));

    // The local rule is untouched by all of that.
    let all = LearnedRulesRepository::list_all(&pool, &ctx)
        .await
        .expect("list_all");
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].rule_text, "Local rule.");
    assert_eq!(all[0].status, RuleStatus::Active);
}
