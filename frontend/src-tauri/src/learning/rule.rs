//! The domain shape of a learned rule (ADR-0024 §4).
//!
//! A rule is what Mityu learned from a user's corrections, expressed in **plain
//! language** and stored as data — never as model weights. That choice is legal
//! before it is technical: KVKK/GDPR erasure is not satisfiable against a
//! fine-tune, whereas an unwanted rule is one `DELETE` (ADR-0024 §10). It is also
//! what lets the user read, rewrite and delete every rule, which is both the
//! product surface (§9) and the EU-AI-Act defence — an opaque "our AI learns you"
//! is precisely what cannot be defended.
//!
//! This module is the domain only: types, the DB token vocabulary, and the scope
//! filter. Persistence lives in `database/repositories/learned_rule.rs`; the
//! prompt rendering lives in `summary::structured`, next to the prompts it
//! shapes.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A stored token was outside the ADR-0024 §4 vocabulary (corrupt or hand-edited
/// row). Tokens are a closed vocabulary, not user content, so echoing one leaks
/// nothing.
#[derive(Debug, Error, PartialEq, Eq)]
#[error("unknown learned-rule token in database: {token}")]
pub struct InvalidRuleToken {
    /// The offending stored token.
    pub token: String,
}

/// Where a rule applies. Stored in `learned_rules.scope` as
/// `global | template:<id> | section:<title>`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub enum RuleScope {
    /// Applies to every summary.
    Global,
    /// Applies only when this template is in play.
    Template(String),
    /// Applies to one named section. Still injected for every template — the
    /// prompt names the section instead, because a section title is not owned by
    /// a template and the model needs to be told where the rule bites.
    Section(String),
}

impl RuleScope {
    /// Stored token.
    pub fn to_db(&self) -> String {
        match self {
            Self::Global => "global".to_string(),
            Self::Template(id) => format!("template:{id}"),
            Self::Section(title) => format!("section:{title}"),
        }
    }

    /// Parse a stored token. `split_once` rather than `split`, so a section title
    /// containing a colon survives the round trip verbatim.
    pub fn from_db(token: &str) -> Result<Self, InvalidRuleToken> {
        if token == "global" {
            return Ok(Self::Global);
        }
        match token.split_once(':') {
            Some(("template", id)) if !id.is_empty() => Ok(Self::Template(id.to_string())),
            Some(("section", title)) if !title.is_empty() => Ok(Self::Section(title.to_string())),
            _ => Err(InvalidRuleToken {
                token: token.to_string(),
            }),
        }
    }
}

/// What kind of preference a rule expresses. Advisory: it drives grouping and
/// the miner's own bookkeeping, never the prompt's meaning — `rule_text` alone
/// carries that.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleKind {
    /// "Say X, not Y."
    TermSubstitution,
    /// Tone, length, formatting.
    Style,
    /// Something about a whole section (drop it, always include it, …).
    SectionPreference,
    /// Anything the above do not describe — including everything a user writes
    /// by hand.
    Freeform,
}

impl RuleKind {
    /// Stored token.
    pub fn to_db(self) -> &'static str {
        match self {
            Self::TermSubstitution => "term_substitution",
            Self::Style => "style",
            Self::SectionPreference => "section_preference",
            Self::Freeform => "freeform",
        }
    }

    /// Parse a stored token.
    pub fn from_db(token: &str) -> Result<Self, InvalidRuleToken> {
        match token {
            "term_substitution" => Ok(Self::TermSubstitution),
            "style" => Ok(Self::Style),
            "section_preference" => Ok(Self::SectionPreference),
            "freeform" => Ok(Self::Freeform),
            other => Err(InvalidRuleToken {
                token: other.to_string(),
            }),
        }
    }
}

/// A rule's lifecycle. Only [`Self::Active`] rules ever reach a prompt.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleStatus {
    /// Mined and waiting for the human. The default a mined rule is born with.
    Proposed,
    /// In force — injected into every applicable generation.
    Active,
    /// The human said no. Kept, not deleted, so the miner can tell "not yet
    /// proposed" from "proposed and refused" and stop re-proposing it.
    Dismissed,
}

impl RuleStatus {
    /// Stored token.
    pub fn to_db(self) -> &'static str {
        match self {
            Self::Proposed => "proposed",
            Self::Active => "active",
            Self::Dismissed => "dismissed",
        }
    }

    /// Parse a stored token.
    pub fn from_db(token: &str) -> Result<Self, InvalidRuleToken> {
        match token {
            "proposed" => Ok(Self::Proposed),
            "active" => Ok(Self::Active),
            "dismissed" => Ok(Self::Dismissed),
            other => Err(InvalidRuleToken {
                token: other.to_string(),
            }),
        }
    }
}

