//! Checking a custom mode before it is installed (BACKLOG I4a).
//!
//! **Pure by contract.** Nothing here opens a file, reads a directory or asks
//! the operating system anything: the caller hands over the bytes it already
//! read and the ids it already knows, and gets back either a [`Mode`] to
//! preview or a sentence to show. That is what makes "preview before install"
//! possible at all — the UI can run this on text the user pasted, before
//! anything is written anywhere.
//!
//! ## JSON today; the Markdown path is not pretended
//!
//! BACKLOG I4a also names "Markdown-with-front-matter" as a custom-mode format.
//! It is **not implemented here**, and deliberately not stubbed: front matter
//! means a YAML parser, which is a new dependency whose licence CLAUDE.md §9
//! requires reviewing, for a format no user has asked for. The seam costs
//! nothing to add later — [`validate_mode`] takes an already-parsed [`Mode`], so
//! a second format is a second *parser* ([`parse_mode_json`]'s sibling) and not
//! a change to a single rule below.

use super::types::{AllowedSource, Mode};

/// The largest custom mode file that will be read.
///
/// A mode is a page of prose at most; anything larger is a mistake or an
/// attempt to push a document into a prompt. Checked against the raw bytes
/// before parsing, so a huge file is refused without being deserialised.
pub const MAX_MODE_BYTES: usize = 16 * 1024;

/// Ids and phrases a custom mode may not announce itself with.
///
/// ADR-0038 rejects candidate-side interview coaching, exam answering and
/// coding-problem solving. **This is a signpost, not a security control**: a
/// determined user can rename a file and the check will pass. It exists so that
/// the product refuses to be *told* it is doing this, and so the refusal is
/// somewhere a reader can find it — not because a word list can enforce ethics.
const REJECTED_PHRASES: [&str; 10] = [
    "candidate side",
    "candidate-side",
    "interview prep",
    "interview cheat",
    "exam",
    "quiz answer",
    "test taking",
    "coding challenge",
    "leetcode",
    "cheat sheet for the interview",
];

/// Why a custom mode was refused. Each variant renders one actionable sentence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModeError {
    TooLarge { bytes: usize },
    NotJson(String),
    EmptyField(&'static str),
    BadId(String),
    IdCollision(String),
    NameCollision(String),
    NoActions,
    NoSources,
    TranscriptNotAllowed,
    RejectedCategory(String),
}

impl std::fmt::Display for ModeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ModeError::TooLarge { bytes } => write!(
                f,
                "This file is {bytes} bytes; a mode may be at most {MAX_MODE_BYTES}. A mode describes the conversation — it is not the place for a document."
            ),
            ModeError::NotJson(detail) => {
                // The detail is serde's own message, which does not end in a
                // full stop; the sentence supplies one.
                write!(f, "This is not a readable mode file: {detail}.")
            }
            ModeError::EmptyField(field) => write!(
                f,
                "The mode's \"{field}\" is empty. Every field is written into the assistant's instructions, so none may be blank."
            ),
            ModeError::BadId(id) => write!(
                f,
                "\"{id}\" cannot be an id. Use lower-case letters, digits and underscores, 2 to 48 characters."
            ),
            ModeError::IdCollision(id) => write!(
                f,
                "A mode with the id \"{id}\" already exists. Rename this one, or edit the existing mode instead."
            ),
            ModeError::NameCollision(name) => write!(
                f,
                "A mode called \"{name}\" already exists. Two modes with one name cannot be told apart in the panel."
            ),
            ModeError::NoActions => write!(
                f,
                "This mode offers nothing during a conversation. Allow at least one action, or use a summary template instead."
            ),
            ModeError::NoSources => write!(
                f,
                "This mode has no sources to answer from. Allow at least the transcript."
            ),
            ModeError::TranscriptNotAllowed => write!(
                f,
                "A live mode must be allowed to read the transcript of the conversation it is in."
            ),
            ModeError::RejectedCategory(phrase) => write!(
                f,
                "Mityu does not provide help with being assessed — this mode describes itself as \"{phrase}\". It helps you run an interview, not sit one."
            ),
        }
    }
}

impl std::error::Error for ModeError {}

/// Parse a custom mode from JSON bytes, refusing an oversized file first.
///
/// Separated from [`validate_mode`] so a second format can be added without
/// touching a single rule.
pub fn parse_mode_json(raw: &str) -> Result<Mode, ModeError> {
    if raw.len() > MAX_MODE_BYTES {
        return Err(ModeError::TooLarge { bytes: raw.len() });
    }
    serde_json::from_str::<Mode>(raw).map_err(|e| ModeError::NotJson(e.to_string()))
}

