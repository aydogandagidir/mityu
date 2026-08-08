//! The rule that makes an answer trustworthy: a claim survives only if the
//! evidence it cites was actually retrieved.
//!
//! This is the load-bearing safety property of "Ask This Meeting" (Product
//! Intelligence slice 3): *"unsupported claims are refused/flagged"*. It is a
//! pure function over data so it can be tested exhaustively without a model.
//!
//! ## Why this is stricter than the summary path
//!
//! `summary::structured::resolve_source_chunk_id` REPAIRS a bad id by matching
//! the block's text against the segments it sent — correct there, because a
//! summariser paraphrases a segment it was shown, so a near-miss id is a
//! formatting slip rather than an invention.
//!
//! An answer is different. A citation to a passage that was never retrieved is
//! not a slip; it is the model asserting evidence that does not exist in the
//! window it was given. Repairing that would attach a real timestamp to an
//! invented claim and make a hallucination *look* sourced — the worst possible
//! failure for a surface whose entire value is verifiability. So here an
//! unknown id is **dropped**, never repaired.
//!
//! ## What a model is trusted with
//!
//! Only the claim text and the id. Timing is looked up from the retrieved
//! passage, so a model cannot invent a timestamp even for a claim it grounds
//! correctly.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::api::api::MeetingEvidencePassage;

/// A claim as returned by the model, before grounding.
#[derive(Debug, Clone, Deserialize)]
pub struct RawClaim {
    pub text: String,
    pub source_chunk_id: String,
}

/// A claim that survived grounding, carrying evidence timing taken from the
/// retrieved passage rather than from the model.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GroundedClaim {
    pub text: String,
    #[serde(rename = "sourceChunkId")]
    pub source_chunk_id: String,
    pub timestamp: String,
    #[serde(rename = "audioStartTime")]
    pub audio_start_time: Option<f64>,
}

/// Why a claim did not survive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DropReason {
    /// Cited a `source_chunk_id` that was not in the retrieved window.
    UngroundedCitation,
    /// Empty or whitespace-only claim text.
    BlankText,
    /// The same claim text and citation had already been kept.
    Duplicate,
}

/// One rejected claim, kept so the UI can show that filtering happened rather
/// than silently presenting a shorter answer.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DroppedClaim {
    pub text: String,
    #[serde(rename = "sourceChunkId")]
    pub source_chunk_id: String,
    pub reason: DropReason,
}

/// Result of grounding a model's claims against the retrieved window.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GroundingOutcome {
    pub kept: Vec<GroundedClaim>,
    pub dropped: Vec<DroppedClaim>,
}

impl GroundingOutcome {
    /// True when the model produced claims but none survived.
    ///
    /// The caller must refuse in this case rather than render an empty answer:
    /// "here is nothing" reads as "there is nothing in the meeting", which is a
    /// different and unsupported statement.
    pub fn is_total_rejection(&self) -> bool {
        self.kept.is_empty() && !self.dropped.is_empty()
    }
}

