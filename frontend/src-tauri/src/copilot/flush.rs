//! Writing pins into the meeting they were made during (BACKLOG **I3c**,
//! ADR-0046).
//!
//! Runs once, from `api_save_transcript`, after the meeting and its transcript
//! rows are committed — the first moment all three things ADR-0041 found
//! missing exist at once: a `meetings` row to attach to, real `transcripts` ids
//! for the citation to resolve against, and a repository method
//! (`append_block`) that appends instead of replacing.
//!
//! ## One rule above all the others
//!
//! **A pin must never cost the user their recording.** The save has already
//! committed by the time this runs, and every failure here — an id that does
//! not resolve, a database error, a concurrent edit — is reported as a count
//! and swallowed. A feature for keeping notes that could fail a save would be
//! a worse bargain than not having it.
//!
//! ## What "resolve" means, and why unresolved pins are not guessed at
//!
//! A pin cites `sequence_id`s. `save_transcript` returns the map from those to
//! the row ids it minted. A pin is written against the FIRST of its cited ids
//! that appears in the map — first because evidence is held in audio order, so
//! the first is where the claim starts. A pin whose ids are all absent is
//! **dropped and counted**, never attached to a nearby segment: a citation
//! under the wrong timestamp is worse than no citation, and it is exactly what
//! this epic's grounding rule exists to prevent.

use crate::context::AuthContext;
use crate::copilot::pin::{self, Pin};
use crate::database::repositories::summary_draft::SummariesRepository;
use crate::summary::draft::{BlockProvenance, BlockStatus, BlockType, DraftBlock};
use serde::Serialize;
use sqlx::SqlitePool;
use std::collections::HashMap;
use tracing::{info, warn};
use uuid::Uuid;

/// What the flush did, for the renderer and the log. Counts only — never text.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FlushReport {
    /// Pins written as blocks.
    pub written: usize,
    /// Pins whose cited segments are not among the saved ones. The commonest
    /// cause is a recovery or import save, which mints segments with no live
    /// `sequence_id`.
    pub unresolved: usize,
    /// Pins the repository refused or could not store.
    pub failed: usize,
}

impl FlushReport {
    /// Did every pin make it? Used by the renderer to decide whether to say
    /// anything at all.
    pub fn is_complete(&self) -> bool {
        self.unresolved == 0 && self.failed == 0
    }
}

/// Turn one pin into the block it becomes.
///
/// The text is the claim as the user read it; `append_block` forces the status
/// and the provenance, so this does not get to decide either. The label names
/// the mode and the moment, because a block in a summary a week later has to
/// say where it came from without the panel next to it.
fn block_for(pin: &Pin, source_chunk_id: String) -> DraftBlock {
    DraftBlock {
        id: Uuid::new_v4().to_string(),
        block_type: BlockType::Bullet,
        content: format!("{} — {} · {}", pin.text, pin.mode_name, pin.timestamp),
        source_chunk_id,
        status: BlockStatus::Draft,
        original_content: None,
        provenance: BlockProvenance::Pinned,
    }
}

/// Write every pin of the finished session into `meeting_id`.
///
/// Drains the buffer whatever happens: the pins belong to this meeting, and one
/// left behind would be written into the next meeting the user records.
pub async fn flush_pins(
    pool: &SqlitePool,
    ctx: &AuthContext,
    meeting_id: &str,
    sequence_ids: &HashMap<u64, String>,
) -> FlushReport {
    let pins = pin::drain(ctx);
    if pins.is_empty() {
        return FlushReport::default();
    }

    let mut report = FlushReport::default();
    for pin in &pins {
        let Some(source_chunk_id) = pin
            .evidence
            .iter()
            .find_map(|sequence_id| sequence_ids.get(sequence_id).cloned())
        else {
            report.unresolved += 1;
            continue;
        };

        match SummariesRepository::append_block(
            pool,
            ctx,
            meeting_id,
            &block_for(pin, source_chunk_id),
        )
        .await
        {
            Ok(true) => report.written += 1,
            Ok(false) => {
                report.failed += 1;
                warn!(
                    meeting_id = %meeting_id,
                    "pin flush: meeting not found in this workspace"
                );
            }
            Err(e) => {
                report.failed += 1;
                // Content-free by construction: `SummaryDraftError` carries ids,
                // counts and reason codes only (CLAUDE.md §0.6).
                warn!(
                    meeting_id = %meeting_id,
                    error = %e,
                    "pin flush: a pinned block could not be stored"
                );
            }
        }
    }

    info!(
        meeting_id = %meeting_id,
        pins = pins.len(),
        written = report.written,
        unresolved = report.unresolved,
        failed = report.failed,
        "pin flush complete"
    );
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modes::LiveAction;

    fn pin(id: &str, evidence: Vec<u64>) -> Pin {
        Pin {
            id: id.to_string(),
            text: "Budget was approved.".to_string(),
            evidence,
            timestamp: "00:14".to_string(),
            action: LiveAction::Recap,
            mode_name: "General".to_string(),
        }
    }

    /// Evidence is held in audio order, so the FIRST resolvable id is where the
    /// claim starts. Taking any other would move the citation later than the
    /// thing it cites.
    #[test]
    fn the_first_resolvable_citation_is_the_one_used() {
        let mut map = HashMap::new();
        map.insert(7, "transcript-b".to_string());
        map.insert(4, "transcript-a".to_string());

        let p = pin("p1", vec![4, 7]);
        let chosen = p
            .evidence
            .iter()
            .find_map(|s| map.get(s).cloned())
            .expect("resolves");
        assert_eq!(chosen, "transcript-a");
    }

    /// A pin whose first id is missing still resolves through a later one — the
    /// renderer may cite a segment the save dropped.
    #[test]
    fn a_missing_first_id_falls_through_to_the_next() {
        let mut map = HashMap::new();
        map.insert(9, "transcript-c".to_string());

        let p = pin("p1", vec![4, 9]);
        let chosen = p.evidence.iter().find_map(|s| map.get(s).cloned());
        assert_eq!(chosen, Some("transcript-c".to_string()));
    }

    /// The property that matters most: nothing nearby is substituted. An
    /// unresolvable pin produces no block at all.
    #[test]
    fn an_unresolvable_pin_is_not_attached_to_a_neighbour() {
        let mut map = HashMap::new();
        map.insert(1, "transcript-a".to_string());
        map.insert(2, "transcript-b".to_string());

        let p = pin("p1", vec![99]);
        assert!(p
            .evidence
            .iter()
            .find_map(|s| map.get(s).cloned())
            .is_none());
    }

    #[test]
    fn a_block_carries_the_pinned_provenance_and_draft_status() {
        let b = block_for(&pin("p1", vec![4]), "transcript-a".to_string());
        assert_eq!(b.provenance, BlockProvenance::Pinned);
        assert_eq!(b.status, BlockStatus::Draft);
        assert_eq!(b.source_chunk_id, "transcript-a");
        assert!(b.content.contains("Budget was approved."));
        assert!(b.content.contains("00:14"));
        assert!(b.original_content.is_none());
    }

    #[test]
    fn a_report_is_complete_only_when_nothing_was_lost() {
        assert!(FlushReport {
            written: 3,
            unresolved: 0,
            failed: 0
        }
        .is_complete());
        assert!(!FlushReport {
            written: 3,
            unresolved: 1,
            failed: 0
        }
        .is_complete());
        assert!(!FlushReport {
            written: 0,
            unresolved: 0,
            failed: 2
        }
        .is_complete());
    }
}
