//! Tenant-scoped repository for `learned_rules` (ADR-0024 §4).
//!
//! The supplier the injection seam has been waiting for:
//! [`LearnedRulesRepository::list_active`] is what
//! `summary::service::run_structured_generation` reads before building a prompt.
//!
//! House rules (B2 / ADR-0010), all upheld: every method takes [`AuthContext`]
//! and scopes EVERY statement with `workspace_id = ctx.tenant_id`; every write
//! bumps `rev` and stamps `updated_by` + `updated_at`; cross-workspace access
//! degrades to `Ok(false)` without touching the foreign row. Reads exclude
//! soft-deleted rows.
//!
//! ## Dismiss and delete are different actions, on purpose
//!
//! - **Dismiss** (`status = 'dismissed'`) keeps the row. It is the human saying
//!   "no" ON THE RECORD, which is what lets the miner tell "never proposed" from
//!   "proposed and refused" and stop offering it again.
//! - **Delete** ([`LearnedRulesRepository::soft_delete`]) removes it from the
//!   user's list. It carries no memory of refusal, so **a deleted rule can be
//!   re-learned** if the behaviour that produced it repeats.
//!
//! That asymmetry is deliberate, not an oversight: if a user deletes a rule and
//! then makes the same correction three more times, being offered it again is
//! the system working. "Never suggest this to me again" is what Dismiss is for,
//! and the rules screen (§9) has to say so in those words.
//!
//! Unlike `correction_events`, rules hold NO transcript text — a rule is an
//! abstraction the human approved ("call it 'takip'"). That is precisely why
//! deleting a meeting does not take its rules with it (ADR-0024 §10), and why
//! rule text is safe to keep after the corrections behind it are erased.

use crate::context::AuthContext;
use crate::learning::config::LearningConfig;
use crate::learning::rule::{
    initial_status, rule_transition_allowed, InvalidRuleToken, LearnedRule, RuleKind, RuleOrigin,
    RuleScope, RuleStatus,
};
use chrono::Utc;
use sqlx::{Row, SqlitePool};
use thiserror::Error;
use tracing::{info, warn};
use uuid::Uuid;

/// Typed error for the learned-rules repository (docs/CONVENTIONS.md).
///
/// Content-free by construction: variants carry ids and closed-vocabulary tokens
/// only. Rule TEXT is user-authored or mined content and never appears in an
/// error or a log line (CLAUDE.md §0.6).
#[derive(Debug, Error)]
pub enum LearnedRuleError {
    /// Underlying database failure.
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    /// A stored token was outside the §4 vocabulary (corrupt or hand-edited row).
    #[error(transparent)]
    InvalidToken(#[from] InvalidRuleToken),
    /// The rule text was blank. A rule that says nothing would still be injected
    /// into every prompt as a numbered instruction.
    #[error("rule text must not be blank")]
    BlankText,
}

/// A rule about to be created.
#[derive(Debug, Clone)]
pub struct NewLearnedRule {
    /// Where it applies.
    pub scope: RuleScope,
    /// What kind of preference it expresses.
    pub kind: RuleKind,
    /// The rule in plain language.
    pub rule_text: String,
    /// Provenance — decides whether it needs approval at all.
    pub origin: RuleOrigin,
    /// How many correction events back it (0 for user-authored).
    pub support_count: i64,
    /// The correction events that produced it. May dangle later, once their
    /// meeting is deleted (ADR-0024 §10).
    pub evidence: Vec<String>,
    /// The miner's stable identity for this rule, or `None` for a user-authored
    /// one — nothing mined it, so nothing can re-propose it. See
    /// [`LearnedRulesRepository::live_signatures`].
    pub signature: Option<String>,
}

pub struct LearnedRulesRepository;

/// Columns every read selects, in `map_row` order.
const SELECT_COLS: &str =
    "id, scope, kind, rule_text, status, origin, support_count, evidence, created_at, activated_at";

fn map_row(row: &sqlx::sqlite::SqliteRow) -> Result<LearnedRule, LearnedRuleError> {
    Ok(LearnedRule {
        id: row.get("id"),
        scope: RuleScope::from_db(&row.get::<String, _>("scope"))?,
        kind: RuleKind::from_db(&row.get::<String, _>("kind"))?,
        rule_text: row.get("rule_text"),
        status: RuleStatus::from_db(&row.get::<String, _>("status"))?,
        origin: RuleOrigin::from_db(&row.get::<String, _>("origin"))?,
        support_count: row.get("support_count"),
    })
}

impl LearnedRulesRepository {
    /// Creates a rule, deciding its birth status via
    /// [`initial_status`] — user-authored rules are born `Active`, mined ones
    /// `Proposed` unless the workspace's policy auto-activates them.
    ///
    /// The status is decided HERE rather than taken from the caller so that no
    /// future caller can mint an `Active` rule by simply asking for one:
    /// auto-activation stays a property of the workspace's policy, not of
    /// whoever is inserting.
    pub async fn create(
        pool: &SqlitePool,
        ctx: &AuthContext,
        new_rule: &NewLearnedRule,
        config: &LearningConfig,
    ) -> Result<String, LearnedRuleError> {
        let text = new_rule.rule_text.trim();
        if text.is_empty() {
            return Err(LearnedRuleError::BlankText);
        }

        let status = initial_status(new_rule.origin, new_rule.support_count, config);
        let id = Uuid::new_v4().to_string();
        let now = Utc::now();
        let evidence_json = serde_json::to_string(&new_rule.evidence)
            .map_err(|e| LearnedRuleError::Database(sqlx::Error::Protocol(e.to_string())))?;

        sqlx::query(
            "INSERT INTO learned_rules \
             (id, workspace_id, scope, kind, rule_text, status, origin, support_count, evidence, \
              signature, created_at, activated_at, activated_by, updated_at, updated_by, rev) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 1)",
        )
        .bind(&id)
        .bind(ctx.tenant_id.as_str())
        .bind(new_rule.scope.to_db())
        .bind(new_rule.kind.to_db())
        .bind(text)
        .bind(status.to_db())
        .bind(new_rule.origin.to_db())
        .bind(new_rule.support_count)
        .bind(&evidence_json)
        .bind(new_rule.signature.as_deref())
        .bind(now)
        // Born active ⇒ it is in force from now, so stamp when.
        .bind(if status == RuleStatus::Active {
            Some(now)
        } else {
            None
        })
        .bind(if status == RuleStatus::Active {
            Some(ctx.user_id.as_str())
        } else {
            None
        })
        .bind(now)
        .bind(ctx.user_id.as_str())
        .execute(pool)
        .await?;

        // Tokens and ids only — never `rule_text` (§0.6).
        info!(
            rule_id = %id,
            origin = new_rule.origin.to_db(),
            status = status.to_db(),
            support = new_rule.support_count,
            "learned rule created"
        );
        Ok(id)
    }

