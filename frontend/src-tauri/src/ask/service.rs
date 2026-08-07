//! Orchestration for "Ask This Meeting": retrieve, answer from what was
//! retrieved, ground every claim, refuse rather than guess.
//!
//! The ordering here is the product guarantee, not an implementation detail.
//! Retrieval happens first and decides whether a model is consulted at all; the
//! model only ever sees passages from the meeting being asked about; and its
//! output is filtered through [`ground_claims`] before anything reaches a user.

use serde::Serialize;
use sqlx::SqlitePool;
use std::path::PathBuf;

use crate::api::api::MeetingEvidencePassage;
use crate::ask::grounding::{ground_claims, DroppedClaim, GroundedClaim, RawClaim};
use crate::context::AuthContext;
use crate::database::repositories::transcript::TranscriptsRepository;
use crate::summary::service::SummaryService;
use crate::summary::structured::extract_first_balanced_object;

/// How many passages an answer may be drawn from.
///
/// Small on purpose: the built-in local model is the one that must cope, and a
/// tight window also keeps every claim close to checkable evidence.
const RETRIEVAL_LIMIT: i64 = 8;

/// What the user is shown. There is no "empty answer" variant by design — an
/// answer with no claims is a refusal, and saying so is different from
/// rendering nothing.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum AskOutcome {
    /// Retrieval found no passage for this question. **No model was called.**
    ///
    /// This is not a failure: a transcript that does not discuss something is
    /// the honest answer, and answering anyway would mean answering from the
    /// model's prior knowledge rather than from this meeting.
    NoEvidence,
    /// At least one claim was supported by a retrieved passage.
    Answered {
        claims: Vec<GroundedClaim>,
        /// Claims the model produced that did not survive grounding. Surfaced,
        /// not hidden: a silently shortened answer looks complete.
        dropped: Vec<DroppedClaim>,
        #[serde(rename = "passagesConsidered")]
        passages_considered: usize,
    },
    /// The model produced claims and none of them were grounded.
    Refused {
        dropped: Vec<DroppedClaim>,
        #[serde(rename = "passagesConsidered")]
        passages_considered: usize,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum AskError {
    #[error("question is empty")]
    EmptyQuestion,
    #[error("retrieval failed: {0}")]
    Retrieval(String),
    #[error("model call failed: {0}")]
    Provider(String),
    #[error("model returned no usable JSON object")]
    Unparsable,
}

/// The contract the model is held to. Deliberately narrow: text plus an id from
/// a list it was given. Everything else about a claim — when it was said, which
/// meeting it belongs to — is supplied by us and cannot be influenced by it.
fn system_prompt() -> String {
    "You answer questions about ONE meeting using ONLY the numbered passages supplied by the user.\n\
     \n\
     Reply with a single JSON object and nothing else:\n\
     {\"claims\":[{\"text\":\"one sentence\",\"source_chunk_id\":\"the id of the passage that supports it\"}]}\n\
     \n\
     Rules:\n\
     - Every claim MUST carry the source_chunk_id of a supplied passage, copied exactly.\n\
     - Never invent an id. Never cite a passage that was not supplied.\n\
     - Use ONLY the passages. Do not add outside knowledge or inference.\n\
     - If the passages do not answer the question, reply {\"claims\":[]}.\n\
     - Do not include timestamps, speaker names, or commentary in the text."
        .to_string()
}

fn user_prompt(question: &str, passages: &[MeetingEvidencePassage]) -> String {
    let rendered: Vec<serde_json::Value> = passages
        .iter()
        .map(|p| {
            serde_json::json!({
                "source_chunk_id": p.source_chunk_id,
                "text": p.text,
            })
        })
        .collect();

    format!(
        "Question: {question}\n\nPassages:\n{}",
        serde_json::to_string_pretty(&rendered).unwrap_or_else(|_| "[]".to_string())
    )
}

/// Parse the model's reply into raw claims.
///
/// Tolerant about wrapping prose (small models like to add a sentence) but not
/// about content: anything that is not a well-formed claim object is skipped
/// rather than guessed at.
fn parse_claims(raw: &str) -> Result<Vec<RawClaim>, AskError> {
    let object = extract_first_balanced_object(raw).ok_or(AskError::Unparsable)?;
    let value: serde_json::Value =
        serde_json::from_str(object).map_err(|_| AskError::Unparsable)?;
    let Some(items) = value.get("claims").and_then(|c| c.as_array()) else {
        return Err(AskError::Unparsable);
    };

    Ok(items
        .iter()
        .filter_map(|item| {
            let text = item.get("text")?.as_str()?.to_string();
            let source_chunk_id = item.get("source_chunk_id")?.as_str()?.to_string();
            Some(RawClaim {
                text,
                source_chunk_id,
            })
        })
        .collect())
}

/// Answer a question about one meeting.
///
/// Read-only: nothing is persisted and no summary or action item is created, so
/// an answer can never become approved content (`CLAUDE.md` §0.5).
#[allow(clippy::too_many_arguments)]
pub async fn ask_meeting(
    pool: &SqlitePool,
    ctx: &AuthContext,
    app_data_dir: Option<&PathBuf>,
    meeting_id: &str,
    question: &str,
    model_provider: &str,
    model_name: &str,
) -> Result<AskOutcome, AskError> {
    let question = question.trim();
    if question.is_empty() {
        return Err(AskError::EmptyQuestion);
    }

    let passages = TranscriptsRepository::retrieve_meeting_evidence(
        pool,
        ctx,
        meeting_id,
        question,
        RETRIEVAL_LIMIT,
    )
    .await
    .map_err(|e| AskError::Retrieval(e.to_string()))?;

    // The model is not consulted when there is nothing to answer from. This is
    // the difference between a retrieval-first answer and a chatbot that happens
    // to have seen a transcript.
    if passages.is_empty() {
        return Ok(AskOutcome::NoEvidence);
    }

    let provider = SummaryService::assemble_provider(pool, ctx, model_provider, model_name)
        .await
        .map_err(AskError::Provider)?;

    let raw = provider
        .call(
            app_data_dir,
            &system_prompt(),
            &user_prompt(question, &passages),
        )
        .await
        .map_err(AskError::Provider)?;

    let claims = parse_claims(&raw)?;
    let outcome = ground_claims(&claims, &passages);
    let passages_considered = passages.len();

    if outcome.is_total_rejection() {
        return Ok(AskOutcome::Refused {
            dropped: outcome.dropped,
            passages_considered,
        });
    }
    if outcome.kept.is_empty() {
        // The model declined (an empty claims array). Retrieval found passages
        // but they did not answer the question -- same user-facing meaning as
        // no evidence, and honest about it.
        return Ok(AskOutcome::NoEvidence);
    }

    Ok(AskOutcome::Answered {
        claims: outcome.kept,
        dropped: outcome.dropped,
        passages_considered,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn passage(id: &str, text: &str) -> MeetingEvidencePassage {
        MeetingEvidencePassage {
            source_chunk_id: id.to_string(),
            timestamp: "00:01".to_string(),
            audio_start_time: Some(1.0),
            text: text.to_string(),
        }
    }

    #[test]
    fn parses_a_clean_reply() {
        let claims =
            parse_claims(r#"{"claims":[{"text":"Budget approved.","source_chunk_id":"c1"}]}"#)
                .expect("parses");
        assert_eq!(claims.len(), 1);
        assert_eq!(claims[0].source_chunk_id, "c1");
    }

    /// Small local models often wrap JSON in a sentence; that must not lose the
    /// answer.
    #[test]
    fn tolerates_prose_around_the_json() {
        let claims = parse_claims(
            "Sure! Here is the answer:\n{\"claims\":[{\"text\":\"A.\",\"source_chunk_id\":\"c1\"}]}\nHope that helps.",
        )
        .expect("parses");
        assert_eq!(claims.len(), 1);
    }

    #[test]
    fn an_empty_claims_array_parses_to_no_claims() {
        assert!(parse_claims(r#"{"claims":[]}"#).expect("parses").is_empty());
    }

    /// A malformed entry is skipped rather than guessed at -- a claim without an
    /// id could never be grounded anyway.
    #[test]
    fn malformed_entries_are_skipped_not_invented() {
        let claims = parse_claims(
            r#"{"claims":[{"text":"No id here"},{"text":"Good","source_chunk_id":"c1"}]}"#,
        )
        .expect("parses");
        assert_eq!(claims.len(), 1);
        assert_eq!(claims[0].text, "Good");
    }

    #[test]
    fn a_reply_without_a_claims_key_is_unparsable() {
        assert!(matches!(
            parse_claims(r#"{"answer":"nope"}"#),
            Err(AskError::Unparsable)
        ));
        assert!(matches!(
            parse_claims("no json here"),
            Err(AskError::Unparsable)
        ));
    }

    /// The passages the model sees carry ids and text only -- no timestamps, so
    /// it cannot echo one back as if it were evidence.
    #[test]
    fn the_prompt_exposes_ids_and_text_but_not_timing() {
        let prompt = user_prompt("What was decided?", &[passage("c1", "We shipped it.")]);
        assert!(prompt.contains("c1"));
        assert!(prompt.contains("We shipped it."));
        assert!(prompt.contains("What was decided?"));
        assert!(
            !prompt.contains("00:01"),
            "timing must not be offered to the model"
        );
    }

    #[test]
    fn the_system_prompt_states_the_refusal_contract() {
        let p = system_prompt();
        assert!(p.contains("ONLY the numbered passages"));
        assert!(p.contains("Never invent an id"));
        assert!(p.contains(r#"{"claims":[]}"#));
    }

    /// End-to-end shape of the decision, without a model: retrieval → grounding
    /// → outcome. Pins that a fully ungrounded reply refuses instead of
    /// rendering an empty answer.
    #[test]
    fn a_fully_ungrounded_reply_refuses() {
        let passages = vec![passage("c1", "Real passage.")];
        let claims = parse_claims(r#"{"claims":[{"text":"Invented.","source_chunk_id":"ghost"}]}"#)
            .expect("parses");
        let grounded = ground_claims(&claims, &passages);

        assert!(grounded.is_total_rejection());
        assert!(grounded.kept.is_empty());
    }
}
