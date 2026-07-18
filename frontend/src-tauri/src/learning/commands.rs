//! Tauri commands for the learned-rule surface (ADR-0030 §9).
//!
//! Thin wrappers in the `summary::commands` mould: resolve identity via
//! `context::current()`, call exactly one repository method, map the typed error
//! to a content-free `String`. No business logic — the status machine, the birth
//! policy and the scope filter all live where they can be unit-tested.
//!
//! **This module is what makes auto-activation defensible.** ADR-0030 §7 permits
//! a mined rule to activate without asking, but only because three things hold,
//! and one of them is that the user can SEE, edit and delete every rule. Before
//! this existed, that bound was a promise; the screen behind these commands is
//! where it becomes true.
//!
//! Wire shapes are flat strings (`scope: "template:daily_standup"`) rather than
//! serde's enum encoding: the frontend gets the same closed vocabulary the
//! database stores, and the tokens are already pinned by tests on both sides of
//! the boundary.

use crate::database::repositories::correction_event::{
    CorrectionAction, CorrectionEventRow, CorrectionEventsRepository,
};
use crate::database::repositories::learned_rule::{
    LearnedRuleError, LearnedRulesRepository, NewLearnedRule,
};
use crate::learning::config::LearningConfig;
use crate::learning::llm_miner::{
    build_llm_miner_prompt, parse_llm_rules, CorrectionSummary, MAX_CORRECTIONS_IN_PROMPT,
};
use crate::learning::rule::{LearnedRule, RuleKind, RuleOrigin, RuleScope, RuleStatus};
use crate::state::AppState;
use crate::{log_error, log_info};
use once_cell::sync::Lazy;
use serde::Serialize;
use std::collections::HashSet;
use std::sync::Mutex;
use tauri::{AppHandle, Manager};

/// Content-free mapping of a repository error to the string the frontend sees.
///
/// Every [`LearnedRuleError`] variant's `Display` is ids/tokens only — rule text
/// never reaches the Tauri error channel or the logs (§0.6).
fn learned_rule_err(err: LearnedRuleError) -> String {
    err.to_string()
}

/// One rule, flattened for the wire.
#[derive(Debug, Clone, Serialize)]
pub struct LearnedRuleView {
    /// Row id.
    pub id: String,
    /// Scope token: `global` | `template:<id>` | `section:<title>`.
    pub scope: String,
    /// Kind token.
    pub kind: String,
    /// The rule in plain language.
    pub rule_text: String,
    /// Status token: `proposed` | `active` | `dismissed`.
    pub status: String,
    /// Origin token: `mined_deterministic` | `mined_llm` | `user_authored`.
    pub origin: String,
    /// How many corrections back it.
    pub support_count: i64,
}

impl From<LearnedRule> for LearnedRuleView {
    fn from(rule: LearnedRule) -> Self {
        Self {
            id: rule.id,
            scope: rule.scope.to_db(),
            kind: rule.kind.to_db().to_string(),
            rule_text: rule.rule_text,
            status: rule.status.to_db().to_string(),
            origin: rule.origin.to_db().to_string(),
            support_count: rule.support_count,
        }
    }
}

/// One correction that backs a rule.
#[derive(Debug, Clone, Serialize)]
pub struct RuleEvidenceView {
    /// Correction-event id.
    pub id: String,
    /// The meeting it happened in — the UI links back to it.
    pub meeting_id: String,
    /// Action token: `edit` | `reject` | `approve` | `restore`.
    pub action: String,
    /// What the model wrote.
    pub original_text: Option<String>,
    /// What the human left behind.
    pub final_text: Option<String>,
    /// Their rationale, if they gave one.
    pub reason: Option<String>,
    /// When (RFC 3339, as stored).
    pub created_at: String,
}