    /// The workspace's ACTIVE rules — what `run_structured_generation` injects.
    ///
    /// Returns EMPTY when learning is switched off, rather than making every
    /// caller remember to check: a caller who forgets would silently keep
    /// injecting rules into a workspace that turned the feature off, and that
    /// bug would be invisible in the output.
    pub async fn list_active(
        pool: &SqlitePool,
        ctx: &AuthContext,
        config: &LearningConfig,
    ) -> Result<Vec<LearnedRule>, LearnedRuleError> {
        if !config.enabled {
            return Ok(Vec::new());
        }
        let sql = format!(
            "SELECT {SELECT_COLS} FROM learned_rules \
             WHERE workspace_id = ? AND status = 'active' AND deleted_at IS NULL \
             ORDER BY id ASC"
        );
        let rows = sqlx::query(&sql)
            .bind(ctx.tenant_id.as_str())
            .fetch_all(pool)
            .await?;
        rows.iter().map(map_row).collect()
    }

    /// The miner's signatures already on record — its "do not ask again" set.
    ///
    /// Deliberately spans EVERY status including `dismissed`: that is what a
    /// dismissed row is FOR (§4). The human answered; asking again is nagging.
    ///
    /// Equally deliberately, it excludes soft-DELETED rows. Delete carries no
    /// record of refusal, so a deleted rule's signature is free again and the
    /// behaviour can be re-learned — which is exactly the difference between
    /// "remove this from my list" and "never suggest it again", and the reason
    /// the screen has to spell that out.
    pub async fn live_signatures(
        pool: &SqlitePool,
        ctx: &AuthContext,
    ) -> Result<Vec<String>, LearnedRuleError> {
        Ok(sqlx::query_scalar(
            "SELECT signature FROM learned_rules \
             WHERE workspace_id = ? AND signature IS NOT NULL AND deleted_at IS NULL",
        )
        .bind(ctx.tenant_id.as_str())
        .fetch_all(pool)
        .await?)
    }

    /// Every live rule in the workspace, whatever its status — the rules screen
    /// (§9). Ordered proposed-first so a pending decision is what the user sees,
    /// then by id for stability.
    pub async fn list_all(
        pool: &SqlitePool,
        ctx: &AuthContext,
    ) -> Result<Vec<LearnedRule>, LearnedRuleError> {
        let sql = format!(
            "SELECT {SELECT_COLS} FROM learned_rules \
             WHERE workspace_id = ? AND deleted_at IS NULL \
             ORDER BY CASE status WHEN 'proposed' THEN 0 WHEN 'active' THEN 1 ELSE 2 END, id ASC"
        );
        let rows = sqlx::query(&sql)
            .bind(ctx.tenant_id.as_str())
            .fetch_all(pool)
            .await?;
        rows.iter().map(map_row).collect()
    }

