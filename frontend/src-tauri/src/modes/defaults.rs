//! The eight built-in modes (BACKLOG I4a, ADR-0038).
//!
//! Written in Rust rather than loaded from JSON on purpose: these ship with the
//! binary, they must exist before any directory is readable, and the compiler
//! then checks every field the way it cannot check a file. Custom modes are the
//! JSON path, and they go through [`super::validator`].
//!
//! ## What is *not* here, and why the absence is tested
//!
//! There is no candidate-side interview mode, no exam mode and no
//! coding-problem mode. That is the category ADR-0038 rejects, and
//! [`tests::no_builtin_mode_helps_someone_be_assessed`] fails if one appears —
//! an absence nobody notices otherwise.
//!
//! ## Every mode resolves to a template that is compiled in
//!
//! `summary::templates::get_template` tries the user's custom directory, then
//! the bundled resources directory (set at startup from
//! `lib.rs`), then the templates compiled into the binary. A mode pointing at a
//! template that exists only as a bundled *file* would therefore resolve on a
//! running app and fail anywhere the resources directory has not been set —
//! including `cargo test`. So every id below names a compiled built-in, and
//! [`tests::every_builtin_mode_resolves_to_a_template`] proves it with no
//! runtime state set at all.

use super::types::{AllowedSource, EvidencePolicy, LiveAction, LivePolicy, Mode};

/// Ids of the modes that ship with the app, in the order Settings shows them.
pub const BUILTIN_MODE_IDS: [&str; 8] = [
    "general",
    "client_call",
    "team_meeting",
    "recruiting_interview",
    "lecture",
    "consultation",
    "field_visit",
    "support_call",
];

/// One built-in mode, written with named fields.
///
/// Deliberately not a positional helper. The two fields that matter most are
/// `user_role` and `counterpart_role` — swapping them turns the interviewer's
/// mode into the candidate's, which is the exact category ADR-0038 rejects —
/// and eleven positional strings make that swap a typo rather than a decision.
struct Spec<'a> {
    id: &'a str,
    name: &'a str,
    purpose: &'a str,
    /// Who the **user** is in this conversation. Never the person being assessed.
    user_role: &'a str,
    /// Who is across the table.
    counterpart_role: &'a str,
    voice: &'a str,
    summary_template_id: &'a str,
    actions: Vec<LiveAction>,
    evidence_policy: EvidencePolicy,
    cite_required: bool,
    sources: Vec<AllowedSource>,
}

impl Spec<'_> {
    fn build(self) -> Mode {
        Mode {
            id: self.id.to_string(),
            name: self.name.to_string(),
            purpose: self.purpose.to_string(),
            user_role: self.user_role.to_string(),
            counterpart_role: self.counterpart_role.to_string(),
            voice: self.voice.to_string(),
            summary_template_id: self.summary_template_id.to_string(),
            live: LivePolicy {
                allowed_actions: self.actions,
                evidence_policy: self.evidence_policy,
                cite_required: self.cite_required,
            },
            allowed_sources: self.sources,
        }
    }
}