/// A rule's evidence, with the dangling ones counted rather than hidden.
#[derive(Debug, Clone, Serialize)]
pub struct RuleEvidenceResponse {
    /// The corrections that still exist, oldest first.
    pub events: Vec<RuleEvidenceView>,
    /// How many of the rule's corrections are GONE — their meeting was deleted,
    /// and the CASCADE took them (ADR-0030 §10).
    ///
    /// This is a first-class field rather than an error because dangling evidence
    /// is the EXPECTED end state of the erasure asymmetry: the rule is an
    /// abstraction the human approved and survives; the meeting text behind it
    /// does not. The UI says "kanıt silindi" and moves on.
    pub missing_count: usize,
}

/// Every live rule in the workspace, proposed first (§9).
///
/// TS binding: `invoke("api_list_learned_rules") -> LearnedRuleView[]`.
#[tauri::command]
pub async fn api_list_learned_rules(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<LearnedRuleView>, String> {
    log_info!("api_list_learned_rules called");
    let pool = state.db_manager.pool();
    let ctx = crate::context::current();
    LearnedRulesRepository::list_all(pool, &ctx)
        .await
        .map(|rules| rules.into_iter().map(LearnedRuleView::from).collect())
        .map_err(learned_rule_err)
}

/// The user writes their own rule.
///
/// Born ACTIVE regardless of the workspace's auto-activation setting — writing
/// the rule IS the approval (ADR-0030 §7). This is also the manual knob that has
/// to work before the automatic one is credible.
///
/// `scope` is a raw token (`global` | `template:<id>` | `section:<title>`);
/// an unparseable one is a typed error rather than a silent fallback to global,
/// which would quietly widen a rule the user meant to narrow.
///
/// TS binding: `invoke("api_create_learned_rule", { ruleText, scope }) -> string`.
#[tauri::command]
pub async fn api_create_learned_rule(
    state: tauri::State<'_, AppState>,
    rule_text: String,
    scope: Option<String>,
) -> Result<String, String> {
    // Token only — never the rule text (§0.6).
    log_info!(
        "api_create_learned_rule called (scope: {})",
        scope.as_deref().unwrap_or("global")
    );
    let scope = match scope.as_deref() {
        None => RuleScope::Global,
        Some(token) => RuleScope::from_db(token).map_err(|e| e.to_string())?,
    };
    let pool = state.db_manager.pool();
    let ctx = crate::context::current();
    let config =
        crate::database::repositories::setting::SettingsRepository::get_learning_config(pool, &ctx)
            .await
            .map_err(|e| {
                log_error!("Failed to read learning config: {}", e);
                format!("Failed to read the learning configuration: {}", e)
            })?;

    LearnedRulesRepository::create(
        pool,
        &ctx,
        &NewLearnedRule {
            scope,
            kind: RuleKind::Freeform,
            rule_text,
            origin: RuleOrigin::UserAuthored,
            support_count: 0,
            evidence: Vec::new(),
            // No miner produced this, so no miner can re-propose it — the
            // signature space belongs to the miner alone.
            signature: None,
        },
        &config,
    )
    .await
    .map_err(learned_rule_err)
}

/// Put a rule in force (`proposed → active`, or `dismissed → active` when the
/// user changes their mind). `Ok(false)` = illegal transition or unknown rule.
///
/// TS binding: `invoke("api_activate_learned_rule", { ruleId }) -> boolean`.
#[tauri::command]
pub async fn api_activate_learned_rule(
    state: tauri::State<'_, AppState>,
    rule_id: String,
) -> Result<bool, String> {
    log_info!("api_activate_learned_rule called for rule_id: {}", rule_id);
    let pool = state.db_manager.pool();
    let ctx = crate::context::current();
    LearnedRulesRepository::set_status(pool, &ctx, &rule_id, RuleStatus::Active)
        .await
        .map_err(learned_rule_err)
}

/// The human says no, ON THE RECORD (`proposed|active → dismissed`).
///
/// Distinct from [`api_delete_learned_rule`]: dismissing keeps the row, which is
/// what stops the miner offering the same thing again. `Ok(false)` = illegal
/// transition or unknown rule.
///
/// TS binding: `invoke("api_dismiss_learned_rule", { ruleId }) -> boolean`.
#[tauri::command]
pub async fn api_dismiss_learned_rule(
    state: tauri::State<'_, AppState>,
    rule_id: String,
) -> Result<bool, String> {
    log_info!("api_dismiss_learned_rule called for rule_id: {}", rule_id);
    let pool = state.db_manager.pool();
    let ctx = crate::context::current();
    LearnedRulesRepository::set_status(pool, &ctx, &rule_id, RuleStatus::Dismissed)
        .await
        .map_err(learned_rule_err)
}

/// The user rewrites a rule in their own words. Keeps status, origin and
/// evidence — they are refining something they already agreed to.
///
/// TS binding: `invoke("api_edit_learned_rule", { ruleId, ruleText }) -> boolean`.
#[tauri::command]
pub async fn api_edit_learned_rule(
    state: tauri::State<'_, AppState>,
    rule_id: String,
    rule_text: String,
) -> Result<bool, String> {
    // id only — never the text (§0.6).
    log_info!("api_edit_learned_rule called for rule_id: {}", rule_id);
    let pool = state.db_manager.pool();
    let ctx = crate::context::current();
    LearnedRulesRepository::edit_text(pool, &ctx, &rule_id, &rule_text)
        .await
        .map_err(learned_rule_err)
}

/// Remove a rule from the user's list.
///
/// NOT "never suggest this again" — that is [`api_dismiss_learned_rule`]. A
/// deleted rule carries no record of refusal, so the same behaviour can be
/// learned again. The UI must say so.
///
/// TS binding: `invoke("api_delete_learned_rule", { ruleId }) -> boolean`.
#[tauri::command]
pub async fn api_delete_learned_rule(
    state: tauri::State<'_, AppState>,
    rule_id: String,
) -> Result<bool, String> {
    log_info!("api_delete_learned_rule called for rule_id: {}", rule_id);
    let pool = state.db_manager.pool();
    let ctx = crate::context::current();
    LearnedRulesRepository::soft_delete(pool, &ctx, &rule_id)
        .await
        .map_err(learned_rule_err)
}

/// The corrections behind a rule — "why does Mityu think this?" (§9).
///
/// Missing evidence is COUNTED, not an error: a rule outlives the meetings it was
/// mined from, by design (§10).
///
/// TS binding: `invoke("api_get_rule_evidence", { ruleId }) -> RuleEvidenceResponse`.
#[tauri::command]
pub async fn api_get_rule_evidence(
    state: tauri::State<'_, AppState>,
    rule_id: String,
) -> Result<RuleEvidenceResponse, String> {
    log_info!("api_get_rule_evidence called for rule_id: {}", rule_id);
    let pool = state.db_manager.pool();
    let ctx = crate::context::current();

    let ids = LearnedRulesRepository::evidence_ids(pool, &ctx, &rule_id)
        .await
        .map_err(learned_rule_err)?
        .unwrap_or_default();

    let events = CorrectionEventsRepository::list_by_ids(pool, &ctx, &ids)
        .await
        .map_err(|e| e.to_string())?;

    // Whatever the rule cited but the log no longer holds has been erased with
    // its meeting. Saturating because a corrupt evidence list must not panic on
    // an audit screen.
    let missing_count = ids.len().saturating_sub(events.len());

    Ok(RuleEvidenceResponse {
        events: events
            .into_iter()
            .map(|e| RuleEvidenceView {
                id: e.id,
                meeting_id: e.meeting_id,
                action: match e.action {
                    crate::database::repositories::correction_event::CorrectionAction::Edit => {
                        "edit"
                    }
                    crate::database::repositories::correction_event::CorrectionAction::Reject => {
                        "reject"
                    }
                    crate::database::repositories::correction_event::CorrectionAction::Approve => {
                        "approve"
                    }
                    crate::database::repositories::correction_event::CorrectionAction::Restore => {
                        "restore"
                    }
                }
                .to_string(),
                original_text: e.original_text,
                final_text: e.final_text,
                reason: e.reason,
                created_at: e.created_at,
            })
            .collect(),
        missing_count,
    })
}

/// Run the deterministic miners over this workspace's correction log and persist
/// whatever is new (ADR-0030 §8).
///
/// Returns how many rules were created. Idempotent in practice: a signature
/// already on record in any status is never re-proposed, so calling this twice in
/// a row creates nothing the second time.
///
/// Cheap by construction — no model, no network, arithmetic over text the user
/// already produced — which is why it can run on a natural trigger rather than
/// behind a button.
///
/// TS binding: `invoke("api_mine_learned_rules") -> number`.
#[tauri::command]
pub async fn api_mine_learned_rules(state: tauri::State<'_, AppState>) -> Result<usize, String> {
    log_info!("api_mine_learned_rules called");
    let pool = state.db_manager.pool();
    let ctx = crate::context::current();
    mine_and_persist(pool, &ctx).await.map_err(|e| {
        log_error!("Mining failed: {}", e);
        e
    })
}

/// The mining pass, callable from anywhere that has a pool (the command above,
/// and the approve-summary hook).
///
/// Returns the number of rules created.
pub async fn mine_and_persist(
    pool: &sqlx::SqlitePool,
    ctx: &crate::context::AuthContext,
) -> Result<usize, String> {
    let config =
        crate::database::repositories::setting::SettingsRepository::get_learning_config(pool, ctx)
            .await
            .map_err(|e| format!("Failed to read the learning configuration: {}", e))?;
    if !config.enabled {
        return Ok(0);
    }

    // A bound, not a preference: mining is O(events) and this is a local app, but
    // an unbounded scan would grow without limit for a heavy user, and rules
    // worth learning show up in recent behaviour anyway. Logged when it bites so
    // a silent cap can never masquerade as "nothing to learn".
    const MINING_WINDOW: i64 = 2_000;
    let events = CorrectionEventsRepository::list_recent(pool, ctx, MINING_WINDOW)
        .await
        .map_err(|e| e.to_string())?;
    if events.len() as i64 == MINING_WINDOW {
        log_info!(
            "Mining over the {} most recent corrections; older ones are outside the window",
            MINING_WINDOW
        );
    }

    let known = LearnedRulesRepository::live_signatures(pool, ctx)
        .await
        .map_err(learned_rule_err)?;
    let candidates =
        crate::learning::miner::mine(&events, config.auto_activate_min_support, &known);

    let mut created = 0;
    for candidate in candidates {
        let new_rule = NewLearnedRule {
            scope: candidate.scope,
            kind: candidate.kind,
            rule_text: candidate.rule_text,
            origin: RuleOrigin::MinedDeterministic,
            support_count: candidate.support_count,
            evidence: candidate.evidence,
            signature: Some(candidate.signature),
        };
        match LearnedRulesRepository::create(pool, ctx, &new_rule, &config).await {
            Ok(_) => created += 1,
            Err(e) => {
                // One bad candidate must not sink the pass.
                log_error!("Failed to persist a mined rule: {}", e);
            }
        }
    }
    log_info!("Mining pass created {} rule(s)", created);
    Ok(created)
}

/// The fewest corrections worth paying a model to read. Below this there is no
/// "pattern across several corrections" for it to find (the prompt says exactly
/// that), so a call would spend tokens to be told nothing. It gates the whole
/// PASS, distinct from the deterministic miner's per-rule support threshold.
const MIN_CORRECTIONS_FOR_LLM: usize = 3;

/// Workspaces with an LLM mining pass currently running.
///
/// The trigger is every approval, and one model call outlives the gap between two
/// quick approvals, so without this a burst of approvals fans out into concurrent
/// identical passes — wasted tokens and a race to create the same rule. The
/// signature dedup would collapse the duplicate rules anyway, but not paying for
/// the second call is better.
static LLM_MINE_IN_FLIGHT: Lazy<Mutex<HashSet<String>>> = Lazy::new(|| Mutex::new(HashSet::new()));

/// An RAII claim on a workspace's LLM-mining slot. [`Self::acquire`] returns
/// `None` when a pass is already running for that workspace; otherwise the guard
/// frees the slot on drop — on every path, including error and panic, so a failed
/// pass can never wedge the flag on.
struct InFlightGuard(String);

impl InFlightGuard {
    fn acquire(workspace_id: &str) -> Option<Self> {
        let mut set = LLM_MINE_IN_FLIGHT.lock().ok()?;
        if set.insert(workspace_id.to_string()) {
            Some(Self(workspace_id.to_string()))
        } else {
            None
        }
    }
}

impl Drop for InFlightGuard {
    fn drop(&mut self) {
        if let Ok(mut set) = LLM_MINE_IN_FLIGHT.lock() {
            set.remove(&self.0);
        }
    }
}

/// Reduce raw correction events to the model→human deltas the LLM miner reads.
///
/// Only [`CorrectionAction::Edit`] and [`CorrectionAction::Reject`] carry a
/// preference signal. An `Approve` of an untouched block is the model getting it
/// right — nothing to learn — and a `Restore` retracts an earlier reject, a
/// signal the user took back; both are dropped. An event with no `original_text`
/// (nothing the model wrote) cannot show a delta and is skipped.
///
/// Pure and borrowing: [`CorrectionSummary`] holds `&str` into `events`, so the
/// caller keeps `events` alive while it builds the prompt. This is the one piece
/// worth unit-testing without a provider.
fn summaries_from_events(events: &[CorrectionEventRow]) -> Vec<CorrectionSummary<'_>> {
    let mut out = Vec::new();
    for event in events {
        match event.action {
            CorrectionAction::Edit => {
                let Some(original) = event.original_text.as_deref() else {
                    continue;
                };
                out.push(CorrectionSummary {
                    original,
                    final_text: event.final_text.as_deref().unwrap_or(""),
                    section: event.section_title.as_deref(),
                    rejected: false,
                });
            }
            CorrectionAction::Reject => {
                let Some(original) = event.original_text.as_deref() else {
                    continue;
                };
                out.push(CorrectionSummary {
                    original,
                    final_text: "",
                    section: event.section_title.as_deref(),
                    rejected: true,
                });
            }
            // Not a preference signal — see the doc comment.
            CorrectionAction::Approve | CorrectionAction::Restore => {}
        }
    }
    out
}