    /// One rule's `evidence` id list — the correction events behind it.
    ///
    /// `Ok(None)` when the rule is not visible in this workspace. An empty vec is
    /// a real answer (a user-authored rule has no evidence); the ids themselves
    /// MAY no longer resolve, and callers must degrade to "kanıt silindi" rather
    /// than error (ADR-0024 §10).
    pub async fn evidence_ids(
        pool: &SqlitePool,
        ctx: &AuthContext,
        rule_id: &str,
    ) -> Result<Option<Vec<String>>, LearnedRuleError> {
        let json: Option<Option<String>> = sqlx::query_scalar(
            "SELECT evidence FROM learned_rules \
             WHERE id = ? AND workspace_id = ? AND deleted_at IS NULL",
        )
        .bind(rule_id)
        .bind(ctx.tenant_id.as_str())
        .fetch_optional(pool)
        .await?;

        let Some(json) = json else {
            return Ok(None);
        };
        let Some(json) = json else {
            return Ok(Some(Vec::new()));
        };
        Ok(Some(serde_json::from_str(&json).unwrap_or_default()))
    }

    /// Applies the [`rule_transition_allowed`] status machine.
    ///
    /// `Ok(false)` (with a content-free reason code) when the rule is not visible
    /// in this workspace or the transition is illegal. Activating stamps
    /// `activated_at`/`activated_by`; leaving `active` clears them, because a
    /// stale activation stamp would misdate the rule if it is ever switched back
    /// on.
    pub async fn set_status(
        pool: &SqlitePool,
        ctx: &AuthContext,
        rule_id: &str,
        new: RuleStatus,
    ) -> Result<bool, LearnedRuleError> {
        let current: Option<String> = sqlx::query_scalar(
            "SELECT status FROM learned_rules \
             WHERE id = ? AND workspace_id = ? AND deleted_at IS NULL",
        )
        .bind(rule_id)
        .bind(ctx.tenant_id.as_str())
        .fetch_optional(pool)
        .await?;

        let Some(current) = current else {
            info!(
                rule_id = %rule_id,
                reason_code = "rule_not_found",
                "set_status refused"
            );
            return Ok(false);
        };
        let current = RuleStatus::from_db(&current)?;

        if !rule_transition_allowed(current, new) {
            info!(
                rule_id = %rule_id,
                from = current.to_db(),
                to = new.to_db(),
                reason_code = "illegal_transition",
                "set_status refused"
            );
            return Ok(false);
        }

        let now = Utc::now();
        let activating = new == RuleStatus::Active;
        let result = sqlx::query(
            "UPDATE learned_rules SET status = ?, activated_at = ?, activated_by = ?, \
             updated_at = ?, updated_by = ?, rev = rev + 1 \
             WHERE id = ? AND workspace_id = ? AND deleted_at IS NULL",
        )
        .bind(new.to_db())
        .bind(if activating { Some(now) } else { None })
        .bind(if activating {
            Some(ctx.user_id.as_str())
        } else {
            None
        })
        .bind(now)
        .bind(ctx.user_id.as_str())
        .bind(rule_id)
        .bind(ctx.tenant_id.as_str())
        .execute(pool)
        .await?;

        info!(
            rule_id = %rule_id,
            from = current.to_db(),
            to = new.to_db(),
            "learned rule status changed"
        );
        Ok(result.rows_affected() > 0)
    }

    /// The user rewrites a rule in their own words (§9).
    ///
    /// The rule KEEPS its origin and evidence: it was still mined from those
    /// corrections, the human has only said it better. Rewriting does not change
    /// status either — editing an active rule leaves it active, because the user
    /// is refining something they already agreed to, not re-proposing it.
    /// `Ok(false)` when the rule is not visible in this workspace.
    pub async fn edit_text(
        pool: &SqlitePool,
        ctx: &AuthContext,
        rule_id: &str,
        rule_text: &str,
    ) -> Result<bool, LearnedRuleError> {
        let text = rule_text.trim();
        if text.is_empty() {
            return Err(LearnedRuleError::BlankText);
        }
        let result = sqlx::query(
            "UPDATE learned_rules SET rule_text = ?, updated_at = ?, updated_by = ?, \
             rev = rev + 1 \
             WHERE id = ? AND workspace_id = ? AND deleted_at IS NULL",
        )
        .bind(text)
        .bind(Utc::now())
        .bind(ctx.user_id.as_str())
        .bind(rule_id)
        .bind(ctx.tenant_id.as_str())
        .execute(pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Removes a rule from the user's list (soft — sets `deleted_at`).
    ///
    /// This is NOT "never suggest this again" — that is `Dismissed`. A deleted
    /// rule leaves no record of refusal, so the miner may propose it again if the
    /// behaviour repeats. See the module docs.
    pub async fn soft_delete(
        pool: &SqlitePool,
        ctx: &AuthContext,
        rule_id: &str,
    ) -> Result<bool, LearnedRuleError> {
        let now = Utc::now();
        let result = sqlx::query(
            "UPDATE learned_rules SET deleted_at = ?, updated_at = ?, updated_by = ?, \
             rev = rev + 1 \
             WHERE id = ? AND workspace_id = ? AND deleted_at IS NULL",
        )
        .bind(now)
        .bind(now)
        .bind(ctx.user_id.as_str())
        .bind(rule_id)
        .bind(ctx.tenant_id.as_str())
        .execute(pool)
        .await?;
        if result.rows_affected() == 0 {
            warn!(
                rule_id = %rule_id,
                reason_code = "rule_not_found",
                "soft_delete found nothing to delete"
            );
        }
        Ok(result.rows_affected() > 0)
    }
}