/// Check a parsed mode against the modes already installed.
///
/// `existing_ids` and `existing_names` are what the caller already knows —
/// the built-ins plus whatever custom modes are installed. Passing them in
/// rather than reading a directory is what keeps this pure.
pub fn validate_mode(
    mode: &Mode,
    existing_ids: &[String],
    existing_names: &[String],
) -> Result<(), ModeError> {
    if !is_valid_id(&mode.id) {
        return Err(ModeError::BadId(mode.id.clone()));
    }
    for (field, value) in [
        ("name", &mode.name),
        ("purpose", &mode.purpose),
        ("userRole", &mode.user_role),
        ("counterpartRole", &mode.counterpart_role),
        ("voice", &mode.voice),
        ("summaryTemplateId", &mode.summary_template_id),
    ] {
        if value.trim().is_empty() {
            return Err(ModeError::EmptyField(field));
        }
    }

    if let Some(phrase) = rejected_phrase(mode) {
        return Err(ModeError::RejectedCategory(phrase.to_string()));
    }

    if existing_ids.iter().any(|id| id == &mode.id) {
        return Err(ModeError::IdCollision(mode.id.clone()));
    }
    if existing_names
        .iter()
        .any(|name| name.trim().eq_ignore_ascii_case(mode.name.trim()))
    {
        return Err(ModeError::NameCollision(mode.name.clone()));
    }

    if mode.live.allowed_actions.is_empty() {
        return Err(ModeError::NoActions);
    }
    if mode.allowed_sources.is_empty() {
        return Err(ModeError::NoSources);
    }
    if !mode.allowed_sources.contains(&AllowedSource::Transcript) {
        return Err(ModeError::TranscriptNotAllowed);
    }

    Ok(())
}

/// Parse and check in one step — what the "install this file" path calls.
pub fn preview_mode(
    raw: &str,
    existing_ids: &[String],
    existing_names: &[String],
) -> Result<Mode, ModeError> {
    let mode = parse_mode_json(raw)?;
    validate_mode(&mode, existing_ids, existing_names)?;
    Ok(mode)
}

