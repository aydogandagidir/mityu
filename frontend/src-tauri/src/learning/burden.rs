//! Correction burden (ADR-0024 §9) — how much of Mityu's writing the human had
//! to change.
//!
//! This is the learning system's objective function, and its whole point is that
//! it needs **no labelled data**: it is computed from rows the app already writes
//! when a person reviews a summary. That is what lets the learning system be
//! measured at all without the Phase-0 clip corpus.
//!
//! ## What it measures, and what it does NOT
//!
//! It measures **what the human did**, not what the model did. Those usually move
//! together, and when they don't, this number lies:
//!
//! - A user who gets tired and approves without reading properly produces a
//!   *falling* burden and a *worse* product. There is no way to tell the two
//!   apart from this side of the screen.
//! - The rules the miner learns are, by construction, the corrections the user
//!   makes most — so burden falling after a rule activates is partly the rule
//!   working and partly regression to the mean.
//! - There is no control group. A before/after around rule activation is a
//!   correlation, and calling it anything else would be a lie the user cannot
//!   check.
//!
//! So: report the number, show the comparison, and let the user draw the
//! conclusion. Any copy built on this that says "because" is claiming something
//! this module cannot support. See [`BurdenStats`] for the shape and the UI for
//! the wording actually shipped.
//!
//! ## The collapse
//!
//! Like the miner, this reads the LAST verdict per `(meeting_id, subject_id)` —
//! a block edited then approved is ONE reviewed item, not two, and its verdict is
//! the approval (which carries the same model→human delta the edit did). Restores
//! are excluded entirely: a restored block is back in draft, awaiting review, so
//! it has no verdict to score.

use crate::database::repositories::correction_event::{CorrectionAction, CorrectionEventRow};
use crate::text::normalized_word_distance;
use serde::Serialize;
use std::collections::HashMap;

/// How many reviewed items make up one comparison window.
///
/// Twenty is small enough that a real user reaches two windows in a handful of
/// meetings, and large enough that one unusual review does not swing the number.
/// It is a judgement, not a measurement — nobody has a corpus of real corrections
/// to tune it against yet.
pub const WINDOW: usize = 20;

/// The burden over some set of reviewed items.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct BurdenStats {
    /// How many items carried a verdict.
    pub reviewed: usize,
    /// How many the human took EXACTLY as written — the cleanest signal there is
    /// that the model got it right, and the one worth showing a person.
    pub accepted_as_written: usize,
    /// Mean per-item distance from what the model wrote to what the human kept,
    /// `0.0..=1.0`. A rejected item counts 1.0: nothing survived.
    pub mean_burden: f64,
}

impl BurdenStats {
    /// Fraction taken exactly as written, `0.0..=1.0`. `None` when nothing has
    /// been reviewed — a rate over zero items is not 0%, it is unknown, and the
    /// difference matters on a screen that claims to explain itself.
    pub fn accepted_rate(&self) -> Option<f64> {
        if self.reviewed == 0 {
            return None;
        }
        Some(self.accepted_as_written as f64 / self.reviewed as f64)
    }
}

/// Burden now versus burden before, when there is enough history to compare.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct BurdenTrend {
    /// Everything ever reviewed in this workspace.
    pub overall: BurdenStats,
    /// The most recent [`WINDOW`] items.
    pub recent: Option<BurdenStats>,
    /// The [`WINDOW`] before those.
    ///
    /// `None` until the workspace has reviewed `2 * WINDOW` items. The comparison
    /// simply does not appear before then, rather than appearing built out of
    /// three data points and pretending to mean something.
    pub earlier: Option<BurdenStats>,
}

/// The verdict on one reviewed item, reduced to a number.
fn burden_of(event: &CorrectionEventRow) -> Option<f64> {
    match event.action {
        // Nothing survived: the maximum a single item can cost.
        CorrectionAction::Reject => Some(1.0),
        CorrectionAction::Edit | CorrectionAction::Approve => {
            let original = event.original_text.as_deref().unwrap_or("");
            let final_text = event.final_text.as_deref().unwrap_or("");
            Some(normalized_word_distance(original, final_text))
        }
        // Not a verdict — the block went back to draft, awaiting review.
        CorrectionAction::Restore => None,
    }
}