/// Keep only the claims whose citation points at a passage that was retrieved.
///
/// Order is preserved: the model's ordering carries its reasoning, and reordering
/// would change how the answer reads.
pub fn ground_claims(
    claims: &[RawClaim],
    retrieved: &[MeetingEvidencePassage],
) -> GroundingOutcome {
    let index: HashMap<&str, &MeetingEvidencePassage> = retrieved
        .iter()
        .map(|p| (p.source_chunk_id.as_str(), p))
        .collect();

    let mut kept: Vec<GroundedClaim> = Vec::new();
    let mut dropped: Vec<DroppedClaim> = Vec::new();
    let mut seen: Vec<(String, String)> = Vec::new();

    for claim in claims {
        let text = claim.text.trim();
        let id = claim.source_chunk_id.trim();

        if text.is_empty() {
            dropped.push(DroppedClaim {
                text: claim.text.clone(),
                source_chunk_id: id.to_string(),
                reason: DropReason::BlankText,
            });
            continue;
        }

        let Some(passage) = index.get(id) else {
            dropped.push(DroppedClaim {
                text: text.to_string(),
                source_chunk_id: id.to_string(),
                reason: DropReason::UngroundedCitation,
            });
            continue;
        };

        let key = (text.to_string(), id.to_string());
        if seen.contains(&key) {
            dropped.push(DroppedClaim {
                text: text.to_string(),
                source_chunk_id: id.to_string(),
                reason: DropReason::Duplicate,
            });
            continue;
        }
        seen.push(key);

        kept.push(GroundedClaim {
            text: text.to_string(),
            source_chunk_id: passage.source_chunk_id.clone(),
            // Taken from the retrieved passage, never from the model.
            timestamp: passage.timestamp.clone(),
            audio_start_time: passage.audio_start_time,
        });
    }

    GroundingOutcome { kept, dropped }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn passage(id: &str, ts: &str, start: Option<f64>) -> MeetingEvidencePassage {
        MeetingEvidencePassage {
            source_chunk_id: id.to_string(),
            timestamp: ts.to_string(),
            audio_start_time: start,
            text: format!("passage {id}"),
        }
    }

    fn claim(text: &str, id: &str) -> RawClaim {
        RawClaim {
            text: text.to_string(),
            source_chunk_id: id.to_string(),
        }
    }

    /// The property this module exists for: a citation to something that was
    /// never retrieved is an invention and must not reach the user.
    #[test]
    fn a_citation_outside_the_retrieved_window_is_dropped() {
        let retrieved = vec![passage("c1", "00:01", Some(1.0))];
        let out = ground_claims(&[claim("The contract renews in March.", "c99")], &retrieved);

        assert!(out.kept.is_empty());
        assert_eq!(out.dropped.len(), 1);
        assert_eq!(out.dropped[0].reason, DropReason::UngroundedCitation);
        assert!(out.is_total_rejection());
    }

    /// It is never repaired to a nearby passage, even when the text matches one
    /// almost exactly -- that is precisely how a hallucination would acquire a
    /// real-looking timestamp.
    #[test]
    fn a_bad_citation_is_not_repaired_by_text_similarity() {
        let mut only = passage("c1", "00:01", Some(1.0));
        only.text = "The vendor contract renews in March.".to_string();
        let out = ground_claims(
            &[claim("The vendor contract renews in March.", "c-typo")],
            &[only],
        );

        assert!(out.kept.is_empty(), "text overlap must not rescue a bad id");
        assert_eq!(out.dropped[0].reason, DropReason::UngroundedCitation);
    }

    /// Timing comes from our retrieval, so a model cannot state when something
    /// was said.
    #[test]
    fn timing_is_taken_from_the_passage_not_the_model() {
        let retrieved = vec![passage("c1", "00:42", Some(42.5))];
        let out = ground_claims(&[claim("Budget was approved.", "c1")], &retrieved);

        assert_eq!(out.kept.len(), 1);
        assert_eq!(out.kept[0].timestamp, "00:42");
        assert_eq!(out.kept[0].audio_start_time, Some(42.5));
    }

    #[test]
    fn grounded_claims_survive_and_keep_model_order() {
        let retrieved = vec![
            passage("c1", "00:01", Some(1.0)),
            passage("c2", "00:09", Some(9.0)),
        ];
        let out = ground_claims(
            &[claim("Second point.", "c2"), claim("First point.", "c1")],
            &retrieved,
        );

        assert_eq!(out.kept.len(), 2);
        assert_eq!(out.kept[0].text, "Second point.");
        assert_eq!(out.kept[1].text, "First point.");
        assert!(out.dropped.is_empty());
        assert!(!out.is_total_rejection());
    }

    #[test]
    fn blank_claims_are_dropped() {
        let retrieved = vec![passage("c1", "00:01", None)];
        let out = ground_claims(&[claim("   ", "c1")], &retrieved);

        assert!(out.kept.is_empty());
        assert_eq!(out.dropped[0].reason, DropReason::BlankText);
    }

    #[test]
    fn repeated_claims_are_kept_once() {
        let retrieved = vec![passage("c1", "00:01", None)];
        let out = ground_claims(
            &[claim("Same thing.", "c1"), claim("Same thing.", "c1")],
            &retrieved,
        );

        assert_eq!(out.kept.len(), 1);
        assert_eq!(out.dropped.len(), 1);
        assert_eq!(out.dropped[0].reason, DropReason::Duplicate);
    }

    /// The same sentence supported by two different passages is not a duplicate
    /// -- it is corroborated, and both citations are worth showing.
    #[test]
    fn the_same_text_from_two_passages_is_not_a_duplicate() {
        let retrieved = vec![passage("c1", "00:01", None), passage("c2", "00:02", None)];
        let out = ground_claims(
            &[
                claim("Agreed on Friday.", "c1"),
                claim("Agreed on Friday.", "c2"),
            ],
            &retrieved,
        );

        assert_eq!(out.kept.len(), 2);
        assert!(out.dropped.is_empty());
    }

    /// A partly-grounded answer keeps what is supported and reports the rest,
    /// so the UI can say that filtering happened.
    #[test]
    fn a_partly_grounded_answer_keeps_only_the_supported_half() {
        let retrieved = vec![passage("c1", "00:01", None)];
        let out = ground_claims(
            &[claim("Supported.", "c1"), claim("Invented.", "nope")],
            &retrieved,
        );

        assert_eq!(out.kept.len(), 1);
        assert_eq!(out.kept[0].text, "Supported.");
        assert_eq!(out.dropped.len(), 1);
        assert!(!out.is_total_rejection());
    }

    /// No claims at all is not a rejection -- there was nothing to reject. The
    /// caller distinguishes this from "everything was rejected".
    #[test]
    fn an_empty_claim_list_is_not_a_rejection() {
        let out = ground_claims(&[], &[passage("c1", "00:01", None)]);
        assert!(out.kept.is_empty());
        assert!(!out.is_total_rejection());
    }

    /// Retrieval returning nothing means every citation is ungrounded by
    /// definition; the caller should not have called a model at all.
    #[test]
    fn nothing_is_grounded_against_an_empty_window() {
        let out = ground_claims(&[claim("Anything.", "c1")], &[]);
        assert!(out.kept.is_empty());
        assert!(out.is_total_rejection());
    }

    #[test]
    fn surrounding_whitespace_does_not_break_a_citation() {
        let retrieved = vec![passage("c1", "00:01", None)];
        let out = ground_claims(&[claim("  Trimmed.  ", "  c1  ")], &retrieved);

        assert_eq!(out.kept.len(), 1);
        assert_eq!(out.kept[0].text, "Trimmed.");
        assert_eq!(out.kept[0].source_chunk_id, "c1");
    }
}