/// Where a rule came from. Load-bearing, not bookkeeping: it decides whether a
/// rule needs approval at all ([`Self::UserAuthored`] rules are born active — the
/// user writing the rule IS the approval) and it is what the rules screen shows
/// the user when it explains itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleOrigin {
    /// A local, deterministic miner spotted a repeated correction.
    MinedDeterministic,
    /// The BYOK provider was asked to spot a pattern the miners cannot.
    MinedLlm,
    /// The user wrote it themselves.
    UserAuthored,
}

impl RuleOrigin {
    /// Stored token.
    pub fn to_db(self) -> &'static str {
        match self {
            Self::MinedDeterministic => "mined_deterministic",
            Self::MinedLlm => "mined_llm",
            Self::UserAuthored => "user_authored",
        }
    }

    /// Parse a stored token.
    pub fn from_db(token: &str) -> Result<Self, InvalidRuleToken> {
        match token {
            "mined_deterministic" => Ok(Self::MinedDeterministic),
            "mined_llm" => Ok(Self::MinedLlm),
            "user_authored" => Ok(Self::UserAuthored),
            other => Err(InvalidRuleToken {
                token: other.to_string(),
            }),
        }
    }
}

/// One rule, as the rest of the app sees it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct LearnedRule {
    /// Row id (uuid v4). Also the injection ORDER key — see
    /// [`applicable_rules`].
    pub id: String,
    /// Where it applies.
    pub scope: RuleScope,
    /// What kind of preference it expresses (advisory).
    pub kind: RuleKind,
    /// The rule itself, in plain language, injected into the prompt verbatim.
    pub rule_text: String,
    /// Lifecycle.
    pub status: RuleStatus,
    /// Provenance.
    pub origin: RuleOrigin,
    /// How many correction events back it.
    pub support_count: i64,
}

/// The rule status machine (ADR-0024 §4), mirroring the shape of
/// `block_transition_allowed`: `proposed → active | dismissed`,
/// `active → dismissed`, `dismissed → active`. Everything else — including
/// self-transitions — is illegal.
///
/// Note what is NOT here: nothing returns to `Proposed`. "Proposed" means the
/// miner is asking; once the human has answered, the answer stands until they
/// change it themselves. Re-proposing something already decided is exactly the
/// nagging this status machine exists to prevent.
pub fn rule_transition_allowed(from: RuleStatus, to: RuleStatus) -> bool {
    use RuleStatus::{Active, Dismissed, Proposed};
    matches!(
        (from, to),
        (Proposed, Active) | (Proposed, Dismissed) | (Active, Dismissed) | (Dismissed, Active)
    )
}

/// The status a freshly mined or authored rule is born with.
///
/// [`RuleOrigin::UserAuthored`] is always [`RuleStatus::Active`]: a user who
/// writes a rule has approved it by writing it, and parking it in a queue for
/// them to approve their own instruction would be absurd.
///
/// A [`RuleOrigin::MinedDeterministic`] rule is born [`RuleStatus::Proposed`]
/// unless the workspace has auto-activation on AND the rule clears the support
/// threshold — see
/// [`LearningConfig::may_auto_activate`](crate::learning::config::LearningConfig::may_auto_activate)
/// for why that is bounded rather than reckless.
///
/// A [`RuleOrigin::MinedLlm`] rule is ALWAYS born `Proposed`, and never
/// auto-activates. Its `support_count` is whatever the model *claimed*, not a
/// verified count of corrections the user actually made — the deterministic
/// miner earns its count by observing repeats, the LLM just asserts one. Letting
/// an asserted number clear the auto-activation threshold would let a model
/// talk its own suggestion into force without a human ever seeing it, which is
/// exactly the bound §7 leans on. So the LLM only ever *suggests*; a person
/// always says yes.
pub fn initial_status(
    origin: RuleOrigin,
    support_count: i64,
    config: &crate::learning::config::LearningConfig,
) -> RuleStatus {
    match origin {
        RuleOrigin::UserAuthored => RuleStatus::Active,
        RuleOrigin::MinedLlm => RuleStatus::Proposed,
        RuleOrigin::MinedDeterministic => {
            if config.may_auto_activate(support_count) {
                RuleStatus::Active
            } else {
                RuleStatus::Proposed
            }
        }
    }
}