fn is_valid_id(id: &str) -> bool {
    (2..=48).contains(&id.len())
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// Does the mode announce itself as the rejected category?
fn rejected_phrase(mode: &Mode) -> Option<&'static str> {
    let haystack = format!(
        "{} {} {} {} {}",
        mode.id, mode.name, mode.purpose, mode.user_role, mode.counterpart_role
    )
    .to_lowercase()
    .replace('_', " ");
    REJECTED_PHRASES
        .into_iter()
        .find(|phrase| haystack.contains(phrase))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modes::types::{EvidencePolicy, LiveAction, LivePolicy};

    fn valid() -> Mode {
        Mode {
            id: "board_review".into(),
            name: "Board review".into(),
            purpose: "A quarterly review with the board.".into(),
            user_role: "the presenter".into(),
            counterpart_role: "the board".into(),
            voice: "Formal and brief.".into(),
            summary_template_id: "standard_meeting".into(),
            live: LivePolicy {
                allowed_actions: vec![LiveAction::Recap, LiveAction::FollowUpQuestions],
                evidence_policy: EvidencePolicy::SourceFirst,
                cite_required: true,
            },
            allowed_sources: vec![AllowedSource::Transcript],
        }
    }

    #[test]
    fn a_well_formed_custom_mode_is_accepted() {
        assert_eq!(validate_mode(&valid(), &[], &[]), Ok(()));
    }

    #[test]
    fn the_validator_touches_nothing_outside_its_arguments() {
        // Not provable by assertion, so it is pinned by construction: the two
        // entry points take `&str` and slices, and a mode that references a
        // template which does not exist still validates — resolving it is the
        // caller's job, and would be I/O.
        let mut mode = valid();
        mode.summary_template_id = "a_template_that_does_not_exist".into();
        assert_eq!(validate_mode(&mode, &[], &[]), Ok(()));
    }

    #[test]
    fn an_oversized_file_is_refused_before_it_is_parsed() {
        let huge = "x".repeat(MAX_MODE_BYTES + 1);
        assert_eq!(
            parse_mode_json(&huge),
            Err(ModeError::TooLarge {
                bytes: MAX_MODE_BYTES + 1
            })
        );
    }

    #[test]
    fn a_file_at_the_cap_is_still_parsed() {
        // Refused for being unreadable, not for its size — the boundary is
        // inclusive and this proves which error came back.
        let at_cap = "x".repeat(MAX_MODE_BYTES);
        assert!(matches!(
            parse_mode_json(&at_cap),
            Err(ModeError::NotJson(_))
        ));
    }

    #[test]
    fn a_missing_field_is_a_parse_error_not_a_silent_default() {
        // `Mode` deliberately does not derive `serde(default)`: a mode that did
        // not say what it allows must not be given permissions by default.
        let json = r#"{"id":"x","name":"X","purpose":"p","userRole":"u","counterpartRole":"c","voice":"v","summaryTemplateId":"standard_meeting"}"#;
        assert!(matches!(parse_mode_json(json), Err(ModeError::NotJson(_))));
    }

    #[test]
    fn ids_are_constrained() {
        for bad in ["", "x", "Board", "board review", "board-review", "böard"] {
            let mut mode = valid();
            mode.id = bad.into();
            assert_eq!(
                validate_mode(&mode, &[], &[]),
                Err(ModeError::BadId(bad.to_string())),
                "{bad:?} should not be a valid id"
            );
        }
        for good in ["ab", "board_review", "mode_2"] {
            let mut mode = valid();
            mode.id = good.into();
            assert_eq!(validate_mode(&mode, &[], &[]), Ok(()), "{good:?}");
        }
    }

    #[test]
    fn a_collision_is_refused_on_either_the_id_or_the_name() {
        let existing_ids = vec!["board_review".to_string()];
        assert_eq!(
            validate_mode(&valid(), &existing_ids, &[]),
            Err(ModeError::IdCollision("board_review".into()))
        );
        // Case and surrounding space do not make two names distinguishable in a
        // panel chip.
        let existing_names = vec!["  BOARD REVIEW ".to_string()];
        assert_eq!(
            validate_mode(&valid(), &[], &existing_names),
            Err(ModeError::NameCollision("Board review".into()))
        );
    }

    #[test]
    fn a_mode_must_offer_something_and_must_read_the_conversation() {
        let mut no_actions = valid();
        no_actions.live.allowed_actions.clear();
        assert_eq!(
            validate_mode(&no_actions, &[], &[]),
            Err(ModeError::NoActions)
        );

        let mut no_sources = valid();
        no_sources.allowed_sources.clear();
        assert_eq!(
            validate_mode(&no_sources, &[], &[]),
            Err(ModeError::NoSources)
        );

        let mut no_transcript = valid();
        no_transcript.allowed_sources = vec![AllowedSource::Knowledge];
        assert_eq!(
            validate_mode(&no_transcript, &[], &[]),
            Err(ModeError::TranscriptNotAllowed)
        );
    }

    /// ADR-0038's reject list, at the one point a user could introduce it.
    #[test]
    fn a_mode_that_announces_itself_as_candidate_side_help_is_refused() {
        for (field, value) in [
            ("id", "interview_prep"),
            ("name", "Exam helper"),
            ("purpose", "Help me pass a coding challenge."),
            ("user_role", "the candidate side"),
        ] {
            let mut mode = valid();
            match field {
                "id" => mode.id = value.into(),
                "name" => mode.name = value.into(),
                "purpose" => mode.purpose = value.into(),
                _ => mode.user_role = value.into(),
            }
            assert!(
                matches!(
                    validate_mode(&mode, &[], &[]),
                    Err(ModeError::RejectedCategory(_))
                ),
                "a mode whose {field} is {value:?} should be refused"
            );
        }
    }

    #[test]
    fn the_interviewers_own_mode_is_not_caught_by_the_reject_list() {
        // The false positive that would matter: the legitimate interviewer-side
        // mode must still validate.
        let mut interviewer = valid();
        interviewer.id = "my_interviews".into();
        interviewer.name = "Interviews I run".into();
        interviewer.purpose = "An interview I am conducting with a candidate.".into();
        interviewer.user_role = "the interviewer".into();
        interviewer.counterpart_role = "the candidate".into();
        assert_eq!(validate_mode(&interviewer, &[], &[]), Ok(()));
    }

    #[test]
    fn every_error_renders_a_sentence_that_says_what_to_do() {
        let errors = [
            ModeError::TooLarge { bytes: 99_999 },
            ModeError::NotJson("expected `,`".into()),
            ModeError::EmptyField("voice"),
            ModeError::BadId("Board".into()),
            ModeError::IdCollision("general".into()),
            ModeError::NameCollision("General".into()),
            ModeError::NoActions,
            ModeError::NoSources,
            ModeError::TranscriptNotAllowed,
            ModeError::RejectedCategory("exam".into()),
        ];
        for error in errors {
            let text = error.to_string();
            assert!(text.len() > 30, "{error:?} renders too tersely: {text}");
            assert!(
                text.ends_with('.') || text.ends_with('?'),
                "{error:?} is not a sentence: {text}"
            );
        }
    }

    #[test]
    fn preview_parses_and_validates_in_one_step() {
        let mode = valid();
        let json = serde_json::to_string(&mode).expect("serialises");
        assert_eq!(preview_mode(&json, &[], &[]), Ok(mode));
    }
}