/// Last verdict per `(meeting_id, subject_id)`, oldest first.
fn last_verdict_per_subject(events: &[CorrectionEventRow]) -> Vec<&CorrectionEventRow> {
    let mut latest: HashMap<(&str, &str), &CorrectionEventRow> = HashMap::new();
    for event in events {
        let key = (event.meeting_id.as_str(), event.subject_id.as_str());
        latest
            .entry(key)
            .and_modify(|existing| {
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

fn stats_of(burdens: &[f64]) -> BurdenStats {
    let reviewed = burdens.len();
    let accepted_as_written = burdens.iter().filter(|b| **b == 0.0).count();
    let mean_burden = if reviewed == 0 {
        0.0
    } else {
        burdens.iter().sum::<f64>() / reviewed as f64
    };
    BurdenStats {
        reviewed,
        accepted_as_written,
        mean_burden,
    }
}

/// Compute the trend over a correction log.
///
/// `events` may be in any order; the collapse sorts them.
pub fn compute(events: &[CorrectionEventRow]) -> BurdenTrend {
    let burdens: Vec<f64> = last_verdict_per_subject(events)
        .into_iter()
        .filter_map(burden_of)
        .collect();

    let overall = stats_of(&burdens);
    let (recent, earlier) = if burdens.len() >= 2 * WINDOW {
        let split = burdens.len() - WINDOW;
        (
            Some(stats_of(&burdens[split..])),
            Some(stats_of(&burdens[split - WINDOW..split])),
        )
    } else {
        (None, None)
    };

    BurdenTrend {
        overall,
        recent,
        earlier,
    }
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
            section_title: Some("Kararlar".to_string()),
            template_id: Some("daily_standup".to_string()),
            model: Some("llama3.2".to_string()),
            created_at: created_at.to_string(),
        }
    }

    fn approved_untouched(n: usize) -> CorrectionEventRow {
        event(
            &format!("e{n}"),
            &format!("m{n}"),
            "b1",
            CorrectionAction::Approve,
            Some("aynı metin"),
            Some("aynı metin"),
            &format!("2026-07-17T10:{n:02}:00Z"),
        )
    }

    /// The cleanest signal in the system: the model got it exactly right, so the
    /// human changed nothing, so the burden is zero.
    #[test]
    fn approving_untouched_text_is_zero_burden() {
        let stats = compute(&[approved_untouched(1)]).overall;
        assert_eq!(stats.reviewed, 1);
        assert_eq!(stats.accepted_as_written, 1);
        assert_eq!(stats.mean_burden, 0.0);
        assert_eq!(stats.accepted_rate(), Some(1.0));
    }

    #[test]
    fn a_reject_costs_the_maximum_because_nothing_survived() {
        let stats = compute(&[event(
            "e1",
            "m1",
            "b1",
            CorrectionAction::Reject,
            Some("bir şey"),
            None,
            "2026-07-17T10:00:00Z",
        )])
        .overall;
        assert_eq!(stats.mean_burden, 1.0);
        assert_eq!(stats.accepted_as_written, 0);
    }

    #[test]
    fn a_one_word_fix_in_four_costs_a_quarter() {
        let stats = compute(&[event(
            "e1",
            "m1",
            "b1",
            CorrectionAction::Edit,
            Some("3 aksiyon çıktı bugün"),
            Some("3 takip çıktı bugün"),
            "2026-07-17T10:00:00Z",
        )])
        .overall;
        assert_eq!(stats.reviewed, 1);
        assert!(
            (stats.mean_burden - 0.25).abs() < 1e-9,
            "got {}",
            stats.mean_burden
        );
        assert_eq!(
            stats.accepted_as_written, 0,
            "changed at all is not accepted as written",
        );
    }

    /// Edit-then-approve is ONE reviewed item, and the approval carries the same
    /// model→human delta the edit did. Counting both would double the reviewed
    /// count and halve the apparent burden.
    #[test]
    fn edit_then_approve_is_one_item_scored_on_the_approval() {
        let events = vec![
            event(
                "e1",
                "m1",
                "b1",
                CorrectionAction::Edit,
                Some("3 aksiyon çıktı bugün"),
                Some("3 takip çıktı bugün"),
                "2026-07-17T10:00:00Z",
            ),
            event(
                "e2",
                "m1",
                "b1",
                CorrectionAction::Approve,
                Some("3 aksiyon çıktı bugün"),
                Some("3 takip çıktı bugün"),
                "2026-07-17T10:01:00Z",
            ),
        ];
        let stats = compute(&events).overall;
        assert_eq!(stats.reviewed, 1);
        assert!((stats.mean_burden - 0.25).abs() < 1e-9);
    }

    /// A restored block is back in draft, awaiting review — it has no verdict, so
    /// it is not a reviewed item and must not be scored either way.
    #[test]
    fn a_restore_removes_the_item_from_the_count_entirely() {
        let events = vec![
            event(
                "e1",
                "m1",
                "b1",
                CorrectionAction::Reject,
                Some("bir şey"),
                None,
                "2026-07-17T10:00:00Z",
            ),
            event(
                "e2",
                "m1",
                "b1",
                CorrectionAction::Restore,
                Some("bir şey"),
                None,
                "2026-07-17T10:01:00Z",
            ),
        ];
        let stats = compute(&events).overall;
        assert_eq!(
            stats.reviewed, 0,
            "the reject was taken back and nothing replaced it",
        );
    }

    /// A rate over zero items is UNKNOWN, not 0% — and a screen that claims to
    /// explain itself must not say "0% accepted" to someone who has reviewed
    /// nothing.
    #[test]
    fn an_empty_log_has_no_rate_rather_than_a_zero_one() {
        let trend = compute(&[]);
        assert_eq!(trend.overall.reviewed, 0);
        assert_eq!(trend.overall.accepted_rate(), None);
        assert!(trend.recent.is_none());
        assert!(trend.earlier.is_none());
    }

    /// The comparison must not appear until there is enough to compare — better
    /// no number than one built from three data points.
    #[test]
    fn the_comparison_appears_only_at_two_full_windows() {
        let almost: Vec<CorrectionEventRow> = (1..2 * WINDOW).map(approved_untouched).collect();
        let trend = compute(&almost);
        assert_eq!(trend.overall.reviewed, 2 * WINDOW - 1);
        assert!(trend.recent.is_none(), "one item short");
        assert!(trend.earlier.is_none());

        let enough: Vec<CorrectionEventRow> = (1..=2 * WINDOW).map(approved_untouched).collect();
        let trend = compute(&enough);
        assert_eq!(trend.recent.expect("recent").reviewed, WINDOW);
        assert_eq!(trend.earlier.expect("earlier").reviewed, WINDOW);
    }

    /// The shape the product claim rests on: the recent window improving while the
    /// earlier one did not.
    #[test]
    fn the_windows_split_oldest_to_newest() {
        let mut events: Vec<CorrectionEventRow> = vec![];
        // The earlier WINDOW: every item rewritten.
        for n in 1..=WINDOW {
            events.push(event(
                &format!("old{n}"),
                &format!("mo{n}"),
                "b1",
                CorrectionAction::Reject,
                Some("bir şey"),
                None,
                &format!("2026-07-01T10:{n:02}:00Z"),
            ));
        }
        // The recent WINDOW: every item taken as written.
        for n in 1..=WINDOW {
            events.push(event(
                &format!("new{n}"),
                &format!("mn{n}"),
                "b1",
                CorrectionAction::Approve,
                Some("aynı"),
                Some("aynı"),
                &format!("2026-07-20T10:{n:02}:00Z"),
            ));
        }

        let trend = compute(&events);
        let earlier = trend.earlier.expect("earlier");
        let recent = trend.recent.expect("recent");
        assert_eq!(earlier.mean_burden, 1.0, "everything was thrown away");
        assert_eq!(recent.mean_burden, 0.0, "everything was kept");
        assert_eq!(earlier.accepted_rate(), Some(0.0));
        assert_eq!(recent.accepted_rate(), Some(1.0));
        assert_eq!(trend.overall.reviewed, 2 * WINDOW);
    }
}
