//! Converting the engine's segments into the shape `speaker_turns` stores.
//!
//! This is a pure function over plain numbers, deliberately: it is the one part
//! of the helper that can be tested without a model, and it is where the two
//! representations actually disagree.
//!
//! ADR-0035 decision 6 spells the conversion out, because "the result maps onto
//! the schema" was an overclaim in an earlier draft. The SHAPE matches — one
//! timed turn per row, which ADR-0034 decided independently — but nothing else
//! does:
//!
//! | sherpa | `speaker_turns` |
//! |---|---|
//! | `start`/`end`: `f32` **seconds** | `start_ms`/`end_ms`: `INTEGER` **milliseconds** |
//! | `speaker`: `i32`, **0-based** | `speaker_label`: `TEXT`, `Speaker 1`… **1-based** |
//! | (not reported) | `confidence`: nullable |
//!
//! Copying the floats straight through would place every turn ~1000× too early;
//! copying the index straight through would write a bare `0` where the report
//! renders a speaker's name.

use anyhow::{bail, Result};
use serde::Serialize;

/// One turn, in the units and labelling the database stores.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SpeakerTurn {
    pub start_ms: i64,
    pub end_ms: i64,
    /// Anonymous only — `Speaker 1`, `Speaker 2`. Never a person's name:
    /// attaching a real identity is a manual human action, and keeping the
    /// purpose away from "unique identification" is what keeps this outside
    /// biometric special-category processing (ADR-0034 decision 6).
    pub speaker_label: String,
    /// Always `None`. The engine reports no per-turn confidence, and NULL means
    /// "not reported", which is not the same as "low".
    pub confidence: Option<f64>,
}

/// Convert `(start_seconds, end_seconds, speaker_index)` triples.
///
/// Fails rather than guessing on input the engine should never produce, because
/// a silently mangled turn is worse than a failed run: it would be stored, shown
/// as fact, and cited as evidence.
pub fn to_speaker_turns(segments: &[(f32, f32, i32)]) -> Result<Vec<SpeakerTurn>> {
    let mut out = Vec::with_capacity(segments.len());
    for (start, end, speaker) in segments.iter().copied() {
        if !start.is_finite() || !end.is_finite() {
            bail!("engine returned a non-finite time ({start}, {end})");
        }
        if start < 0.0 {
            bail!("engine returned a negative start time ({start})");
        }
        if speaker < 0 {
            // `Speaker {speaker + 1}` would render "Speaker 0" or worse, and the
            // labelling is 1-based by contract.
            bail!("engine returned a negative speaker index ({speaker})");
        }

        let start_ms = secs_to_ms(start);
        let end_ms = secs_to_ms(end);
        // A turn that rounds away to nothing is dropped, not stored: the schema
        // requires end > start, and a zero-length turn describes no audio.
        if end_ms <= start_ms {
            continue;
        }

        out.push(SpeakerTurn {
            start_ms,
            end_ms,
            speaker_label: format!("Speaker {}", speaker + 1),
            confidence: None,
        });
    }
    out.sort_by_key(|t| (t.start_ms, t.end_ms, t.speaker_label.clone()));
    Ok(out)
}

fn secs_to_ms(secs: f32) -> i64 {
    (f64::from(secs) * 1000.0).round() as i64
}

