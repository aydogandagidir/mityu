//! Deterministic rule mining (ADR-0024 §8) — the part that actually learns.
//!
//! Reads the append-only correction log and proposes plain-language rules. No
//! model, no network, no cost: everything here is arithmetic over text the user
//! already produced. That is why it is the DEFAULT miner and the LLM one (§8,
//! phase C2) is opt-in.
//!
//! ## Four decisions worth reading before changing anything
//!
//! **1. One subject, one vote.** Events are collapsed to the LAST verdict per
//! `(meeting_id, subject_id)` before anything is counted. A user who edits the
//! same block twice has not corrected you twice — they corrected you once and
//! then refined it — and a user who rejects a block and RESTORES it has not
//! rejected anything at all. Counting raw events would let one indecisive
//! afternoon manufacture a rule.
//!
//! **2. A term substitution is one small replacement, not a rewrite.** Only a
//! diff with exactly ONE contiguous replacement block, both sides short, counts.
//! When someone rewrites a sentence wholesale, the word-level diff is real but
//! it means nothing — "they replaced these nine words with those eleven" is not
//! a preference, and mining it would produce confident nonsense.
//!
//! **3. `strict` normalization, never `folded`.** A user correcting "konveyor"
//! to "konveyör" is teaching exactly the distinction folding erases; a folded
//! miner sees no change and learns nothing. See `crate::text`.
//!
//! **4. A signature, not the text, is the identity.** Rules are user-editable,
//! so matching a candidate against existing `rule_text` would re-propose
//! everything the moment someone reworded it. Every candidate carries a stable
//! `signature` derived from the correction itself.

use crate::database::repositories::correction_event::{CorrectionAction, CorrectionEventRow};
use crate::learning::rule::{RuleKind, RuleScope};
use crate::text::normalize;
use std::collections::HashMap;

/// Longest run of words on either side of a replacement that still counts as a
/// term substitution rather than a rewrite. Three covers "aksiyon" → "takip
/// maddesi" and the like; beyond that the user is rephrasing, not renaming.
const MAX_SUBSTITUTION_WORDS: usize = 3;

/// How much of the text must survive an edit for it to be a term substitution.
/// Below this the edit is a rewrite that happens to share a few words.
const MIN_UNCHANGED_RATIO: f64 = 0.5;

/// A rule the miner is proposing, before it becomes a row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleCandidate {
    /// Stable identity, independent of how the rule is worded — see decision 4.
    pub signature: String,
    /// Where it applies.
    pub scope: RuleScope,
    /// What kind of preference it expresses.
    pub kind: RuleKind,
    /// The proposed rule, in plain language.
    pub rule_text: String,
    /// How many DISTINCT corrections back it (after decision 1's collapsing).
    pub support_count: i64,
    /// The correction events behind it.
    pub evidence: Vec<String>,
}

/// One contiguous replacement found in a word-level diff.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Substitution {
    from: String,
    to: String,
}

/// Word-level diff, returning a substitution ONLY when the edit looks like a
/// rename rather than a rewrite (decision 2).
///
/// `None` when: nothing changed, more than one region changed, either side is
/// longer than [`MAX_SUBSTITUTION_WORDS`], or too little of the text survived.
fn single_substitution(original: &str, final_text: &str) -> Option<Substitution> {
    let a: Vec<&str> = original.split_whitespace().collect();
    let b: Vec<&str> = final_text.split_whitespace().collect();
    if a.is_empty() || b.is_empty() || a == b {
        return None;
    }

    // Common prefix and suffix; whatever is left in the middle is the change.
    let prefix = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let suffix = a[prefix..]
        .iter()
        .rev()
        .zip(b[prefix..].iter().rev())
        .take_while(|(x, y)| x == y)
        .count();

    let from: Vec<&str> = a[prefix..a.len() - suffix].to_vec();
    let to: Vec<&str> = b[prefix..b.len() - suffix].to_vec();

    // A pure insertion or deletion is not a substitution: "they added a word"
    // teaches nothing about what to call things.
    if from.is_empty() || to.is_empty() {
        return None;
    }
    if from.len() > MAX_SUBSTITUTION_WORDS || to.len() > MAX_SUBSTITUTION_WORDS {
        return None;
    }
    // Enough of the sentence has to survive, or this is a rewrite that happens
    // to share a first and last word.
    let unchanged = prefix + suffix;
    if (unchanged as f64) < MIN_UNCHANGED_RATIO * (a.len() as f64) {
        return None;
    }

    Some(Substitution {
        from: from.join(" "),
        to: to.join(" "),
    })
}