/// Redact the fields that will reach the provider, when redaction is active.
///
/// The LLM miner sends correction TEXT — what the model wrote and what the human
/// kept — so the same opt-in PII/keyword policy that guards the transcript before
/// summarization (BACKLOG C6) must guard this. Section titles are structural
/// template labels rather than conversation content, so they are left as-is.
fn redact_events_for_miner(
    events: Vec<CorrectionEventRow>,
    cfg: &crate::redaction::RedactionConfig,
) -> Vec<CorrectionEventRow> {
    events
        .into_iter()
        .map(|mut event| {
            if let Some(text) = event.original_text.as_deref() {
                event.original_text = Some(crate::redaction::redact(text, cfg));
            }
            if let Some(text) = event.final_text.as_deref() {
                event.final_text = Some(crate::redaction::redact(text, cfg));
            }
            event
        })
        .collect()
}

/// The LLM mining pass (ADR-0030 §8, phase C2) — the opt-in miner that asks the
/// user's own BYOK provider to name subtler preferences than the deterministic
/// miner can see. Returns the number of rules created.
///
/// **Every safety property is enforced upstream of this glue and stays there.**
/// LLM candidates are born `Proposed` and never auto-activate (`initial_status`
/// special-cases `MinedLlm`, whatever the claimed support and config); they carry
/// `support_count = 0` and `evidence = []` because the parser never fabricates
/// provenance; and they dedup against `live_signatures`. This function only
/// gathers corrections, calls the model, and persists what the parser validates.
///
/// **It must never block, and never surface an error to the user.** A model call
/// takes seconds even on local Ollama, so the trigger runs it fire-and-forget
/// (see `api_approve_summary`), and every failure here is content-free-logged and
/// swallowed: the user asked for a summary, not a miner.
pub async fn mine_llm_and_persist<R: tauri::Runtime>(
    app: &AppHandle<R>,
    pool: &sqlx::SqlitePool,
    ctx: &crate::context::AuthContext,
) -> Result<usize, String> {
    let config =
        crate::database::repositories::setting::SettingsRepository::get_learning_config(pool, ctx)
            .await
            .map_err(|e| format!("Failed to read the learning configuration: {}", e))?;
    if !(config.enabled && config.llm_miner_enabled) {
        return Ok(0);
    }

    // One LLM mine per workspace at a time (see `LLM_MINE_IN_FLIGHT`). RAII, so a
    // failed or panicking pass frees the slot regardless.
    let _in_flight = match InFlightGuard::acquire(ctx.tenant_id.as_str()) {
        Some(guard) => guard,
        None => {
            log_info!("LLM mining already in flight for this workspace; skipping");
            return Ok(0);
        }
    };

    // Recent raw events, reduced to the model→human deltas worth reading.
    let events =
        CorrectionEventsRepository::list_recent(pool, ctx, MAX_CORRECTIONS_IN_PROMPT as i64)
            .await
            .map_err(|e| e.to_string())?;

    // Redaction (opt-in, C6) is applied BEFORE anything reaches the provider, and
    // fail-safe: an unreadable policy aborts the pass rather than risk sending
    // unredacted correction text to a (possibly cloud) model.
    let redaction =
        crate::database::repositories::setting::SettingsRepository::get_redaction_config(pool, ctx)
            .await
            .map_err(|e| format!("Failed to load redaction configuration: {}", e))?;
    let events = if redaction.is_active() {
        redact_events_for_miner(events, &redaction)
    } else {
        events
    };

    let summaries = summaries_from_events(&events);
    if summaries.len() < MIN_CORRECTIONS_FOR_LLM {
        // Not enough for a model to call anything a pattern; don't spend tokens.
        return Ok(0);
    }

    let (system_prompt, user_prompt) = build_llm_miner_prompt(&summaries);

    // Resolve the provider the SAME way summarization does (shared helper), from
    // the workspace's saved model config.
    let setting =
        match crate::database::repositories::setting::SettingsRepository::get_model_config(
            pool, ctx,
        )
        .await
        {
            Ok(Some(setting)) => setting,
            Ok(None) => {
                log_info!("No model configured; skipping LLM mining");
                return Ok(0);
            }
            Err(e) => return Err(format!("Failed to read the model configuration: {}", e)),
        };
    let assembled = crate::summary::service::SummaryService::assemble_provider(
        pool,
        ctx,
        &setting.provider,
        &setting.model,
    )
    .await?;

    // `app_data_dir` is only consulted for the BuiltInAI sidecar; other providers
    // ignore it, so a resolution failure need not abort the pass.
    let app_data_dir = app.path().app_data_dir().ok();
    let reply = assembled
        .call(app_data_dir.as_ref(), &system_prompt, &user_prompt)
        .await?;

    let known = LearnedRulesRepository::live_signatures(pool, ctx)
        .await
        .map_err(learned_rule_err)?;
    let candidates = parse_llm_rules(&reply, &known);

    let mut created = 0;
    for candidate in candidates {
        let new_rule = NewLearnedRule {
            scope: candidate.scope,
            kind: candidate.kind,
            rule_text: candidate.rule_text,
            // Born Proposed by `initial_status` — an LLM rule never auto-activates.
            origin: RuleOrigin::MinedLlm,
            // The parser never fabricates a count or citations; keep it honest.
            support_count: candidate.support_count,
            evidence: candidate.evidence,
            signature: Some(candidate.signature),
        };
        match LearnedRulesRepository::create(pool, ctx, &new_rule, &config).await {
            Ok(_) => created += 1,
            Err(e) => {
                // One bad candidate must not sink the pass.
                log_error!("Failed to persist an LLM-mined rule: {}", e);
            }
        }
    }
    log_info!("LLM mining pass created {} rule(s)", created);
    Ok(created)
}

