//! LLM-assisted rule mining (ADR-0030 §8, phase C2) — the opt-in miner.
//!
//! The deterministic miner (`learning::miner`) only sees patterns it was taught
//! to look for: term substitutions and section rejections. This one asks the
//! user's own BYOK provider to read the correction log and name subtler habits
//! ("you always add a deadline the model omitted", "you drop hedging language").
//!
//! **Off by default, and not for privacy** — the transcript already goes to that
//! provider to be summarized, so this opens no new data path — **but because it
//! spends the user's own API budget** (`LearningConfig::llm_miner_enabled`).
//! Enabling a paid background process on someone's behalf is not a default.
//!
//! ## What is here vs. what is not
//!
//! This module is PURE and harness-testable: [`build_llm_miner_prompt`] turns a
//! slice of corrections into a prompt, and [`parse_llm_rules`] turns the model's
//! JSON reply into validated [`RuleCandidate`]s. The provider CALL that sits
//! between them lives in `learning::commands` beside the deterministic pass, so
//! this module never touches the network and can be verified without one.
//!
//! ## Two safety decisions, because an LLM is a less trustworthy witness
//!
//! 1. **LLM rules never auto-activate.** Enforced upstream in
//!    [`initial_status`](crate::learning::rule::initial_status): a `MinedLlm`
//!    rule is always born `Proposed`. The model's claimed support is an
//!    assertion, not a count of corrections it watched the user make, so it must
//!    never clear an auto-activation threshold. The LLM suggests; a human agrees.
//! 2. **LLM rules carry no fabricated support or evidence.** `support_count = 0`
//!    and `evidence = []`. The model cannot reliably map a pattern back to the
//!    specific correction-event ids that show it, so claiming a count or citing
//!    events would be inventing provenance. Zero is honest, and the rules screen
//!    shows nothing where a real count would go.
//!
//! Term-substitution candidates are given the SAME signature the deterministic
//! miner would (`term_substitution|<scope>|<from>=>=<to>`, normalized), so if the
//! LLM proposes a rename the deterministic miner already found — or that the user
//! already dismissed — it dedups against the record instead of nagging.

use crate::learning::miner::RuleCandidate;
use crate::learning::rule::{RuleKind, RuleScope};
use crate::text::normalize;
use serde::Deserialize;

/// How many recent corrections to show the model. Bounded to keep the prompt
/// (and the user's token bill) in hand; the deterministic miner has already
/// mopped up the high-frequency patterns, so this one is looking in the tail.
pub const MAX_CORRECTIONS_IN_PROMPT: usize = 60;

/// The model's reply shape. Every field optional so a near-miss reply yields
/// whatever is salvageable rather than failing wholesale (the same leniency the
/// structured-summary parser uses).
#[derive(Debug, Deserialize)]
struct LlmReply {
    #[serde(default)]
    rules: Vec<LlmRule>,
}

#[derive(Debug, Deserialize)]
struct LlmRule {
    kind: Option<String>,
    scope: Option<String>,
    /// For a term substitution: the word the model wrote.
    from: Option<String>,
    /// For a term substitution: the word the user prefers.
    to: Option<String>,
    /// The plain-language rule for everything else (and a fallback wording for
    /// term substitutions).
    rule: Option<String>,
}

/// One correction, reduced to what the prompt shows — no ids, no meeting refs,
/// just the model→human delta and where it happened.
#[derive(Debug, Clone)]
pub struct CorrectionSummary<'a> {
    pub original: &'a str,
    pub final_text: &'a str,
    pub section: Option<&'a str>,
    /// `true` for a reject (nothing survived), so the prompt can say so.
    pub rejected: bool,
}