/// Collapses the log to the LAST verdict per `(meeting_id, subject_id)` —
/// decision 1.
///
/// This is where a restore cancels its reject and a re-edit stops double-voting:
/// both are simply not the last word on that subject. Doing it here, once, means
/// no individual miner has to remember.
fn last_verdict_per_subject(events: &[CorrectionEventRow]) -> Vec<&CorrectionEventRow> {
    let mut latest: HashMap<(&str, &str), &CorrectionEventRow> = HashMap::new();
    for event in events {
        let key = (event.meeting_id.as_str(), event.subject_id.as_str());
        latest
            .entry(key)
            .and_modify(|existing| {
                // RFC 3339 strings from the same writer sort lexicographically;
                // the id breaks a same-instant tie so the result is stable rather
                // than dependent on HashMap iteration order.
                if (&event.created_at, &event.id) > (&existing.created_at, &existing.id) {
                    *existing = event;
                }
            })
            .or_insert(event);
    }
    let mut out: Vec<&CorrectionEventRow> = latest.into_values().collect();
    out.sort_by(|a, b| (&a.created_at, &a.id).cmp(&(&b.created_at, &b.id)));
    out
}

/// "You keep renaming X to Y."
fn mine_term_substitutions(
    verdicts: &[&CorrectionEventRow],
    min_support: i64,
) -> Vec<RuleCandidate> {
    // (from, to) → the events that show it.
    let mut hits: HashMap<(String, String), Vec<&CorrectionEventRow>> = HashMap::new();

    for event in verdicts {
        if event.action != CorrectionAction::Edit {
            continue;
        }
        let (Some(original), Some(final_text)) = (&event.original_text, &event.final_text) else {
            continue;
        };
        // strict, never folded — decision 3.
        let original = normalize(original).strict;
        let final_text = normalize(final_text).strict;
        if let Some(sub) = single_substitution(&original, &final_text) {
            hits.entry((sub.from, sub.to)).or_default().push(event);
        }
    }

    let mut candidates: Vec<RuleCandidate> = hits
        .into_iter()
        .filter(|(_, events)| events.len() as i64 >= min_support)
        .map(|((from, to), events)| RuleCandidate {
            signature: format!("term_substitution|global|{from}=>{to}"),
            scope: RuleScope::Global,
            kind: RuleKind::TermSubstitution,
            rule_text: format!("Say \"{to}\" rather than \"{from}\"."),
            support_count: events.len() as i64,
            evidence: events.iter().map(|e| e.id.clone()).collect(),
        })
        .collect();
    candidates.sort_by(|a, b| a.signature.cmp(&b.signature));
    candidates
}

/// "You throw away everything this section produces."
///
/// Scoped to the section AND counted only against that section's own verdicts:
/// a section the user rejects three times but also keeps twice is not a section
/// they want gone, it is one the model is getting wrong half the time — a
/// different problem, and not one a rule can fix.
fn mine_section_rejections(
    verdicts: &[&CorrectionEventRow],
    min_support: i64,
) -> Vec<RuleCandidate> {
    let mut by_section: HashMap<&str, (Vec<&CorrectionEventRow>, i64)> = HashMap::new();

    for event in verdicts {
        let Some(section) = event.section_title.as_deref() else {
            continue; // action items have no section
        };
        let entry = by_section.entry(section).or_insert((Vec::new(), 0));
        match event.action {
            CorrectionAction::Reject => entry.0.push(event),
            // Anything the user kept is evidence AGAINST dropping the section.
            CorrectionAction::Approve | CorrectionAction::Edit => entry.1 += 1,
            CorrectionAction::Restore => {}
        }
    }

    let mut candidates: Vec<RuleCandidate> = by_section
        .into_iter()
        .filter(|(_, (rejects, kept))| rejects.len() as i64 >= min_support && *kept == 0)
        .map(|(section, (rejects, _))| RuleCandidate {
            signature: format!("section_rejection|section:{section}|drop"),
            scope: RuleScope::Section(section.to_string()),
            kind: RuleKind::SectionPreference,
            rule_text: format!(
                "Leave the \"{section}\" section empty unless the transcript clearly supports it."
            ),
            support_count: rejects.len() as i64,
            evidence: rejects.iter().map(|e| e.id.clone()).collect(),
        })
        .collect();
    candidates.sort_by(|a, b| a.signature.cmp(&b.signature));
    candidates
}