/// What the learning system has to show for itself (ADR-0030 §9).
#[derive(Debug, Clone, Serialize)]
pub struct LearningStats {
    /// Corrections recorded in this workspace, ever.
    pub corrections_recorded: i64,
    /// Rules currently in force.
    pub rules_active: usize,
    /// Rules waiting for a human.
    pub rules_proposed: usize,
    /// How much of Mityu's writing the human has had to change, and whether that
    /// has moved.
    ///
    /// **This is a correlation, not a result**, and the copy that renders it must
    /// stay that way — see `learning::burden`. The number measures what the USER
    /// did; a user reviewing less carefully moves it the same direction a working
    /// rule does, and nothing here can tell those apart.
    pub burden: crate::learning::burden::BurdenTrend,
}

/// Corrections, rules and burden — the numbers behind "is this working?".
///
/// TS binding: `invoke("api_get_learning_stats") -> LearningStats`.
#[tauri::command]
pub async fn api_get_learning_stats(
    state: tauri::State<'_, AppState>,
) -> Result<LearningStats, String> {
    log_info!("api_get_learning_stats called");
    let pool = state.db_manager.pool();
    let ctx = crate::context::current();

    let corrections_recorded = CorrectionEventsRepository::count(pool, &ctx)
        .await
        .map_err(|e| e.to_string())?;

    let rules = LearnedRulesRepository::list_all(pool, &ctx)
        .await
        .map_err(learned_rule_err)?;
    let rules_active = rules
        .iter()
        .filter(|r| r.status == RuleStatus::Active)
        .count();
    let rules_proposed = rules
        .iter()
        .filter(|r| r.status == RuleStatus::Proposed)
        .count();

    // Bounded like the mining pass, and for the same reason.
    const STATS_WINDOW: i64 = 2_000;
    let events = CorrectionEventsRepository::list_recent(pool, &ctx, STATS_WINDOW)
        .await
        .map_err(|e| e.to_string())?;

    Ok(LearningStats {
        corrections_recorded,
        rules_active,
        rules_proposed,
        burden: crate::learning::burden::compute(&events),
    })
}