/// Build the miner prompt. Deterministic given its input (no timestamps, no
/// randomness) so it is testable and cache-stable.
pub fn build_llm_miner_prompt(corrections: &[CorrectionSummary<'_>]) -> (String, String) {
    let system = r#"You study how a user edits AI-written meeting summaries and name the PREFERENCES behind their edits, so a future summary can match their style without them having to fix it again.

Return ONLY a JSON object of this shape — no prose, no markdown:
{"rules": [{"kind": "...", "scope": "...", "from": "...", "to": "...", "rule": "..."}]}

Rules for your rules:
- Propose a rule ONLY for a pattern you can see across SEVERAL corrections. One edit is not a preference.
- `kind` is one of: "term_substitution" (they consistently replace one word/phrase with another), "style" (tone, length, formatting), "section_preference" (something about a whole named section), "freeform" (anything else).
- For "term_substitution", set `from` (what the model wrote) and `to` (what they prefer); you may omit `rule`.
- For every other kind, set `rule` to a single imperative sentence, and omit `from`/`to`.
- `scope` is "global", or "section:<Exact Section Name>" for a section-specific habit.
- Do NOT invent preferences. If you see no repeated pattern, return {"rules": []}.
- Never restate the meeting content; describe only the editing preference."#;

    let mut body = String::from(
        "Here are recent corrections (what the model wrote → what the user kept):\n\n",
    );
    for (i, c) in corrections
        .iter()
        .take(MAX_CORRECTIONS_IN_PROMPT)
        .enumerate()
    {
        let n = i + 1;
        let where_ = c
            .section
            .map(|s| format!(" [in section: {s}]"))
            .unwrap_or_default();
        if c.rejected {
            body.push_str(&format!("{n}.{where_} REJECTED: \"{}\"\n", c.original));
        } else {
            body.push_str(&format!(
                "{n}.{where_} \"{}\" -> \"{}\"\n",
                c.original, c.final_text
            ));
        }
    }
    body.push_str("\nName the editing preferences you can justify from repeated patterns above.");
    (system.to_string(), body)
}

/// The scope token if it parses, else `None` — an unparseable scope drops the
/// rule rather than silently widening it to global.
fn scope_of(raw: Option<&str>) -> Option<RuleScope> {
    match raw {
        None => Some(RuleScope::Global),
        Some(token) => RuleScope::from_db(token).ok(),
    }
}

fn kind_of(raw: Option<&str>) -> RuleKind {
    match raw {
        Some("term_substitution") => RuleKind::TermSubstitution,
        Some("style") => RuleKind::Style,
        Some("section_preference") => RuleKind::SectionPreference,
        _ => RuleKind::Freeform,
    }
}

/// Parse a model reply into candidates, dropping anything malformed, anything
/// whose signature is already on record (`known_signatures`), and any duplicate
/// within the batch. Order is stable (by signature) for reproducibility.
///
/// Returns an empty vec on unparseable JSON — a garbled reply means "learned
/// nothing this pass", never an error that would surface to the user.
pub fn parse_llm_rules(reply: &str, known_signatures: &[String]) -> Vec<RuleCandidate> {
    let parsed: LlmReply = match serde_json::from_str(reply) {
        Ok(r) => r,
        Err(_) => {
            // Tolerate a reply wrapped in prose/fences by extracting the first
            // balanced object — reuse would be nicer, but keep this module
            // network- and crate-independent for the harness.
            match extract_first_object(reply).and_then(|o| serde_json::from_str(o).ok()) {
                Some(r) => r,
                None => return Vec::new(),
            }
        }
    };

    let mut seen = std::collections::HashSet::new();
    let mut out: Vec<RuleCandidate> = Vec::new();

    for rule in parsed.rules {
        let Some(scope) = scope_of(rule.scope.as_deref()) else {
            continue;
        };
        let scope_token = scope.to_db();
        let kind = kind_of(rule.kind.as_deref());

        let (signature, rule_text) = if matches!(kind, RuleKind::TermSubstitution) {
            // Needs both sides, and normalizes them the SAME way the
            // deterministic miner does so the two dedup.
            let (Some(from), Some(to)) = (rule.from.as_deref(), rule.to.as_deref()) else {
                continue;
            };
            let nf = normalize(from).strict;
            let nt = normalize(to).strict;
            if nf.is_empty() || nt.is_empty() || nf == nt {
                continue;
            }
            let signature = format!("term_substitution|{scope_token}|{nf}=>{nt}");
            let text = rule
                .rule
                .filter(|r| !r.trim().is_empty())
                .unwrap_or_else(|| format!("Say \"{to}\" rather than \"{from}\"."));
            (signature, text)
        } else {
            // Everything else needs a plain-language rule.
            let Some(text) = rule
                .rule
                .map(|r| r.trim().to_string())
                .filter(|r| !r.is_empty())
            else {
                continue;
            };
            // Signature keyed on the normalized text so identical suggestions
            // dedup; prefixed `llm|` so it never collides with a deterministic
            // signature of a different shape.
            let signature = format!("llm|{scope_token}|{}", normalize(&text).strict);
            (signature, text)
        };

        if known_signatures.iter().any(|s| s == &signature) || !seen.insert(signature.clone()) {
            continue;
        }

        out.push(RuleCandidate {
            signature,
            scope,
            kind,
            rule_text,
            // Never fabricated — see the module docs.
            support_count: 0,
            evidence: Vec::new(),
        });
    }

    out.sort_by(|a, b| a.signature.cmp(&b.signature));
    out
}

/// The first `{`…`}`-balanced object in a string, for replies wrapped in prose
/// or code fences. Ignores braces inside string literals.
fn extract_first_object(text: &str) -> Option<&str> {
    let start = text.find('{')?;
    let bytes = text.as_bytes();
    let mut depth = 0usize;
    let mut in_str = false;
    let mut escaped = false;
    for i in start..bytes.len() {
        let c = bytes[i];
        if in_str {
            if escaped {
                escaped = false;
            } else if c == b'\\' {
                escaped = true;
            } else if c == b'"' {
                in_str = false;
            }
            continue;
        }
        match c {
            b'"' => in_str = true,
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&text[start..=i]);
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_clean_term_substitution_reply_parses_and_matches_the_deterministic_signature() {
        let reply = r#"{"rules":[{"kind":"term_substitution","scope":"global","from":"aksiyon","to":"takip"}]}"#;
        let rules = parse_llm_rules(reply, &[]);
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].rule_text, "Say \"takip\" rather than \"aksiyon\".");
        // THE dedup property: identical to what `learning::miner` produces.
        assert_eq!(
            rules[0].signature,
            "term_substitution|global|aksiyon=>takip"
        );
        assert_eq!(rules[0].support_count, 0, "never fabricated");
        assert!(rules[0].evidence.is_empty(), "no invented provenance");
    }

    #[test]
    fn a_signature_already_on_record_is_dropped() {
        let reply = r#"{"rules":[{"kind":"term_substitution","scope":"global","from":"aksiyon","to":"takip"}]}"#;
        let known = vec!["term_substitution|global|aksiyon=>takip".to_string()];
        assert!(
            parse_llm_rules(reply, &known).is_empty(),
            "the deterministic miner already found it, or the user dismissed it",
        );
    }

    #[test]
    fn a_freeform_rule_gets_an_llm_prefixed_signature() {
        let reply = r#"{"rules":[{"kind":"style","scope":"global","rule":"Write decisions as one sentence."}]}"#;
        let rules = parse_llm_rules(reply, &[]);
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].kind, RuleKind::Style);
        assert!(
            rules[0].signature.starts_with("llm|global|"),
            "got: {}",
            rules[0].signature,
        );
    }

    #[test]
    fn a_section_scope_survives_and_a_bad_scope_drops_the_rule() {
        let good = r#"{"rules":[{"kind":"section_preference","scope":"section:Risks","rule":"Keep it short."}]}"#;
        let rules = parse_llm_rules(good, &[]);
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].scope, RuleScope::Section("Risks".to_string()));

        let bad = r#"{"rules":[{"kind":"style","scope":"nonsense:x","rule":"whatever"}]}"#;
        assert!(
            parse_llm_rules(bad, &[]).is_empty(),
            "an unparseable scope drops the rule rather than widening it to global",
        );
    }

    #[test]
    fn term_substitution_missing_a_side_is_dropped_and_a_noop_rename_too() {
        // from/to required
        assert!(parse_llm_rules(
            r#"{"rules":[{"kind":"term_substitution","scope":"global","from":"x"}]}"#,
            &[]
        )
        .is_empty());
        // from == to after normalization is not a substitution
        assert!(parse_llm_rules(
            r#"{"rules":[{"kind":"term_substitution","scope":"global","from":"Takip","to":"takip"}]}"#,
            &[]
        )
        .is_empty());
    }

    #[test]
    fn duplicates_within_one_reply_collapse() {
        let reply = r#"{"rules":[
            {"kind":"style","scope":"global","rule":"Be concise."},
            {"kind":"style","scope":"global","rule":"be concise"}
        ]}"#;
        // Both normalize to the same signature.
        assert_eq!(parse_llm_rules(reply, &[]).len(), 1);
    }

    #[test]
    fn a_reply_wrapped_in_prose_or_fences_still_parses() {
        let reply = "Sure! Here you go:\n```json\n{\"rules\":[{\"kind\":\"style\",\"scope\":\"global\",\"rule\":\"Prefer active voice.\"}]}\n```\nHope that helps.";
        let rules = parse_llm_rules(reply, &[]);
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].rule_text, "Prefer active voice.");
    }

    #[test]
    fn garbage_yields_no_rules_never_an_error() {
        assert!(parse_llm_rules("not json at all", &[]).is_empty());
        assert!(parse_llm_rules("", &[]).is_empty());
        assert!(parse_llm_rules("{}", &[]).is_empty());
        assert!(parse_llm_rules(r#"{"rules":[]}"#, &[]).is_empty());
    }

    #[test]
    fn the_prompt_shows_deltas_and_rejects_without_ids_or_meeting_refs() {
        let corrections = vec![
            CorrectionSummary {
                original: "3 aksiyon çıktı",
                final_text: "3 takip çıktı",
                section: Some("Kararlar"),
                rejected: false,
            },
            CorrectionSummary {
                original: "müşteri memnun görünüyordu",
                final_text: "",
                section: None,
                rejected: true,
            },
        ];
        let (system, body) = build_llm_miner_prompt(&corrections);
        assert!(system.contains("term_substitution"));
        assert!(body.contains("\"3 aksiyon çıktı\" -> \"3 takip çıktı\""));
        assert!(body.contains("[in section: Kararlar]"));
        assert!(body.contains("REJECTED: \"müşteri memnun görünüyordu\""));
    }
}