/// Build the built-in modes. Cheap enough to call per request; there are eight.
pub fn builtin_modes() -> Vec<Mode> {
    use AllowedSource::{Knowledge, Transcript};
    use EvidencePolicy::{Open, SourceFirst};
    use LiveAction::{Define, FollowUpQuestions, Recap, Suggest};

    vec![
        Spec {
            id: "general",
            name: "General",
            purpose: "An ordinary working conversation with no particular shape. Help the user follow it and remember what was said.",
            user_role: "a participant",
            counterpart_role: "the other participants",
            voice: "Plain and short. Prefer the speaker's own words over a paraphrase.",
            summary_template_id: "standard_meeting",
            actions: vec![Suggest, FollowUpQuestions, Recap, Define],
            evidence_policy: SourceFirst,
            cite_required: true,
            sources: vec![Transcript, Knowledge],
        }
        .build(),
        Spec {
            id: "client_call",
            name: "Client or sales call",
            purpose: "A conversation with a customer or prospect. Help the user answer accurately, hear what was actually asked, and leave with the commitments written down.",
            user_role: "the person representing their own company",
            counterpart_role: "the client or prospect",
            voice: "Professional and concrete. Never invent a capability, a price or a date.",
            summary_template_id: "sales_marketing_client_call",
            actions: vec![Suggest, FollowUpQuestions, Recap, Define],
            evidence_policy: SourceFirst,
            cite_required: true,
            sources: vec![Transcript, Knowledge],
        }
        .build(),
        Spec {
            id: "team_meeting",
            name: "Team meeting",
            purpose: "An internal working session. Help the user track decisions, owners and what is still open.",
            user_role: "a team member",
            counterpart_role: "colleagues",
            voice: "Direct and unceremonious, the way colleagues talk.",
            summary_template_id: "project_sync",
            actions: vec![FollowUpQuestions, Recap, Define],
            evidence_policy: SourceFirst,
            cite_required: true,
            sources: vec![Transcript, Knowledge],
        }
        .build(),
        Spec {
            id: "recruiting_interview",
            name: "Recruiting interview (interviewer side)",
            purpose: "An interview the user is *conducting*. Help them ask better follow-up questions and keep a fair record of what the candidate actually said.",
            user_role: "the interviewer",
            counterpart_role: "the candidate",
            voice: "Neutral and non-leading. Never evaluate, score or rank a person.",
            summary_template_id: "standard_meeting",
            // Deliberately no Suggest: "what should I say next" in an interview
            // is a step toward putting words in the interviewer's mouth, and
            // from there toward assessing the candidate — which ADR-0038 and
            // ADR-0034 both rule out.
            actions: vec![FollowUpQuestions, Recap],
            evidence_policy: SourceFirst,
            cite_required: true,
            sources: vec![Transcript],
        }
        .build(),
        Spec {
            id: "lecture",
            name: "Lecture or training",
            purpose: "A session the user is listening to and learning from. Help them keep up and understand the terms being used.",
            user_role: "a listener",
            counterpart_role: "the speaker",
            voice: "Explanatory. A definition may go beyond what was said, and is marked as doing so.",
            summary_template_id: "standard_meeting",
            actions: vec![Recap, Define],
            // The one mode where answering from general knowledge is the point:
            // explaining a term the speaker used but did not define.
            evidence_policy: Open,
            cite_required: false,
            sources: vec![Transcript, Knowledge],
        }
        .build(),
        Spec {
            id: "consultation",
            name: "Consultation (professional services)",
            purpose: "A client consultation in a professional-services setting. Help the user capture the facts precisely and notice what has not been asked yet.",
            user_role: "the professional advising",
            counterpart_role: "the client",
            voice: "Careful and precise. State what was said; never offer an opinion the user did not give.",
            summary_template_id: "standard_meeting",
            actions: vec![FollowUpQuestions, Recap, Define],
            evidence_policy: SourceFirst,
            cite_required: true,
            sources: vec![Transcript, Knowledge],
        }
        .build(),
        Spec {
            id: "field_visit",
            name: "Field or site visit",
            purpose: "A conversation on site, often noisy and interrupted. Help the user keep the findings, measurements and commitments straight.",
            user_role: "the visiting engineer or inspector",
            counterpart_role: "the site staff",
            voice: "Terse. Numbers, locations and named parts exactly as spoken.",
            summary_template_id: "standard_meeting",
            actions: vec![FollowUpQuestions, Recap],
            evidence_policy: SourceFirst,
            cite_required: true,
            sources: vec![Transcript, Knowledge],
        }
        .build(),
        Spec {
            id: "support_call",
            name: "Support call",
            purpose: "A customer reporting a problem. Help the user get the facts needed to reproduce it and agree the next step.",
            user_role: "the support engineer",
            counterpart_role: "the customer",
            voice: "Patient and specific. Distinguish what the customer observed from what they concluded.",
            summary_template_id: "standard_meeting",
            actions: vec![Suggest, FollowUpQuestions, Recap, Define],
            evidence_policy: SourceFirst,
            cite_required: true,
            sources: vec![Transcript, Knowledge],
        }
        .build(),
    ]
}