/// The workspace's learning policy.
///
/// TS binding: `invoke("api_get_learning_config") -> LearningConfig`.
#[tauri::command]
pub async fn api_get_learning_config(
    state: tauri::State<'_, AppState>,
) -> Result<LearningConfig, String> {
    log_info!("api_get_learning_config called");
    let pool = state.db_manager.pool();
    let ctx = crate::context::current();
    crate::database::repositories::setting::SettingsRepository::get_learning_config(pool, &ctx)
        .await
        .map_err(|e| {
            log_error!("Failed to get learning config: {}", e);
            format!("Failed to get the learning configuration: {}", e)
        })
}

/// Persists the workspace's learning policy.
///
/// TS binding: `invoke("api_set_learning_config", { config }) -> void`.
#[tauri::command]
pub async fn api_set_learning_config(
    state: tauri::State<'_, AppState>,
    config: LearningConfig,
) -> Result<(), String> {
    log_info!(
        "api_set_learning_config called: enabled={}, auto_activate={}, min_support={}, llm_miner={}",
        config.enabled,
        config.auto_activate,
        config.auto_activate_min_support,
        config.llm_miner_enabled
    );
    let pool = state.db_manager.pool();
    let ctx = crate::context::current();
    crate::database::repositories::setting::SettingsRepository::set_learning_config(
        pool, &ctx, &config,
    )
    .await
    .map_err(|e| {
        log_error!("Failed to save learning config: {}", e);
        format!("Failed to save the learning configuration: {}", e)
    })
}