/// One rule AS IT STOOD when it shaped a summary — the ADR-0024 §5 snapshot,
/// stored in `summaries.applied_rules`.
///
/// Two deliberate differences from [`LearnedRule`], both because this is an
/// archival record rather than live state:
///
/// 1. **The text is copied, not referenced.** Rules are editable and deletable,
///    so an id alone cannot answer "why does this six-month-old summary read like
///    this?" — the rule it names may since have been rewritten, or be gone. That
///    question is the EU-AI-Act Art.50 requirement and the product's evidence
///    claim, and it is the precondition that makes auto-activation (§7)
///    defensible at all.
/// 2. **`scope` is the raw token, not [`RuleScope`].** A snapshot must stay
///    readable even if the scope vocabulary later grows or changes; parsing an
///    old record against today's enum would turn an archive into a liability.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppliedRule {
    /// The rule's id at generation time. May no longer resolve — that is
    /// expected, and why the text travels with it.
    pub rule_id: String,
    /// The rule's text at generation time, verbatim.
    pub rule_text: String,
    /// The scope token at generation time (`global`, `template:…`, `section:…`).
    pub scope: String,
}

impl AppliedRule {
    /// Freeze a live rule into its snapshot form.
    pub fn from_rule(rule: &LearnedRule) -> Self {
        Self {
            rule_id: rule.id.clone(),
            rule_text: rule.rule_text.clone(),
            scope: rule.scope.to_db(),
        }
    }
}