/// One built-in mode by id.
pub fn builtin_mode(id: &str) -> Option<Mode> {
    builtin_modes().into_iter().find(|m| m.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::summary::templates;

    #[test]
    fn the_id_list_matches_the_modes_it_indexes() {
        let ids: Vec<String> = builtin_modes().into_iter().map(|m| m.id).collect();
        assert_eq!(ids, BUILTIN_MODE_IDS.to_vec());
    }

    #[test]
    fn built_in_ids_are_unique() {
        let mut ids = BUILTIN_MODE_IDS.to_vec();
        ids.sort_unstable();
        let before = ids.len();
        ids.dedup();
        assert_eq!(before, ids.len(), "two built-in modes share an id");
    }

    /// The AC's own verification line, and the reason every mode points at a
    /// *compiled* template: this test sets no bundled directory, so it passes
    /// only if the template is in the binary.
    #[test]
    fn every_builtin_mode_resolves_to_a_template() {
        for mode in builtin_modes() {
            let template = templates::get_template(&mode.summary_template_id).unwrap_or_else(|e| {
                panic!(
                    "mode '{}' points at template '{}', which does not resolve: {e}",
                    mode.id, mode.summary_template_id
                )
            });
            assert!(
                !template.sections.is_empty(),
                "mode '{}' resolved to an empty template",
                mode.id
            );
        }
    }

    /// ADR-0038's reject list, as an absence test. A future contributor adding
    /// "interview prep" or "exam helper" to the list above trips this.
    ///
    /// The distinction the first draft of this test got wrong, and the one that
    /// actually matters: **which side the user is on**. "the candidate" as the
    /// *counterpart* is the legitimate interviewer-side mode describing who is
    /// across the table; "the candidate" as the *user's* role is the rejected
    /// category. Banning the bare word would have forbidden the correct design.
    #[test]
    fn no_builtin_mode_helps_someone_be_assessed() {
        const BANNED: [&str; 7] = [
            "exam",
            "quiz",
            "test taking",
            "coding challenge",
            "leetcode",
            "cheat",
            "interview prep",
        ];
        for mode in builtin_modes() {
            let described = format!("{} {} {}", mode.id, mode.name, mode.purpose).to_lowercase();
            for banned in BANNED {
                assert!(
                    !described.contains(banned),
                    "mode '{}' reads as {banned} help, which ADR-0038 rejects",
                    mode.id
                );
            }
            // The user is never the one being assessed.
            let role = mode.user_role.to_lowercase();
            for assessed in ["candidate", "student being examined", "applicant"] {
                assert!(
                    !role.contains(assessed),
                    "mode '{}' puts the user on the assessed side ({assessed})",
                    mode.id
                );
            }
        }
        // The one interview mode that does exist is the interviewer's, and it
        // names the candidate as the counterpart precisely so the assistant
        // knows whose side it is not on.
        let interview = builtin_mode("recruiting_interview").expect("mode exists");
        assert_eq!(interview.user_role, "the interviewer");
        assert_eq!(interview.counterpart_role, "the candidate");
    }

    /// An interview is the one place where "tell me what to say" turns the tool
    /// into something that shapes a decision about a person.
    #[test]
    fn the_interview_mode_does_not_offer_to_write_the_users_lines() {
        let interview = builtin_mode("recruiting_interview").expect("mode exists");
        assert!(!interview.allows(LiveAction::Suggest));
        assert!(interview.allows(LiveAction::FollowUpQuestions));
    }

    #[test]
    fn every_mode_offers_something_and_can_read_the_transcript() {
        for mode in builtin_modes() {
            assert!(
                !mode.live.allowed_actions.is_empty(),
                "mode '{}' offers nothing live",
                mode.id
            );
            assert!(
                mode.allowed_sources.contains(&AllowedSource::Transcript),
                "mode '{}' cannot read the conversation it is in",
                mode.id
            );
            // Whatever else a mode lists, only the transcript can be retrieved
            // from today — and `usable_sources` is what I3 must build on.
            assert_eq!(mode.usable_sources(), vec![AllowedSource::Transcript]);
        }
    }

    /// Every mode that may end up quoted in a client note requires citations.
    /// `lecture` is the deliberate exception and says so in its own field.
    #[test]
    fn only_the_lecture_mode_answers_without_a_citation() {
        for mode in builtin_modes() {
            let expected_open = mode.id == "lecture";
            assert_eq!(
                mode.live.evidence_policy == EvidencePolicy::Open,
                expected_open,
                "mode '{}' has an unexpected evidence policy",
                mode.id
            );
            assert_eq!(
                mode.live.cite_required, !expected_open,
                "mode '{}' has an unexpected citation requirement",
                mode.id
            );
        }
    }

    #[test]
    fn no_field_a_prompt_is_built_from_is_empty() {
        for mode in builtin_modes() {
            for (field, value) in [
                ("name", &mode.name),
                ("purpose", &mode.purpose),
                ("user_role", &mode.user_role),
                ("counterpart_role", &mode.counterpart_role),
                ("voice", &mode.voice),
                ("summary_template_id", &mode.summary_template_id),
            ] {
                assert!(
                    !value.trim().is_empty(),
                    "mode '{}' has an empty {field}",
                    mode.id
                );
            }
        }
    }

    #[test]
    fn a_mode_round_trips_as_camel_case() {
        let mode = builtin_mode("general").expect("mode exists");
        let json = serde_json::to_string(&mode).expect("serialises");
        assert!(
            json.contains("summaryTemplateId"),
            "wire shape changed: {json}"
        );
        assert!(
            json.contains("allowedActions"),
            "wire shape changed: {json}"
        );
        assert!(json.contains("citeRequired"), "wire shape changed: {json}");
        assert_eq!(
            serde_json::from_str::<Mode>(&json).expect("deserialises"),
            mode
        );
    }
}