#[cfg(test)]
mod llm_glue_tests {
    use super::*;
    use crate::database::repositories::correction_event::CorrectionSubject;

    /// One correction event with only the fields the miner reads set; the rest are
    /// the never-synced insert-time baseline.
    fn event(
        action: CorrectionAction,
        original: Option<&str>,
        final_text: Option<&str>,
        section: Option<&str>,
    ) -> CorrectionEventRow {
        CorrectionEventRow {
            id: "e1".to_string(),
            meeting_id: "m1".to_string(),
            subject_kind: CorrectionSubject::SummaryBlock,
            subject_id: "b1".to_string(),
            action,
            original_text: original.map(str::to_string),
            final_text: final_text.map(str::to_string),
            reason: None,
            block_type: None,
            section_title: section.map(str::to_string),
            template_id: None,
            model: None,
            created_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn only_edits_and_rejects_become_summaries() {
        let events = vec![
            event(
                CorrectionAction::Edit,
                Some("3 aksiyon çıktı"),
                Some("3 takip çıktı"),
                Some("Kararlar"),
            ),
            event(CorrectionAction::Reject, Some("dolgu cümlesi"), None, None),
            // Signal-free: the model got it right, then a reject was taken back.
            event(CorrectionAction::Approve, Some("x"), Some("x"), None),
            event(CorrectionAction::Restore, Some("y"), None, None),
        ];
        let summaries = summaries_from_events(&events);
        assert_eq!(summaries.len(), 2, "approve + restore carry no signal");

        // Edit → the model→human delta, not a reject.
        assert_eq!(summaries[0].original, "3 aksiyon çıktı");
        assert_eq!(summaries[0].final_text, "3 takip çıktı");
        assert_eq!(summaries[0].section, Some("Kararlar"));
        assert!(!summaries[0].rejected);

        // Reject → flagged, and nothing survived on the human side.
        assert_eq!(summaries[1].original, "dolgu cümlesi");
        assert_eq!(summaries[1].final_text, "");
        assert!(summaries[1].rejected);
    }

    #[test]
    fn an_event_without_original_text_is_skipped() {
        // Nothing the model wrote ⇒ no delta to show a model.
        let events = vec![event(CorrectionAction::Edit, None, Some("takip"), None)];
        assert!(summaries_from_events(&events).is_empty());
    }

    #[test]
    fn an_edit_with_no_final_text_shows_an_empty_human_side() {
        // Defensive: an edit should carry both sides, but a missing final must map
        // to "" rather than drop the correction or panic.
        let events = vec![event(CorrectionAction::Edit, Some("aksiyon"), None, None)];
        let summaries = summaries_from_events(&events);
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].final_text, "");
        assert!(!summaries[0].rejected);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::learning::rule::{RuleKind, RuleOrigin, RuleScope, RuleStatus};

    /// The wire is flat tokens, not serde's enum encoding — the frontend reads
    /// the same closed vocabulary the database stores.
    #[test]
    fn the_view_flattens_every_enum_to_its_db_token() {
        let view = LearnedRuleView::from(LearnedRule {
            id: "r1".to_string(),
            scope: RuleScope::Template("daily_standup".to_string()),
            kind: RuleKind::TermSubstitution,
            rule_text: "Say takip.".to_string(),
            status: RuleStatus::Proposed,
            origin: RuleOrigin::MinedDeterministic,
            support_count: 4,
        });
        assert_eq!(view.scope, "template:daily_standup");
        assert_eq!(view.kind, "term_substitution");
        assert_eq!(view.status, "proposed");
        assert_eq!(view.origin, "mined_deterministic");

        let json = serde_json::to_string(&view).unwrap();
        assert!(
            json.contains("\"scope\":\"template:daily_standup\""),
            "got: {json}"
        );
        assert!(json.contains("\"support_count\":4"), "got: {json}");
    }

    #[test]
    fn a_section_scope_round_trips_through_the_view() {
        let view = LearnedRuleView::from(LearnedRule {
            id: "r1".to_string(),
            scope: RuleScope::Section("Risks: open".to_string()),
            kind: RuleKind::Style,
            rule_text: "Keep it short.".to_string(),
            status: RuleStatus::Active,
            origin: RuleOrigin::UserAuthored,
            support_count: 0,
        });
        assert_eq!(view.scope, "section:Risks: open");
        assert_eq!(
            RuleScope::from_db(&view.scope).unwrap(),
            RuleScope::Section("Risks: open".to_string()),
        );
    }
}