/// Every rule the deterministic miners can find in this log.
///
/// `known_signatures` are the signatures already on record in ANY status,
/// including dismissed — that is the point of keeping a dismissed row (§4): the
/// human already answered, and asking again is nagging. A DELETED rule's
/// signature is deliberately absent from that set, so deleting is the way to say
/// "offer it to me again if I keep doing it".
pub fn mine(
    events: &[CorrectionEventRow],
    min_support: i64,
    known_signatures: &[String],
) -> Vec<RuleCandidate> {
    let verdicts = last_verdict_per_subject(events);
    let mut candidates = mine_term_substitutions(&verdicts, min_support);
    candidates.extend(mine_section_rejections(&verdicts, min_support));
    candidates.retain(|c| !known_signatures.contains(&c.signature));
    candidates.sort_by(|a, b| a.signature.cmp(&b.signature));
    candidates
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::repositories::correction_event::CorrectionSubject;

    fn event(
        id: &str,
        meeting: &str,
        subject: &str,
        action: CorrectionAction,
        original: Option<&str>,
        final_text: Option<&str>,
        section: Option<&str>,
        created_at: &str,
    ) -> CorrectionEventRow {
        CorrectionEventRow {
            id: id.to_string(),
            meeting_id: meeting.to_string(),
            subject_kind: CorrectionSubject::SummaryBlock,
            subject_id: subject.to_string(),
            action,
            original_text: original.map(str::to_string),
            final_text: final_text.map(str::to_string),
            reason: None,
            block_type: Some("bullet".to_string()),
            section_title: section.map(str::to_string),
            template_id: Some("daily_standup".to_string()),
            model: Some("llama3.2".to_string()),
            created_at: created_at.to_string(),
        }
    }

    /// A rename in different meetings, three times: the signal the whole system
    /// exists for.
    #[test]
    fn a_repeated_rename_becomes_a_rule() {
        let events: Vec<CorrectionEventRow> = (1..=3)
            .map(|i| {
                event(
                    &format!("e{i}"),
                    &format!("m{i}"),
                    "b1",
                    CorrectionAction::Edit,
                    Some("3 aksiyon çıktı"),
                    Some("3 takip çıktı"),
                    Some("Kararlar"),
                    &format!("2026-07-1{i}T10:00:00Z"),
                )
            })
            .collect();

        let mined = mine(&events, 3, &[]);
        assert_eq!(mined.len(), 1);
        assert_eq!(mined[0].rule_text, "Say \"takip\" rather than \"aksiyon\".");
        assert_eq!(mined[0].support_count, 3);
        assert_eq!(mined[0].kind, RuleKind::TermSubstitution);
        assert_eq!(mined[0].scope, RuleScope::Global);
        assert_eq!(mined[0].evidence, ["e1", "e2", "e3"]);
    }

    #[test]
    fn below_the_threshold_nothing_is_proposed() {
        let events: Vec<CorrectionEventRow> = (1..=2)
            .map(|i| {
                event(
                    &format!("e{i}"),
                    &format!("m{i}"),
                    "b1",
                    CorrectionAction::Edit,
                    Some("3 aksiyon çıktı"),
                    Some("3 takip çıktı"),
                    None,
                    &format!("2026-07-1{i}T10:00:00Z"),
                )
            })
            .collect();
        assert!(mine(&events, 3, &[]).is_empty());
    }

    /// Decision 1. Editing the same block three times is ONE correction, refined
    /// — not three. Without the collapse, one indecisive afternoon manufactures a
    /// rule out of a single opinion.
    #[test]
    fn re_editing_one_block_is_one_vote_not_three() {
        let events: Vec<CorrectionEventRow> = (1..=3)
            .map(|i| {
                event(
                    &format!("e{i}"),
                    "m1",
                    "b1",
                    CorrectionAction::Edit,
                    Some("3 aksiyon çıktı"),
                    Some("3 takip çıktı"),
                    None,
                    &format!("2026-07-1{i}T10:00:00Z"),
                )
            })
            .collect();
        assert!(
            mine(&events, 3, &[]).is_empty(),
            "three edits of ONE block must not clear a threshold of three",
        );
        // …and one edit each on three blocks does.
        let spread: Vec<CorrectionEventRow> = (1..=3)
            .map(|i| {
                event(
                    &format!("e{i}"),
                    "m1",
                    &format!("b{i}"),
                    CorrectionAction::Edit,
                    Some("3 aksiyon çıktı"),
                    Some("3 takip çıktı"),
                    None,
                    &format!("2026-07-1{i}T10:00:00Z"),
                )
            })
            .collect();
        assert_eq!(mine(&spread, 3, &[]).len(), 1);
    }

    /// Decision 1, the other half: a restore RETRACTS its reject. Counting the
    /// reject anyway would teach a preference the user explicitly took back.
    #[test]
    fn a_restored_reject_does_not_count_against_its_section() {
        let mut events = vec![];
        for i in 1..=3 {
            events.push(event(
                &format!("r{i}"),
                &format!("m{i}"),
                "b1",
                CorrectionAction::Reject,
                Some("bir şey"),
                None,
                Some("Riskler"),
                &format!("2026-07-0{i}T10:00:00Z"),
            ));
        }
        assert_eq!(mine(&events, 3, &[]).len(), 1, "three rejects → a rule");

        // The user takes the last one back.
        events.push(event(
            "r4",
            "m3",
            "b1",
            CorrectionAction::Restore,
            Some("bir şey"),
            None,
            Some("Riskler"),
            "2026-07-03T11:00:00Z",
        ));
        assert!(
            mine(&events, 3, &[]).is_empty(),
            "the restore is the last word on m3/b1, so only two rejects stand",
        );
    }

    /// Decision 2. A wholesale rewrite has a real word-diff and no meaning.
    #[test]
    fn a_rewrite_is_not_a_rename() {
        let events: Vec<CorrectionEventRow> = (1..=3)
            .map(|i| {
                event(
                    &format!("e{i}"),
                    &format!("m{i}"),
                    "b1",
                    CorrectionAction::Edit,
                    Some("The customer asked about pricing on the conveyor line"),
                    Some("Fiyat revizyonu talep edildi ve teklif hazırlanacak"),
                    None,
                    &format!("2026-07-1{i}T10:00:00Z"),
                )
            })
            .collect();
        assert!(mine(&events, 3, &[]).is_empty());
    }

    #[test]
    fn pure_insertions_and_deletions_are_not_substitutions() {
        assert_eq!(single_substitution("a b c", "a b c d"), None, "insertion");
        assert_eq!(single_substitution("a b c d", "a b c"), None, "deletion");
        assert_eq!(single_substitution("a b c", "a b c"), None, "no change");
        assert_eq!(single_substitution("", "a"), None, "empty");
    }

    #[test]
    fn a_one_to_many_word_rename_still_counts() {
        assert_eq!(
            single_substitution("3 aksiyon çıktı", "3 takip maddesi çıktı"),
            Some(Substitution {
                from: "aksiyon".to_string(),
                to: "takip maddesi".to_string(),
            }),
        );
    }

    /// Decision 3, at the miner level: a diacritic-only correction is exactly the
    /// kind of thing a Turkish user teaches, and folding would erase it.
    #[test]
    fn a_diacritic_only_rename_is_learned() {
        let events: Vec<CorrectionEventRow> = (1..=3)
            .map(|i| {
                event(
                    &format!("e{i}"),
                    &format!("m{i}"),
                    "b1",
                    CorrectionAction::Edit,
                    Some("konveyor durdu"),
                    Some("konveyör durdu"),
                    None,
                    &format!("2026-07-1{i}T10:00:00Z"),
                )
            })
            .collect();
        let mined = mine(&events, 3, &[]);
        assert_eq!(mined.len(), 1);
        assert_eq!(
            mined[0].rule_text,
            "Say \"konveyör\" rather than \"konveyor\".",
        );
    }

    /// Decision 4. The signature, not the text, is the identity — so a rule the
    /// user reworded is still recognised and not offered twice.
    #[test]
    fn a_known_signature_is_never_re_proposed() {
        let events: Vec<CorrectionEventRow> = (1..=3)
            .map(|i| {
                event(
                    &format!("e{i}"),
                    &format!("m{i}"),
                    "b1",
                    CorrectionAction::Edit,
                    Some("3 aksiyon çıktı"),
                    Some("3 takip çıktı"),
                    None,
                    &format!("2026-07-1{i}T10:00:00Z"),
                )
            })
            .collect();
        let first = mine(&events, 3, &[]);
        assert_eq!(first.len(), 1);

        let known = vec![first[0].signature.clone()];
        assert!(
            mine(&events, 3, &known).is_empty(),
            "the human already answered this one",
        );
        assert_eq!(
            first[0].signature,
            "term_substitution|global|aksiyon=>takip"
        );
    }

    /// A section the user sometimes keeps is not a section they want gone — it is
    /// one the model gets wrong half the time, which no rule fixes.
    #[test]
    fn a_section_that_is_ever_kept_is_not_proposed_for_removal() {
        let mut events: Vec<CorrectionEventRow> = (1..=3)
            .map(|i| {
                event(
                    &format!("r{i}"),
                    &format!("m{i}"),
                    "b1",
                    CorrectionAction::Reject,
                    Some("bir şey"),
                    None,
                    Some("Riskler"),
                    &format!("2026-07-0{i}T10:00:00Z"),
                )
            })
            .collect();
        events.push(event(
            "a1",
            "m4",
            "b1",
            CorrectionAction::Approve,
            Some("gerçek bir risk"),
            Some("gerçek bir risk"),
            Some("Riskler"),
            "2026-07-04T10:00:00Z",
        ));
        assert!(mine(&events, 3, &[]).is_empty());
    }

    #[test]
    fn a_section_rejected_every_time_is_proposed_for_removal() {
        let events: Vec<CorrectionEventRow> = (1..=3)
            .map(|i| {
                event(
                    &format!("r{i}"),
                    &format!("m{i}"),
                    "b1",
                    CorrectionAction::Reject,
                    Some("bir şey"),
                    None,
                    Some("Riskler"),
                    &format!("2026-07-0{i}T10:00:00Z"),
                )
            })
            .collect();
        let mined = mine(&events, 3, &[]);
        assert_eq!(mined.len(), 1);
        assert_eq!(mined[0].scope, RuleScope::Section("Riskler".to_string()));
        assert_eq!(mined[0].kind, RuleKind::SectionPreference);
        assert!(mined[0].rule_text.contains("\"Riskler\""));
    }

    /// The order rules are proposed in must not depend on HashMap iteration.
    #[test]
    fn output_is_deterministic() {
        let mut events: Vec<CorrectionEventRow> = vec![];
        for (n, (from, to)) in [("aksiyon", "takip"), ("madde", "başlık")]
            .iter()
            .enumerate()
        {
            for i in 1..=3 {
                events.push(event(
                    &format!("e{n}{i}"),
                    &format!("m{n}{i}"),
                    "b1",
                    CorrectionAction::Edit,
                    Some(&format!("3 {from} çıktı")),
                    Some(&format!("3 {to} çıktı")),
                    None,
                    &format!("2026-07-1{i}T10:0{n}:00Z"),
                ));
            }
        }
        let first: Vec<String> = mine(&events, 3, &[])
            .iter()
            .map(|c| c.signature.clone())
            .collect();
        for _ in 0..5 {
            let again: Vec<String> = mine(&events, 3, &[])
                .iter()
                .map(|c| c.signature.clone())
                .collect();
            assert_eq!(first, again);
        }
        assert_eq!(first.len(), 2);
    }
}