/// Render turns as RTTM, so the helper's own output can be scored directly by
/// `eval-harness der` without a conversion step in between.
///
/// Rejects a `file_id` containing whitespace. RTTM is whitespace-separated, so
/// `meeting 1` would emit an extra field and every later column would shift —
/// the parser would read `meeting` as the file, the wrong times, and `<NA>` as
/// the speaker. Rejected rather than mangled like the speaker label is, because
/// the id is how a reference and a hypothesis are matched to the same recording
/// (`ensure_same_recording`); silently rewriting it would break that pairing.
pub fn to_rttm(file_id: &str, turns: &[SpeakerTurn]) -> Result<String> {
    if file_id.is_empty() || file_id.chars().any(char::is_whitespace) {
        bail!("RTTM file id must be a single non-empty token without whitespace, got {file_id:?}");
    }
    let mut out = String::new();
    for t in turns {
        out.push_str(&format!(
            "SPEAKER {} 1 {:.3} {:.3} <NA> <NA> {} <NA> <NA>\n",
            file_id,
            t.start_ms as f64 / 1000.0,
            (t.end_ms - t.start_ms) as f64 / 1000.0,
            // RTTM is whitespace-separated, so a label with a space would split
            // into two fields and the parser would read the wrong column.
            t.speaker_label.replace(' ', "_"),
        ));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seconds_become_milliseconds_and_speakers_become_one_based_labels() {
        let turns = to_speaker_turns(&[(0.0, 9.0, 0), (9.0, 20.5, 1)]).expect("converted");
        assert_eq!(turns[0].start_ms, 0);
        assert_eq!(turns[0].end_ms, 9000);
        assert_eq!(turns[0].speaker_label, "Speaker 1");
        assert_eq!(turns[1].start_ms, 9000);
        assert_eq!(turns[1].end_ms, 20500);
        assert_eq!(turns[1].speaker_label, "Speaker 2");
        // Never invented: the engine reports no confidence.
        assert!(turns.iter().all(|t| t.confidence.is_none()));
    }

    /// The failure this conversion exists to prevent: storing the float
    /// straight through would put every turn a thousand times too early.
    #[test]
    fn a_turn_is_not_stored_a_thousand_times_too_early() {
        let turns = to_speaker_turns(&[(12.345, 15.0, 0)]).expect("converted");
        assert_eq!(turns[0].start_ms, 12345, "12.345 s is 12345 ms, not 12 ms");
    }

    #[test]
    fn sub_millisecond_turns_are_dropped_not_stored_inverted() {
        // Rounds to the same millisecond -> no audio described.
        let turns = to_speaker_turns(&[(1.0001, 1.0002, 0), (2.0, 3.0, 0)]).expect("converted");
        assert_eq!(turns.len(), 1);
        assert_eq!(turns[0].start_ms, 2000);
    }

    #[test]
    fn turns_come_back_in_time_order() {
        let turns =
            to_speaker_turns(&[(10.0, 12.0, 1), (0.0, 5.0, 0), (5.0, 10.0, 1)]).expect("converted");
        let starts: Vec<i64> = turns.iter().map(|t| t.start_ms).collect();
        assert_eq!(starts, vec![0, 5000, 10000]);
    }

    /// Overlapping turns are legitimate — people talk over each other — and must
    /// survive, because refusing to represent overlap is refusing to represent
    /// what happened.
    #[test]
    fn overlapping_turns_are_kept() {
        let turns = to_speaker_turns(&[(0.0, 10.0, 0), (8.0, 15.0, 1)]).expect("converted");
        assert_eq!(turns.len(), 2);
        assert!(turns[1].start_ms < turns[0].end_ms);
    }

    #[test]
    fn impossible_engine_output_is_refused_rather_than_mangled() {
        assert!(to_speaker_turns(&[(f32::NAN, 1.0, 0)]).is_err());
        assert!(to_speaker_turns(&[(0.0, f32::INFINITY, 0)]).is_err());
        assert!(to_speaker_turns(&[(-1.0, 1.0, 0)]).is_err());
        // Would have rendered "Speaker 0", which the 1-based contract forbids.
        assert!(to_speaker_turns(&[(0.0, 1.0, -1)]).is_err());
    }

    /// A file id with a space would add a field and shift every later column,
    /// so our own parser would read the wrong times and `<NA>` as the speaker --
    /// output that claims to be directly scoreable but is not.
    #[test]
    fn an_rttm_file_id_with_whitespace_is_refused() {
        let turns = to_speaker_turns(&[(0.0, 9.0, 0)]).expect("converted");
        assert!(to_rttm("meeting 1", &turns).is_err());
        assert!(to_rttm("", &turns).is_err());
        assert!(to_rttm("meeting	1", &turns).is_err());
        assert!(to_rttm("meeting_1", &turns).is_ok());
    }

    /// RTTM is whitespace-separated, so `Speaker 1` must not ship as two fields.
    #[test]
    fn rttm_labels_carry_no_spaces() {
        let turns = to_speaker_turns(&[(0.0, 9.0, 0)]).expect("converted");
        let rttm = to_rttm("meeting1", &turns).expect("rendered");
        let fields: Vec<&str> = rttm.split_whitespace().collect();
        assert_eq!(fields.len(), 10, "one RTTM record is 10 fields: {rttm}");
        assert_eq!(fields[7], "Speaker_1");
        assert_eq!(fields[3], "0.000");
        assert_eq!(fields[4], "9.000");
    }
}