/// The rules that apply to a generation using `template_id`, in a STABLE order.
///
/// Two properties, both deliberate:
///
/// 1. **Only `Active`.** A proposed rule has not been agreed to, and a dismissed
///    one was refused; neither may shape a summary.
/// 2. **Ordered by id, always.** The order rules appear in changes the prompt,
///    and the prompt has to be reproducible: `summaries.applied_rules` snapshots
///    what shaped a given summary (ADR-0024 §5) and that snapshot is worthless if
///    the same rule set can render two different ways. Row order out of SQLite is
///    not a contract; this is.
pub fn applicable_rules<'a>(
    rules: &'a [LearnedRule],
    template_id: Option<&str>,
) -> Vec<&'a LearnedRule> {
    let mut applicable: Vec<&LearnedRule> = rules
        .iter()
        .filter(|rule| rule.status == RuleStatus::Active)
        .filter(|rule| match &rule.scope {
            RuleScope::Global | RuleScope::Section(_) => true,
            RuleScope::Template(wanted) => template_id == Some(wanted.as_str()),
        })
        .collect();
    applicable.sort_by(|a, b| a.id.cmp(&b.id));
    applicable
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(id: &str, scope: RuleScope, status: RuleStatus) -> LearnedRule {
        LearnedRule {
            id: id.to_string(),
            scope,
            kind: RuleKind::Freeform,
            rule_text: format!("rule {id}"),
            status,
            origin: RuleOrigin::UserAuthored,
            support_count: 0,
        }
    }

    #[test]
    fn birth_status_by_origin() {
        use crate::learning::config::LearningConfig;
        let auto = LearningConfig::default(); // auto-activate on, threshold 3
        let manual = LearningConfig {
            auto_activate: false,
            ..LearningConfig::default()
        };

        // A user's own rule is always in force.
        assert_eq!(
            initial_status(RuleOrigin::UserAuthored, 0, &manual),
            RuleStatus::Active,
        );

        // Deterministic: policy + threshold decide.
        assert_eq!(
            initial_status(RuleOrigin::MinedDeterministic, 3, &auto),
            RuleStatus::Active,
        );
        assert_eq!(
            initial_status(RuleOrigin::MinedDeterministic, 2, &auto),
            RuleStatus::Proposed,
        );
        assert_eq!(
            initial_status(RuleOrigin::MinedDeterministic, 9, &manual),
            RuleStatus::Proposed,
        );

        // LLM: ALWAYS proposed — even with a huge claimed support and
        // auto-activation on. The model's asserted count is not a verified one,
        // so it can never talk itself into force (ADR-0024 §8, C2 safety).
        assert_eq!(
            initial_status(RuleOrigin::MinedLlm, 999, &auto),
            RuleStatus::Proposed,
        );
    }

    #[test]
    fn scope_tokens_round_trip() {
        for scope in [
            RuleScope::Global,
            RuleScope::Template("daily_standup".to_string()),
            RuleScope::Section("Risks".to_string()),
        ] {
            assert_eq!(RuleScope::from_db(&scope.to_db()).unwrap(), scope);
        }
    }

    /// A section title is free text a template author chose; a colon in it must
    /// not truncate the title or turn into a different scope.
    #[test]
    fn a_section_title_containing_a_colon_survives() {
        let scope = RuleScope::Section("Risks: open".to_string());
        assert_eq!(scope.to_db(), "section:Risks: open");
        assert_eq!(RuleScope::from_db("section:Risks: open").unwrap(), scope);
    }

    #[test]
    fn malformed_scope_tokens_are_typed_errors() {
        for token in ["", "template", "template:", "section:", "nonsense", ":x"] {
            assert!(
                RuleScope::from_db(token).is_err(),
                "{token:?} must not parse"
            );
        }
    }

    #[test]
    fn the_other_token_vocabularies_round_trip() {
        for kind in [
            RuleKind::TermSubstitution,
            RuleKind::Style,
            RuleKind::SectionPreference,
            RuleKind::Freeform,
        ] {
            assert_eq!(RuleKind::from_db(kind.to_db()).unwrap(), kind);
        }
        for status in [
            RuleStatus::Proposed,
            RuleStatus::Active,
            RuleStatus::Dismissed,
        ] {
            assert_eq!(RuleStatus::from_db(status.to_db()).unwrap(), status);
        }
        for origin in [
            RuleOrigin::MinedDeterministic,
            RuleOrigin::MinedLlm,
            RuleOrigin::UserAuthored,
        ] {
            assert_eq!(RuleOrigin::from_db(origin.to_db()).unwrap(), origin);
        }
    }

    #[test]
    fn only_active_rules_are_applicable() {
        let rules = vec![
            rule("a", RuleScope::Global, RuleStatus::Active),
            rule("b", RuleScope::Global, RuleStatus::Proposed),
            rule("c", RuleScope::Global, RuleStatus::Dismissed),
        ];
        let applicable = applicable_rules(&rules, None);
        assert_eq!(applicable.len(), 1);
        assert_eq!(applicable[0].id, "a");
    }

    #[test]
    fn template_scoped_rules_only_apply_to_their_template() {
        let rules = vec![
            rule(
                "a",
                RuleScope::Template("daily_standup".to_string()),
                RuleStatus::Active,
            ),
            rule(
                "b",
                RuleScope::Template("standard_meeting".to_string()),
                RuleStatus::Active,
            ),
            rule("c", RuleScope::Global, RuleStatus::Active),
        ];

        let ids: Vec<&str> = applicable_rules(&rules, Some("daily_standup"))
            .iter()
            .map(|r| r.id.as_str())
            .collect();
        assert_eq!(ids, ["a", "c"]);

        // No template in play → template-scoped rules cannot claim to apply.
        let ids: Vec<&str> = applicable_rules(&rules, None)
            .iter()
            .map(|r| r.id.as_str())
            .collect();
        assert_eq!(ids, ["c"]);
    }

    /// Section-scoped rules apply regardless of template: a section title is not
    /// owned by one template, and the prompt names the section anyway.
    #[test]
    fn section_scoped_rules_apply_under_any_template() {
        let rules = vec![rule(
            "a",
            RuleScope::Section("Risks".to_string()),
            RuleStatus::Active,
        )];
        assert_eq!(applicable_rules(&rules, Some("anything")).len(), 1);
        assert_eq!(applicable_rules(&rules, None).len(), 1);
    }

    /// The order IS the contract — `summaries.applied_rules` snapshots what
    /// shaped a summary, and that snapshot is worthless if the same set of rules
    /// can render the prompt two different ways.
    #[test]
    fn ordering_is_by_id_and_independent_of_input_order() {
        let forwards = vec![
            rule("a1", RuleScope::Global, RuleStatus::Active),
            rule("b2", RuleScope::Global, RuleStatus::Active),
            rule("c3", RuleScope::Global, RuleStatus::Active),
        ];
        let mut backwards = forwards.clone();
        backwards.reverse();

        let ids = |rules: &[LearnedRule]| -> Vec<String> {
            applicable_rules(rules, None)
                .iter()
                .map(|r| r.id.clone())
                .collect()
        };
        assert_eq!(ids(&forwards), ["a1", "b2", "c3"]);
        assert_eq!(ids(&backwards), ids(&forwards));
    }
}
