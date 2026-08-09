//! Diarization Error Rate (DER) over RTTM, with OPTIMAL speaker mapping.
//!
//! This is the measuring instrument for ADR-0034 step (c). It exists BEFORE any
//! diarization engine, on purpose: `metrics.rs` scores only WER/CER/term-recall,
//! so today the project cannot tell a good speaker segmentation from a bad one.
//! Choosing an engine without this would be choosing blind.
//!
//! ## What DER is
//!
//! Reference and hypothesis are both sets of `(speaker, start, end)` turns. The
//! timeline is cut at every boundary; on each elementary interval let
//! `R` = distinct reference speakers active, `H` = hypothesis speakers active,
//! and `C` = pairs of them joined by the speaker mapping. Then
//!
//! ```text
//! missed      = max(0, |R| - |H|)
//! false_alarm = max(0, |H| - |R|)
//! confusion   = min(|R|, |H|) - |C|
//! DER = Σ dur·(missed + false_alarm + confusion) / Σ dur·|R|
//! ```
//!
//! This is the NIST `md-eval` formulation, which pyannote also implements, so
//! numbers here are comparable with published ones. DER can exceed 1.0 — a
//! system that invents speech everywhere is not capped at "100% wrong".
//!
//! ## Two decisions that determine whether the number is trustworthy
//!
//! **1. Optimal mapping, never greedy.** Diarization labels are arbitrary: a
//! system that segments perfectly but calls the speakers `X`/`Y` instead of
//! `A`/`B` must score 0.0, not 1.0. So reference speakers are matched to
//! hypothesis speakers by the assignment that MAXIMISES total mapped overlap,
//! solved exactly (Hungarian / Kuhn–Munkres). Picking pairs greedily by largest
//! overlap is the tempting shortcut and it silently INFLATES DER — see
//! `greedy_mapping_would_overstate_der`, where greedy reports 8/13 (0.62) on
//! data whose true error is 5/13 (0.38). A harness that overstates error would
//! send us rewriting an engine that was fine. Replacing this solver with a
//! greedy one is confirmed to fail that test and the brute-force check, and no
//! others — the two are the only guards on this property.
//!
//! **2. Integer microseconds, not floating-point seconds.** Every quantity here
//! is an exact `i64` count of microseconds. The algorithm is built on sorting
//! and comparing interval boundaries, and with `f64` two boundaries that are
//! mathematically equal (`0.1 + 0.2` vs `0.3`) compare unequal, producing
//! zero-width phantom intervals and drift in the totals. RTTM carries at most
//! millisecond precision, so microseconds lose nothing and make boundary
//! equality exact. Only the final division returns to `f64`.
//!
//! ## Deliberate fail-closed behaviours
//!
//! - An RTTM containing more than one `file_id` is an ERROR unless a file is
//!   named. Silently concatenating two meetings' turns yields a plausible number
//!   that means nothing.
//! - A reference and a hypothesis naming DIFFERENT recordings is an ERROR
//!   ([`ensure_same_recording`]). Rejecting several ids inside one file does not
//!   cover this: two files each carry one valid id, and two unrelated recordings
//!   whose timelines happen to line up would otherwise score a confident 0%.
//! - A reference with no speech is an ERROR. DER's denominator would be zero;
//!   reporting `0.0` ("perfect") or `1.0` ("all wrong") would both be lies.
//! - A malformed line is an ERROR with its line number, never skipped. Silently
//!   dropping turns makes the reference shorter and the score better.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{bail, Context, Result};

/// Exact time, in microseconds. See the module note on why this is not `f64`.
pub type Micros = i64;

const MICROS_PER_SEC: f64 = 1_000_000.0;

/// Largest time this module accepts, in seconds. A hundred days is far beyond
/// any meeting and far below the point where microsecond arithmetic overflows.
const MAX_SECS: f64 = 100.0 * 24.0 * 60.0 * 60.0;

/// Convert seconds to exact microseconds, rounding to the nearest microsecond.
///
/// Callers must reject out-of-range input first ([`checked_secs_to_micros`]).
/// Rust's float→int `as` cast SATURATES, so a huge value would silently become
/// `i64::MAX` and the turn would be dropped rather than reported.
pub fn secs_to_micros(secs: f64) -> Micros {
    (secs * MICROS_PER_SEC).round() as Micros
}

/// Convert seconds to microseconds, refusing anything outside a sane range.
///
/// This exists because the plain cast fails OPEN: `secs_to_micros(1e30)` is
/// `i64::MAX`, and downstream that turn simply vanishes with no error — a
/// shorter reference and therefore a better-looking score. Parsing goes through
/// this; the unchecked form stays for callers holding known-good values.
pub fn checked_secs_to_micros(secs: f64) -> Option<Micros> {
    if !secs.is_finite() || secs.abs() > MAX_SECS {
        return None;
    }
    Some(secs_to_micros(secs))
}

/// Convert microseconds back to seconds, for reporting only.
pub fn micros_to_secs(us: Micros) -> f64 {
    us as f64 / MICROS_PER_SEC
}

/// One speaker turn: `speaker` is active over `[start, end)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Turn {
    pub speaker: String,
    pub start: Micros,
    pub end: Micros,
}

// The constructors and the RTTM writer are the half of this module that only
// tests and the (not yet written) engine exercise; in a plain binary build they
// are legitimately unreferenced. Dropping the writer to silence that would cost
// the parser its round-trip test, which is the only thing proving the two halves
// agree on the format.
#[cfg_attr(not(test), allow(dead_code))]
impl Turn {
    pub fn new(speaker: impl Into<String>, start: Micros, end: Micros) -> Self {
        Turn {
            speaker: speaker.into(),
            start,
            end,
        }
    }

    /// Build from seconds — convenience for tests and for callers holding RTTM
    /// floats.
    pub fn from_secs(speaker: impl Into<String>, start: f64, end: f64) -> Self {
        Turn::new(speaker, secs_to_micros(start), secs_to_micros(end))
    }

    pub fn duration(&self) -> Micros {
        (self.end - self.start).max(0)
    }
}

/// Which stretch of the timeline is scored at all.
///
/// This exists because of a real disagreement found by cross-checking against
/// NIST `md-eval.pl` on VoxConverse: hypothesis speech BEFORE the first
/// reference turn or AFTER the last was counted as false alarm here and not
/// there. Verified from md-eval's source rather than inferred — `uem_from_rttm`
/// (md-eval-22.pl:2245) returns a single span `[min TBEG, max TEND]` over the
/// REFERENCE, and line 626 installs it whenever no UEM is supplied.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum EvalRegion {
    /// `[earliest reference start, latest reference end]`, one contiguous span
    /// with internal gaps still scored. This is md-eval's default and therefore
    /// the convention every published DER number is computed under, which is why
    /// it is ours: a number that cannot be compared to the literature cannot
    /// tell us whether an engine is good.
    #[default]
    ReferenceExtent,
    /// Score the whole timeline; nothing is out of bounds. Use when the question
    /// is "did this system invent speech anywhere", not "how does it compare".
    Full,
    /// Explicit spans, as read from a UEM file.
    Uem(Vec<(Micros, Micros)>),
}

/// How to score.
///
/// The derived default — `collar = 0`, `skip_overlap = false`,
/// `region = ReferenceExtent` — is the convention published VoxConverse numbers
/// are computed with. Defaulting to the more forgiving NIST 0.25 s collar would
/// make our numbers look better than the papers we compare against.
#[derive(Debug, Clone, Default)]
pub struct DerOptions {
    /// No-score margin around every REFERENCE turn boundary, in microseconds.
    /// Absorbs annotation imprecision at speaker changes. NIST convention is
    /// 0.25 s; pyannote publishes with 0.
    pub collar: Micros,
    /// Exclude regions where more than one reference speaker is active. Some
    /// papers report this variant; it always lowers DER, so it must never be
    /// compared against a number computed with overlap included.
    pub skip_overlap: bool,
    /// The stretch of timeline that is scored at all.
    pub region: EvalRegion,
}

/// A scored comparison. Component times are in seconds, for reporting.
#[derive(Debug, Clone)]
pub struct DerReport {
    /// `(missed + false_alarm + confusion) / total_reference`. May exceed 1.0.
    pub der: f64,
    pub missed: f64,
    pub false_alarm: f64,
    pub confusion: f64,
    /// Denominator: Σ over intervals of duration × number of active reference
    /// speakers, so overlapped speech counts once per speaker.
    pub total_reference: f64,
    /// Reference speaker → hypothesis speaker, for the chosen optimal mapping.
    /// Reference speakers matched to nothing are absent.
    pub mapping: BTreeMap<String, String>,
    pub ref_speakers: usize,
    pub hyp_speakers: usize,
    /// Wall-clock time excluded by the collar / overlap options, in seconds.
    pub excluded: f64,
    /// Hypothesis speech that fell OUTSIDE the evaluation region and was
    /// therefore not scored, in speaker-seconds.
    ///
    /// Reported separately and never folded away, because under the default
    /// region this is exactly the speech a diarizer invented in a meeting's
    /// leading or trailing silence — a phantom speaker in the report. The
    /// convention says not to count it; nothing says to hide it.
    pub unscored_hypothesis: f64,
    /// The evaluation region actually applied, in seconds, for the record.
    pub region: Vec<(f64, f64)>,
}

impl DerReport {
    /// DER as a percentage, the unit papers quote.
    pub fn der_percent(&self) -> f64 {
        self.der * 100.0
    }
}

// ---------------------------------------------------------------------------
// RTTM
// ---------------------------------------------------------------------------

/// One parsed RTTM: the turns, plus the recording they belong to.
///
/// The identity is carried rather than discarded because it is the only thing
/// that can tell whether a reference and a hypothesis describe the SAME audio —
/// see [`ensure_same_recording`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rttm {
    /// `None` only when the text contained no `SPEAKER` records at all.
    pub file_id: Option<String>,
    pub turns: Vec<Turn>,
}

/// Parse RTTM text.
///
/// Format (NIST): whitespace-separated
/// `SPEAKER <file_id> <chan> <tbeg> <tdur> <ortho> <stype> <name> <conf> <slat>`
///
/// Only `SPEAKER` records are read; other record types are ignored by design,
/// since RTTM also carries `SPKR-INFO` and non-speech records that are not turns.
/// `file_id` selects one file when the stream holds several; passing `None`
/// while several are present is an error (see the module note).
pub fn parse_rttm(text: &str, file_id: Option<&str>) -> Result<Rttm> {
    let mut turns = Vec::new();
    let mut seen_files: BTreeSet<String> = BTreeSet::new();

    for (idx, raw) in text.lines().enumerate() {
        let lineno = idx + 1;
        // A UTF-8 BOM is not whitespace, so without this the first record's
        // type token is "\u{feff}SPEAKER", it fails the match below, and the
        // turn is silently SKIPPED -- a shorter reference and a better-looking
        // score, from a file that looks fine in every editor. `Set-Content
        // -Encoding utf8` on PowerShell 5.1 writes exactly this, and that is
        // the command this harness's own error messages tell users to run.
        let line = raw.trim_start_matches('\u{feff}').trim();
        if line.is_empty() || line.starts_with(";;") || line.starts_with('#') {
            continue;
        }
        let f: Vec<&str> = line.split_whitespace().collect();
        if !f[0].eq_ignore_ascii_case("SPEAKER") {
            continue;
        }
        // SPEAKER + file + chan + tbeg + tdur + ortho + stype + name = 8 minimum.
        if f.len() < 8 {
            bail!(
                "RTTM satır {lineno}: SPEAKER kaydı {} alan içeriyor, en az 8 bekleniyor: {line}",
                f.len()
            );
        }

        let file = f[1].to_string();
        if let Some(want) = file_id {
            if file != want {
                continue;
            }
        }
        seen_files.insert(file);

        let tbeg: f64 = f[3].parse().with_context(|| {
            format!("RTTM satır {lineno}: başlangıç zamanı okunamadı ({})", f[3])
        })?;
        let tdur: f64 = f[4]
            .parse()
            .with_context(|| format!("RTTM satır {lineno}: süre okunamadı ({})", f[4]))?;
        if tdur < 0.0 {
            bail!("RTTM satır {lineno}: negatif süre ({tdur})");
        }
        if tbeg < 0.0 {
            bail!("RTTM satır {lineno}: negatif başlangıç zamanı ({tbeg})");
        }
        let speaker = f[7].to_string();
        if speaker.is_empty() || speaker == "<NA>" {
            bail!("RTTM satır {lineno}: konuşmacı adı yok");
        }

        // Range-checked: the plain cast saturates, so an absurd time would
        // otherwise become i64::MAX and the turn would vanish without a word.
        let (Some(start), Some(dur)) = (checked_secs_to_micros(tbeg), checked_secs_to_micros(tdur))
        else {
            bail!(
                "RTTM satır {lineno}: zaman değeri aralık dışı (tbeg={tbeg}, tdur={tdur}); \
                 en fazla {} saniye kabul edilir",
                MAX_SECS
            );
        };
        let end = start + dur;
        if end > start {
            turns.push(Turn {
                speaker,
                start,
                end,
            });
        }
    }

    if file_id.is_none() && seen_files.len() > 1 {
        let names: Vec<&str> = seen_files.iter().map(|s| s.as_str()).collect();
        bail!(
            "RTTM {} farklı dosya kimliği içeriyor ({}); hangisinin puanlanacağı belirtilmeli \
             — birden fazla toplantının turn'lerini birleştirmek anlamsız bir DER üretir",
            seen_files.len(),
            names.join(", ")
        );
    }

    Ok(Rttm {
        file_id: seen_files.into_iter().next(),
        turns,
    })
}

/// Refuse to score a reference and a hypothesis that describe different
/// recordings.
///
/// Rejecting several ids WITHIN one file is not enough: two separately parsed
/// files each carry one valid id, and nothing else notices that they are not the
/// same meeting. Two unrelated recordings whose timelines happen to line up can
/// then report a low, entirely meaningless DER — the same class of plausible
/// lie this module refuses everywhere else.
pub fn ensure_same_recording(reference: &Rttm, hypothesis: &Rttm) -> Result<()> {
    match (&reference.file_id, &hypothesis.file_id) {
        (Some(r), Some(h)) if r != h => bail!(
            "referans '{r}' kaydını, hipotez '{h}' kaydını tanımlıyor — farklı kayıtları \
             karşılaştırmak anlamsız bir DER üretir. Aynı kaydın iki RTTM'ini verin ya da \
             ortak kimliği --file-id ile seçin"
        ),
        _ => Ok(()),
    }
}

/// Render turns as RTTM. Times are written with millisecond precision, which is
/// what the format carries in practice.
#[cfg_attr(not(test), allow(dead_code))]
pub fn write_rttm(file_id: &str, turns: &[Turn]) -> String {
    let mut out = String::new();
    for t in turns {
        out.push_str(&format!(
            "SPEAKER {} 1 {:.3} {:.3} <NA> <NA> {} <NA> <NA>\n",
            file_id,
            micros_to_secs(t.start),
            micros_to_secs(t.duration()),
            t.speaker
        ));
    }
    out
}

// ---------------------------------------------------------------------------
// Scoring
// ---------------------------------------------------------------------------

/// Parse a UEM (Un-partitioned Evaluation Map): `<file-id> <chan> <beg> <end>`.
///
/// Spans are returned merged and sorted. Overlapping spans are an error, as they
/// are in md-eval — they would double-count reference time.
pub fn parse_uem(text: &str, file_id: Option<&str>) -> Result<Vec<(Micros, Micros)>> {
    let mut spans: Vec<(Micros, Micros)> = Vec::new();
    for (idx, raw) in text.lines().enumerate() {
        let lineno = idx + 1;
        // Same BOM hazard as the RTTM parser above.
        let line = raw.trim_start_matches('\u{feff}').trim();
        if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
            continue;
        }
        let f: Vec<&str> = line.split_whitespace().collect();
        if f.len() < 4 {
            bail!("UEM satır {lineno}: 4 alan bekleniyor (<dosya> <kanal> <baş> <son>): {line}");
        }
        if let Some(want) = file_id {
            if f[0] != want {
                continue;
            }
        }
        let beg: f64 = f[2]
            .parse()
            .with_context(|| format!("UEM satır {lineno}: başlangıç okunamadı ({})", f[2]))?;
        let end: f64 = f[3]
            .parse()
            .with_context(|| format!("UEM satır {lineno}: bitiş okunamadı ({})", f[3]))?;
        if end <= beg || beg < 0.0 {
            bail!("UEM satır {lineno}: geçersiz aralık ({beg}, {end})");
        }
        let (Some(b), Some(e)) = (checked_secs_to_micros(beg), checked_secs_to_micros(end)) else {
            bail!("UEM satır {lineno}: zaman değeri aralık dışı ({beg}, {end})");
        };
        spans.push((b, e));
    }
    spans.sort_unstable();
    for pair in spans.windows(2) {
        if pair[1].0 < pair[0].1 {
            bail!(
                "UEM örtüşen değerlendirme aralıkları içeriyor ({}, {}) ve ({}, {}) \
                 — referans süresi iki kez sayılırdı",
                micros_to_secs(pair[0].0),
                micros_to_secs(pair[0].1),
                micros_to_secs(pair[1].0),
                micros_to_secs(pair[1].1)
            );
        }
    }
    Ok(spans)
}

/// Restrict turns to `region`, returning the clipped turns and the speaker-time
/// dropped.
fn clip(turns: &[Turn], region: &[(Micros, Micros)]) -> (Vec<Turn>, Micros) {
    let mut kept = Vec::with_capacity(turns.len());
    let mut dropped: Micros = 0;
    for t in turns {
        let mut covered: Micros = 0;
        for (a, b) in region {
            let s = t.start.max(*a);
            let e = t.end.min(*b);
            if e > s {
                covered += e - s;
                kept.push(Turn {
                    speaker: t.speaker.clone(),
                    start: s,
                    end: e,
                });
            }
        }
        dropped += t.duration() - covered;
    }
    (kept, dropped)
}

/// Score a hypothesis against a reference.
pub fn der(reference: &[Turn], hypothesis: &[Turn], opts: &DerOptions) -> Result<DerReport> {
    // Resolve the evaluation region first: everything below is scored inside it.
    let region: Vec<(Micros, Micros)> = match &opts.region {
        EvalRegion::Full => vec![(Micros::MIN / 4, Micros::MAX / 4)],
        EvalRegion::Uem(spans) => spans.clone(),
        EvalRegion::ReferenceExtent => {
            let beg = reference
                .iter()
                .filter(|t| t.duration() > 0)
                .map(|t| t.start)
                .min();
            let end = reference
                .iter()
                .filter(|t| t.duration() > 0)
                .map(|t| t.end)
                .max();
            match (beg, end) {
                (Some(b), Some(e)) if e > b => vec![(b, e)],
                _ => Vec::new(),
            }
        }
    };

    // Collars come from the ORIGINAL reference boundaries, computed BEFORE the
    // region clips anything. Clipping first would put an artificial turn edge
    // wherever a UEM span cuts through a turn, and a UEM edge is not a speaker
    // change — a `[0,10)` turn scored under UEM `[2,8]` would silently lose
    // `[2,2.25)` and `[7.75,8)` to no-score zones that correspond to nothing in
    // the audio, shrinking the denominator.
    let collar_zones = collar_zones(&index_by_speaker(reference), opts.collar);

    let (reference, _) = clip(reference, &region);
    let (hypothesis, unscored_hypothesis) = clip(hypothesis, &region);
    let reference = &reference[..];
    let hypothesis = &hypothesis[..];

    let ref_by_spk = index_by_speaker(reference);
    let hyp_by_spk = index_by_speaker(hypothesis);

    let ref_names: Vec<String> = ref_by_spk.keys().cloned().collect();
    let hyp_names: Vec<String> = hyp_by_spk.keys().cloned().collect();

    // Elementary intervals: every boundary that any turn or collar edge
    // introduces. Exact integer comparison is what makes this safe.
    let mut cuts: BTreeSet<Micros> = BTreeSet::new();
    for t in reference.iter().chain(hypothesis.iter()) {
        cuts.insert(t.start);
        cuts.insert(t.end);
    }
    for (a, b) in &collar_zones {
        cuts.insert(*a);
        cuts.insert(*b);
    }
    let cuts: Vec<Micros> = cuts.into_iter().collect();

    // Pass 1: per-interval active-speaker sets, and the pairwise overlap matrix
    // the assignment is solved on. See the note inside the loop for why the
    // matrix spans the whole evaluation region while the counted intervals do
    // not.
    let mut overlap = vec![vec![0i64; hyp_names.len()]; ref_names.len()];
    let mut intervals: Vec<(Micros, Vec<usize>, Vec<usize>)> = Vec::new();
    let mut excluded: Micros = 0;

    for w in cuts.windows(2) {
        let (a, b) = (w[0], w[1]);
        let dur = b - a;
        if dur <= 0 {
            continue;
        }
        let active_ref = active(&ref_by_spk, &ref_names, a, b);
        let active_hyp = active(&hyp_by_spk, &hyp_names, a, b);

        // The mapping basis is accumulated BEFORE the collar and overlap
        // exclusions, and the error components after. That split is md-eval's,
        // verified from its source: `$spkr_overlap` is summed over `$eval_segs`
        // and `map_speakers` is called on it (md-eval-22.pl:1890-1908), while
        // `$uem_score` — with collars, and with overlap removed under `-1` — is
        // only built afterwards (:1913-1918) and used solely for counting.
        //
        // Deciding WHICH speaker is which on the same region the errors are
        // counted on is the tempting shortcut: it optimises the mapping against
        // the score, so it yields a systematically LOWER DER than the
        // literature's. A measuring instrument that flatters us is worse than
        // no instrument. Cross-checking against md-eval on VoxConverse found
        // exactly this: identical at collar 0, ours too low at collar 0.25.
        for &r in &active_ref {
            for &h in &active_hyp {
                overlap[r][h] += dur;
            }
        }

        let in_collar = in_any_zone(&collar_zones, a, b);
        let skipped_overlap = opts.skip_overlap && active_ref.len() > 1;
        if in_collar || skipped_overlap {
            if !active_ref.is_empty() || !active_hyp.is_empty() {
                excluded += dur;
            }
            continue;
        }

        intervals.push((dur, active_ref, active_hyp));
    }

    let total_reference: Micros = intervals.iter().map(|(d, r, _)| d * r.len() as i64).sum();
    if total_reference == 0 {
        bail!(
            "referansta puanlanabilir konuşma yok — DER'in paydası sıfır olurdu; \
             0.0 (kusursuz) veya 1.0 (tamamen yanlış) raporlamak ikisi de yanlış olurdu"
        );
    }

    // Pass 2: optimal mapping, then the three error components under it.
    let assignment = max_weight_assignment(&overlap);
    let mut mapping = BTreeMap::new();
    let mut hyp_of_ref: Vec<Option<usize>> = vec![None; ref_names.len()];
    for (r, maybe_h) in assignment.iter().enumerate() {
        if let Some(h) = maybe_h {
            // A pair with zero overlap is not a match; recording it would put a
            // meaningless row in the report.
            if overlap[r][*h] > 0 {
                hyp_of_ref[r] = Some(*h);
                mapping.insert(ref_names[r].clone(), hyp_names[*h].clone());
            }
        }
    }

    let (mut missed, mut false_alarm, mut confusion) = (0i64, 0i64, 0i64);
    for (dur, active_ref, active_hyp) in &intervals {
        let nr = active_ref.len() as i64;
        let nh = active_hyp.len() as i64;
        let correct = active_ref
            .iter()
            .filter(|r| match hyp_of_ref[**r] {
                Some(h) => active_hyp.contains(&h),
                None => false,
            })
            .count() as i64;

        missed += dur * (nr - nh).max(0);
        false_alarm += dur * (nh - nr).max(0);
        confusion += dur * (nr.min(nh) - correct);
    }

    let denom = total_reference as f64;
    Ok(DerReport {
        der: (missed + false_alarm + confusion) as f64 / denom,
        missed: micros_to_secs(missed),
        false_alarm: micros_to_secs(false_alarm),
        confusion: micros_to_secs(confusion),
        total_reference: micros_to_secs(total_reference),
        mapping,
        ref_speakers: ref_names.len(),
        hyp_speakers: hyp_names.len(),
        excluded: micros_to_secs(excluded),
        unscored_hypothesis: micros_to_secs(unscored_hypothesis),
        region: if matches!(opts.region, EvalRegion::Full) {
            Vec::new()
        } else {
            region
                .iter()
                .map(|(a, b)| (micros_to_secs(*a), micros_to_secs(*b)))
                .collect()
        },
    })
}

/// Group turns by speaker, merging a speaker's self-overlaps.
///
/// A speaker overlapping themselves is malformed input, but tolerating it
/// matters: left alone it would count that speaker twice in `|R|`, inflating the
/// denominator and quietly LOWERING DER.
fn index_by_speaker(turns: &[Turn]) -> BTreeMap<String, Vec<(Micros, Micros)>> {
    let mut by: BTreeMap<String, Vec<(Micros, Micros)>> = BTreeMap::new();
    for t in turns {
        if t.duration() > 0 {
            by.entry(t.speaker.clone())
                .or_default()
                .push((t.start, t.end));
        }
    }
    for spans in by.values_mut() {
        spans.sort_unstable();
        let mut merged: Vec<(Micros, Micros)> = Vec::with_capacity(spans.len());
        for (s, e) in spans.iter().copied() {
            match merged.last_mut() {
                Some(last) if s <= last.1 => last.1 = last.1.max(e),
                _ => merged.push((s, e)),
            }
        }
        *spans = merged;
    }
    by
}

/// Indices of speakers active across the whole of `[a, b)`.
fn active(
    by: &BTreeMap<String, Vec<(Micros, Micros)>>,
    names: &[String],
    a: Micros,
    b: Micros,
) -> Vec<usize> {
    names
        .iter()
        .enumerate()
        .filter(|(_, name)| {
            by.get(*name)
                .is_some_and(|spans| spans.iter().any(|(s, e)| *s <= a && *e >= b))
        })
        .collect::<Vec<_>>()
        .into_iter()
        .map(|(i, _)| i)
        .collect()
}

/// No-score zones: `±collar` around every reference speaker-change boundary.
///
/// Built from the MERGED per-speaker spans, never from the raw records. An
/// annotator who splits one speaker's continuous speech into two lines — per
/// sentence, per breath — has not created a speaker change, and treating that
/// split as a boundary would put a no-score zone in the middle of uninterrupted
/// speech. That made the metric depend on annotation style: see
/// `splitting_one_speakers_turn_does_not_change_der`, which scored 1.05% and
/// 0.00% on the same audio before this was fixed.
fn collar_zones(
    by_speaker: &BTreeMap<String, Vec<(Micros, Micros)>>,
    collar: Micros,
) -> Vec<(Micros, Micros)> {
    if collar <= 0 {
        return Vec::new();
    }
    let mut zones: Vec<(Micros, Micros)> = Vec::new();
    for spans in by_speaker.values() {
        for (start, end) in spans {
            zones.push((start - collar, start + collar));
            zones.push((end - collar, end + collar));
        }
    }
    zones.sort_unstable();
    let mut merged: Vec<(Micros, Micros)> = Vec::with_capacity(zones.len());
    for (s, e) in zones {
        match merged.last_mut() {
            Some(last) if s <= last.1 => last.1 = last.1.max(e),
            _ => merged.push((s, e)),
        }
    }
    merged
}

fn in_any_zone(zones: &[(Micros, Micros)], a: Micros, b: Micros) -> bool {
    zones.iter().any(|(s, e)| *s <= a && *e >= b)
}

// ---------------------------------------------------------------------------
// Corpus aggregation
// ---------------------------------------------------------------------------

/// A whole corpus scored as one number.
#[derive(Debug, Clone)]
pub struct CorpusReport {
    /// **The** DER for the corpus: total error time over total reference time.
    pub pooled_der: f64,
    /// The mean of the per-file DERs, reported ONLY so the gap with `pooled_der`
    /// is visible. Never quote this as the corpus DER — see [`pool`].
    pub macro_der: f64,
    pub missed: f64,
    pub false_alarm: f64,
    pub confusion: f64,
    pub total_reference: f64,
    pub unscored_hypothesis: f64,
    pub files: usize,
    /// `(file id, DER)` worst first.
    pub per_file: Vec<(String, f64)>,
}

/// Aggregate per-file results into one corpus number.
///
/// **Pooled, not averaged.** DER is a ratio of times, and the mean of ratios is
/// not the ratio of sums: a corpus with one 30-minute file at 5% and one
/// 10-second file at 100% is 5.5% pooled and 52.5% averaged. Averaging lets a
/// handful of tiny files dominate a number that is supposed to describe hours of
/// audio — and VoxConverse dev really does mix a 22-second file with a
/// 20-minute one. `md-eval` and `pyannote` both pool, so an averaged number
/// would also not be comparable with any published one.
///
/// The macro average is still computed and reported, because a large gap between
/// the two is real information: it means performance depends strongly on file
/// length, which pooling alone would hide.
pub fn pool(results: &[(String, DerReport)]) -> CorpusReport {
    let mut missed = 0.0;
    let mut false_alarm = 0.0;
    let mut confusion = 0.0;
    let mut total_reference = 0.0;
    let mut unscored_hypothesis = 0.0;
    let mut per_file: Vec<(String, f64)> = Vec::with_capacity(results.len());

    for (id, r) in results {
        missed += r.missed;
        false_alarm += r.false_alarm;
        confusion += r.confusion;
        total_reference += r.total_reference;
        unscored_hypothesis += r.unscored_hypothesis;
        per_file.push((id.clone(), r.der));
    }
    per_file.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    let macro_der = if results.is_empty() {
        0.0
    } else {
        results.iter().map(|(_, r)| r.der).sum::<f64>() / results.len() as f64
    };

    CorpusReport {
        pooled_der: if total_reference > 0.0 {
            (missed + false_alarm + confusion) / total_reference
        } else {
            0.0
        },
        macro_der,
        missed,
        false_alarm,
        confusion,
        total_reference,
        unscored_hypothesis,
        files: results.len(),
        per_file,
    }
}

// ---------------------------------------------------------------------------
// Assignment
// ---------------------------------------------------------------------------

/// Exact maximum-weight bipartite assignment (Hungarian / Kuhn–Munkres),
/// O(n²m). Returns, for each row, the column it is assigned to.
///
/// Written out rather than pulled in as a dependency so that its result can be
/// checked against brute-force optimal in tests — which is stronger evidence
/// than trusting a crate, and is exactly what
/// `hungarian_matches_brute_force_on_random_matrices` does.
pub fn max_weight_assignment(weights: &[Vec<i64>]) -> Vec<Option<usize>> {
    let n = weights.len();
    if n == 0 {
        return Vec::new();
    }
    let m = weights[0].len();
    if m == 0 {
        return vec![None; n];
    }

    // The solver requires rows <= columns; transpose and invert the result if not.
    if n > m {
        let mut t = vec![vec![0i64; n]; m];
        for (i, row) in weights.iter().enumerate() {
            for (j, w) in row.iter().enumerate() {
                t[j][i] = *w;
            }
        }
        let solved = max_weight_assignment(&t);
        let mut out = vec![None; n];
        for (col, maybe_row) in solved.iter().enumerate() {
            if let Some(row) = maybe_row {
                out[*row] = Some(col);
            }
        }
        return out;
    }

    // Minimise the negated weights: a maximum-weight assignment of `w` is a
    // minimum-cost assignment of `-w`.
    let cost = |i: usize, j: usize| -weights[i][j];

    const INF: i64 = i64::MAX / 4;
    let mut u = vec![0i64; n + 1];
    let mut v = vec![0i64; m + 1];
    let mut p = vec![0usize; m + 1]; // p[j] = 1-based row matched to column j
    let mut way = vec![0usize; m + 1];

    for i in 1..=n {
        p[0] = i;
        let mut j0 = 0usize;
        let mut minv = vec![INF; m + 1];
        let mut used = vec![false; m + 1];
        loop {
            used[j0] = true;
            let i0 = p[j0];
            let mut delta = INF;
            let mut j1 = 0usize;
            for j in 1..=m {
                if used[j] {
                    continue;
                }
                let cur = cost(i0 - 1, j - 1) - u[i0] - v[j];
                if cur < minv[j] {
                    minv[j] = cur;
                    way[j] = j0;
                }
                if minv[j] < delta {
                    delta = minv[j];
                    j1 = j;
                }
            }
            for j in 0..=m {
                if used[j] {
                    u[p[j]] += delta;
                    v[j] -= delta;
                } else {
                    minv[j] -= delta;
                }
            }
            j0 = j1;
            if p[j0] == 0 {
                break;
            }
        }
        loop {
            let j1 = way[j0];
            p[j0] = p[j1];
            j0 = j1;
            if j0 == 0 {
                break;
            }
        }
    }

    let mut out = vec![None; n];
    for j in 1..=m {
        if p[j] != 0 {
            out[p[j] - 1] = Some(j - 1);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    fn score(reference: &[Turn], hypothesis: &[Turn]) -> DerReport {
        der(reference, hypothesis, &DerOptions::default()).expect("scored")
    }

    // -- The property the whole feature rests on ---------------------------

    /// Diarization labels are arbitrary. A system that segments perfectly but
    /// names the speakers differently is CORRECT and must score 0.
    #[test]
    fn relabelled_but_identical_segmentation_scores_zero() {
        let reference = vec![
            Turn::from_secs("A", 0.0, 10.0),
            Turn::from_secs("B", 10.0, 20.0),
        ];
        let hypothesis = vec![
            Turn::from_secs("spk1", 0.0, 10.0),
            Turn::from_secs("spk0", 10.0, 20.0),
        ];

        let r = score(&reference, &hypothesis);
        assert!(approx(r.der, 0.0), "der was {}", r.der);
        assert_eq!(r.mapping.get("A").map(String::as_str), Some("spk1"));
        assert_eq!(r.mapping.get("B").map(String::as_str), Some("spk0"));
    }

    /// The reason the mapping is solved exactly instead of greedily. Greedy
    /// takes the single largest overlap first and strands the rest; here that
    /// costs 0.50 DER on data where the truth is 0.20.
    #[test]
    fn greedy_mapping_would_overstate_der() {
        // Overlap matrix by construction:  A×X = 5, A×Y = 4, B×X = 4, B×Y = 0.
        // Greedy: A→X (5), then B has only Y left (0)  → mapped 5 s.
        // Optimal: A→Y (4) + B→X (4)                    → mapped 8 s.
        let reference = vec![
            Turn::from_secs("A", 0.0, 5.0),
            Turn::from_secs("A", 10.0, 14.0),
            Turn::from_secs("B", 20.0, 24.0),
        ];
        let hypothesis = vec![
            Turn::from_secs("X", 0.0, 5.0),
            Turn::from_secs("X", 20.0, 24.0),
            Turn::from_secs("Y", 10.0, 14.0),
        ];

        let r = score(&reference, &hypothesis);
        // Total reference speech = 5 + 4 + 4 = 13 s. Under the optimal mapping
        // only A's first 5 s are confused; missed and false alarm are zero.
        assert!(approx(r.total_reference, 13.0));
        assert!(approx(r.missed, 0.0));
        assert!(approx(r.false_alarm, 0.0));
        assert!(approx(r.confusion, 5.0));
        assert!(approx(r.der, 5.0 / 13.0), "der was {}", r.der);
        assert_eq!(r.mapping.get("A").map(String::as_str), Some("Y"));
        assert_eq!(r.mapping.get("B").map(String::as_str), Some("X"));

        // And prove greedy really is worse, so this test keeps its meaning:
        // greedy maps A→X, leaving B→Y with zero overlap.
        // Its confusion would be A's 4 s at [10,14) + B's 4 s at [20,24) = 8 s.
        let greedy_der = 8.0 / 13.0;
        assert!(greedy_der > r.der);
    }

    // -- Hand-computed known answers ---------------------------------------

    #[test]
    fn missed_speech_only() {
        // ref A[0,10) B[10,20); hyp X[0,8) Y[12,20) → 2 s + 2 s not covered.
        let reference = vec![
            Turn::from_secs("A", 0.0, 10.0),
            Turn::from_secs("B", 10.0, 20.0),
        ];
        let hypothesis = vec![
            Turn::from_secs("X", 0.0, 8.0),
            Turn::from_secs("Y", 12.0, 20.0),
        ];

        let r = score(&reference, &hypothesis);
        assert!(approx(r.missed, 4.0));
        assert!(approx(r.false_alarm, 0.0));
        assert!(approx(r.confusion, 0.0));
        assert!(approx(r.total_reference, 20.0));
        assert!(approx(r.der, 0.2));
    }

    #[test]
    fn one_reference_speaker_split_in_two_is_confusion() {
        // ref A[0,10); hyp X[0,5) Y[5,10). Only one hyp speaker can map to A.
        let reference = vec![Turn::from_secs("A", 0.0, 10.0)];
        let hypothesis = vec![
            Turn::from_secs("X", 0.0, 5.0),
            Turn::from_secs("Y", 5.0, 10.0),
        ];

        let r = score(&reference, &hypothesis);
        assert!(approx(r.confusion, 5.0));
        assert!(approx(r.missed, 0.0));
        assert!(approx(r.false_alarm, 0.0));
        assert!(approx(r.der, 0.5));
    }

    #[test]
    fn overlapped_reference_speech_counts_once_per_speaker() {
        // Both A and B talk over [0,10); the system hears only one voice.
        let reference = vec![
            Turn::from_secs("A", 0.0, 10.0),
            Turn::from_secs("B", 0.0, 10.0),
        ];
        let hypothesis = vec![Turn::from_secs("X", 0.0, 10.0)];

        let r = score(&reference, &hypothesis);
        // Denominator counts 10 s twice; the unheard speaker is 10 s missed.
        assert!(approx(r.total_reference, 20.0));
        assert!(approx(r.missed, 10.0));
        assert!(approx(r.der, 0.5));
    }

    #[test]
    fn invented_speech_is_false_alarm_and_der_may_exceed_one() {
        // `Full` on purpose: under the default region this invention lies past
        // the last reference turn and is not scored at all — see
        // `speech_outside_the_reference_extent_is_unscored_but_reported`.
        let reference = vec![Turn::from_secs("A", 0.0, 5.0)];
        let hypothesis = vec![
            Turn::from_secs("X", 0.0, 5.0),
            Turn::from_secs("Y", 5.0, 20.0),
        ];

        let r = der(
            &reference,
            &hypothesis,
            &DerOptions {
                region: EvalRegion::Full,
                ..DerOptions::default()
            },
        )
        .expect("scored");
        assert!(approx(r.false_alarm, 15.0));
        assert!(approx(r.der, 3.0), "der was {}", r.der);
        assert!(approx(r.unscored_hypothesis, 0.0));
    }

    // -- Evaluation region --------------------------------------------------

    /// The disagreement that cross-checking against NIST md-eval on VoxConverse
    /// actually found. Under the field's convention, hypothesis speech outside
    /// the reference's extent is NOT scored — so our numbers are comparable with
    /// published ones — but it is still reported, because under that convention
    /// a diarizer inventing a speaker in a meeting's trailing silence would
    /// otherwise be invisible.
    #[test]
    fn speech_outside_the_reference_extent_is_unscored_but_reported() {
        let reference = vec![
            Turn::from_secs("A", 10.0, 15.0),
            Turn::from_secs("A", 20.0, 25.0),
        ];
        // One invention before the extent, one after, one in the internal gap.
        let before = vec![
            Turn::from_secs("X", 5.0, 6.0),
            Turn::from_secs("X", 10.0, 15.0),
            Turn::from_secs("X", 20.0, 25.0),
        ];
        let after = vec![
            Turn::from_secs("X", 10.0, 15.0),
            Turn::from_secs("X", 20.0, 25.0),
            Turn::from_secs("X", 30.0, 31.0),
        ];
        let inside = vec![
            Turn::from_secs("X", 10.0, 15.0),
            Turn::from_secs("X", 16.0, 17.0),
            Turn::from_secs("X", 20.0, 25.0),
        ];

        for (name, hyp) in [("before", &before), ("after", &after)] {
            let r = score(&reference, hyp);
            assert!(approx(r.der, 0.0), "{name}: der was {}", r.der);
            // Not scored — but not hidden either.
            assert!(
                approx(r.unscored_hypothesis, 1.0),
                "{name}: unscored was {}",
                r.unscored_hypothesis
            );
        }

        // Inside the extent, an internal gap IS scored: the region is one span,
        // not the union of reference speech.
        let r = score(&reference, &inside);
        assert!(approx(r.der, 0.1), "der was {}", r.der);
        assert!(approx(r.false_alarm, 1.0));
        assert!(approx(r.unscored_hypothesis, 0.0));

        // `Full` scores everything, and says so by reporting nothing unscored.
        let full = der(
            &reference,
            &after,
            &DerOptions {
                region: EvalRegion::Full,
                ..DerOptions::default()
            },
        )
        .expect("scored");
        assert!(approx(full.der, 0.1), "der was {}", full.der);
        assert!(approx(full.unscored_hypothesis, 0.0));
    }

    // -- Corpus aggregation -------------------------------------------------

    /// The trap this function exists to avoid: averaging per-file DERs lets a
    /// tiny file outvote hours of audio.
    #[test]
    fn a_corpus_der_is_pooled_not_averaged() {
        // A 30-minute file at 5% and a 10-second file at 100%.
        let long = der(
            &[Turn::from_secs("A", 0.0, 1800.0)],
            &[Turn::from_secs("X", 0.0, 1710.0)],
            &DerOptions::default(),
        )
        .expect("scored");
        let short = der(
            &[Turn::from_secs("A", 0.0, 10.0)],
            &[],
            &DerOptions::default(),
        )
        .expect("scored");
        assert!(approx(long.der, 0.05));
        assert!(approx(short.der, 1.0));

        let c = pool(&[("long".into(), long), ("short".into(), short)]);
        // 90 s + 10 s of error over 1810 s of reference.
        assert!(approx(c.pooled_der, 100.0 / 1810.0), "{}", c.pooled_der);
        assert!(c.pooled_der < 0.056, "pooled must stay near the long file");
        // The average is over nine times larger — and is reported, not hidden.
        assert!(approx(c.macro_der, 0.525));
        assert_eq!(c.files, 2);
        // Worst first, so the per-file table leads with what to look at.
        assert_eq!(c.per_file[0].0, "short");
    }

    /// A system that produced nothing for a file must score that file, not skip
    /// it — otherwise crashing on the hard files improves the corpus number.
    #[test]
    fn a_missing_hypothesis_scores_as_total_miss() {
        let r = der(
            &[Turn::from_secs("A", 0.0, 10.0)],
            &[],
            &DerOptions::default(),
        )
        .expect("scored");
        assert!(approx(r.der, 1.0));
        assert!(approx(r.missed, 10.0));
    }

    #[test]
    fn a_uem_restricts_scoring_to_its_spans() {
        let reference = vec![
            Turn::from_secs("A", 0.0, 10.0),
            Turn::from_secs("A", 20.0, 30.0),
        ];
        let hypothesis = vec![Turn::from_secs("X", 0.0, 10.0)];

        // Without a UEM the second reference turn is missed: 10 / 20 = 50%.
        assert!(approx(score(&reference, &hypothesis).der, 0.5));

        // A UEM covering only the first half scores only that, and it is perfect.
        let uem = parse_uem("meeting 1 0.0 10.0\n", None).expect("parsed");
        let r = der(
            &reference,
            &hypothesis,
            &DerOptions {
                region: EvalRegion::Uem(uem),
                ..DerOptions::default()
            },
        )
        .expect("scored");
        assert!(approx(r.der, 0.0), "der was {}", r.der);
        assert!(approx(r.total_reference, 10.0));
    }

    /// A UEM edge is not a speaker change. Clipping the reference before
    /// building collars would put a no-score zone wherever a UEM span cuts a
    /// turn, shrinking the denominator for a boundary that exists nowhere in the
    /// audio.
    #[test]
    fn a_uem_edge_does_not_create_a_collar() {
        let reference = vec![Turn::from_secs("A", 0.0, 10.0)];
        let hypothesis = vec![Turn::from_secs("X", 0.0, 10.0)];
        let uem = parse_uem("meeting 1 2.0 8.0\n", None).expect("parsed");

        let r = der(
            &reference,
            &hypothesis,
            &DerOptions {
                collar: secs_to_micros(0.25),
                region: EvalRegion::Uem(uem),
                ..DerOptions::default()
            },
        )
        .expect("scored");

        // The whole 6 s window is scored: A never changes speaker inside it.
        assert!(
            approx(r.total_reference, 6.0),
            "reference was {}",
            r.total_reference
        );
        assert!(approx(r.excluded, 0.0), "excluded was {}", r.excluded);
        assert!(approx(r.der, 0.0));
    }

    #[test]
    fn overlapping_uem_spans_are_refused() {
        let e = parse_uem("m 1 0.0 10.0\nm 1 5.0 15.0\n", None).expect_err("must refuse");
        assert!(e.to_string().contains("örtüşen"), "{e}");
    }

    #[test]
    fn a_malformed_uem_line_is_an_error() {
        assert!(parse_uem("m 1 0.0\n", None).is_err());
        assert!(parse_uem("m 1 10.0 5.0\n", None).is_err());
        assert!(parse_uem("m 1 zero 5.0\n", None).is_err());
    }

    #[test]
    fn empty_hypothesis_scores_one() {
        let reference = vec![Turn::from_secs("A", 0.0, 10.0)];
        let r = score(&reference, &[]);
        assert!(approx(r.der, 1.0));
        assert!(approx(r.missed, 10.0));
        assert!(r.mapping.is_empty());
    }

    /// Fail closed: a reference with no speech has no denominator, and both
    /// "perfect" and "all wrong" would be false statements about the system.
    #[test]
    fn empty_reference_is_an_error_not_a_score() {
        let hypothesis = vec![Turn::from_secs("X", 0.0, 5.0)];
        let e = der(&[], &hypothesis, &DerOptions::default()).expect_err("must refuse");
        assert!(e.to_string().contains("paydası sıfır"), "{e}");
    }

    #[test]
    fn a_speaker_overlapping_itself_is_not_counted_twice() {
        // Malformed reference: A annotated twice over the same second.
        let reference = vec![
            Turn::from_secs("A", 0.0, 10.0),
            Turn::from_secs("A", 5.0, 15.0),
        ];
        let r = score(&reference, &[Turn::from_secs("X", 0.0, 15.0)]);
        // 15 s of speech by ONE speaker, not 25 s by two.
        assert!(approx(r.total_reference, 15.0));
        assert!(approx(r.der, 0.0));
    }

    // -- Options ------------------------------------------------------------

    #[test]
    fn collar_excludes_the_region_around_reference_boundaries() {
        // hyp is late by 0.2 s at the A→B switch.
        let reference = vec![
            Turn::from_secs("A", 0.0, 10.0),
            Turn::from_secs("B", 10.0, 20.0),
        ];
        let hypothesis = vec![
            Turn::from_secs("X", 0.0, 10.2),
            Turn::from_secs("Y", 10.2, 20.0),
        ];

        let strict = der(&reference, &hypothesis, &DerOptions::default()).expect("scored");
        assert!(strict.confusion > 0.0, "0.2 s of B is called X");

        let forgiving = der(
            &reference,
            &hypothesis,
            &DerOptions {
                collar: secs_to_micros(0.25),
                ..DerOptions::default()
            },
        )
        .expect("scored");
        assert!(approx(forgiving.der, 0.0), "der was {}", forgiving.der);
        assert!(forgiving.excluded > 0.0);
    }

    /// How an annotator CHUNKS one speaker's continuous speech must not change
    /// the score. `A [0,10)` and `A [0,5) + A [5,10)` describe the same audio;
    /// if the collar is built from raw records, the second invents a no-score
    /// zone at t=5 and the two disagree.
    #[test]
    fn splitting_one_speakers_turn_does_not_change_der() {
        let whole = vec![
            Turn::from_secs("A", 0.0, 10.0),
            Turn::from_secs("B", 10.0, 20.0),
        ];
        let chunked = vec![
            Turn::from_secs("A", 0.0, 5.0),
            Turn::from_secs("A", 5.0, 10.0),
            Turn::from_secs("B", 10.0, 20.0),
        ];
        // An error placed right at the artificial split point.
        let hypothesis = vec![
            Turn::from_secs("X", 0.0, 4.9),
            Turn::from_secs("Y", 4.9, 5.1),
            Turn::from_secs("X", 5.1, 10.0),
            Turn::from_secs("Z", 10.0, 20.0),
        ];
        let opts = DerOptions {
            collar: secs_to_micros(0.25),
            ..DerOptions::default()
        };

        let a = der(&whole, &hypothesis, &opts).expect("scored");
        let b = der(&chunked, &hypothesis, &opts).expect("scored");
        assert!(
            approx(a.der, b.der),
            "chunking changed DER: {} vs {}",
            a.der,
            b.der
        );
        assert!(approx(a.total_reference, b.total_reference));
    }

    /// The speaker mapping is decided on the whole evaluation region, not on
    /// what survives the collar — md-eval's split, and the reason ours matched
    /// it at collar 0 but not at collar 0.25 until this was fixed.
    ///
    /// Here the collar hides most of `X`, so a mapping chosen on the SCORED
    /// region alone would prefer `A -> Y` and report less error. Choosing on the
    /// full region keeps `A -> X`, which is what actually happened in the audio.
    #[test]
    fn the_speaker_mapping_is_decided_before_the_collar_not_after() {
        // A speaks throughout; the boundary at 10 s puts a collar over [9.5, 10.5).
        let reference = vec![
            Turn::from_secs("A", 0.0, 10.0),
            Turn::from_secs("A", 10.0, 12.0),
        ];
        // X covers almost everything; Y covers only what the collar spares at the
        // very end.
        let hypothesis = vec![
            Turn::from_secs("X", 0.0, 10.6),
            Turn::from_secs("Y", 10.6, 12.0),
        ];

        let r = der(
            &reference,
            &hypothesis,
            &DerOptions {
                collar: secs_to_micros(0.5),
                ..DerOptions::default()
            },
        )
        .expect("scored");

        // X owns 10.6 s of the 12 s reference; the mapping must follow the audio.
        assert_eq!(r.mapping.get("A").map(String::as_str), Some("X"));
        // And the error is real: Y's stretch outside the collar is confusion.
        assert!(r.confusion > 0.0, "confusion was {}", r.confusion);
    }

    #[test]
    fn skip_overlap_removes_overlapped_regions_from_scoring() {
        let reference = vec![
            Turn::from_secs("A", 0.0, 10.0),
            Turn::from_secs("B", 0.0, 10.0),
            Turn::from_secs("A", 10.0, 20.0),
        ];
        let hypothesis = vec![Turn::from_secs("X", 0.0, 20.0)];

        let with_overlap = score(&reference, &hypothesis);
        let without = der(
            &reference,
            &hypothesis,
            &DerOptions {
                skip_overlap: true,
                ..DerOptions::default()
            },
        )
        .expect("scored");

        // Only the single-speaker stretch [10,20) survives, and it is correct.
        assert!(approx(without.total_reference, 10.0));
        assert!(approx(without.der, 0.0));
        assert!(
            without.der < with_overlap.der,
            "skipping overlap must not raise DER"
        );
    }

    // -- RTTM ---------------------------------------------------------------

    #[test]
    fn rttm_round_trips() {
        let turns = vec![
            Turn::from_secs("spk0", 0.5, 3.25),
            Turn::from_secs("spk1", 3.25, 7.125),
        ];
        let text = write_rttm("meeting1", &turns);
        let back = parse_rttm(&text, None).expect("parsed");
        assert_eq!(back.turns, turns);
        assert_eq!(back.file_id.as_deref(), Some("meeting1"));
    }

    #[test]
    fn rttm_ignores_comments_and_non_speaker_records() {
        let text = "\
;; a comment
SPKR-INFO meeting1 1 <NA> <NA> <NA> unknown spk0 <NA> <NA>
SPEAKER meeting1 1 0.000 2.500 <NA> <NA> spk0 <NA> <NA>

SPEAKER meeting1 1 2.500 1.000 <NA> <NA> spk1 <NA> <NA>
";
        let turns = parse_rttm(text, None).expect("parsed").turns;
        assert_eq!(turns.len(), 2);
        assert_eq!(turns[1].speaker, "spk1");
        assert_eq!(turns[1].start, secs_to_micros(2.5));
    }

    /// Silently skipping a broken line would shorten the reference and make the
    /// score look better.
    #[test]
    fn a_malformed_line_is_an_error_naming_its_line_number() {
        let text = "SPEAKER m 1 0.000 2.500 <NA> <NA> spk0 <NA> <NA>\nSPEAKER m 1 oops 1.0 <NA> <NA> spk1 <NA> <NA>\n";
        let e = parse_rttm(text, None).expect_err("must refuse");
        assert!(e.to_string().contains("satır 2"), "{e}");
    }

    /// A BOM is invisible in every editor and is exactly what `Set-Content
    /// -Encoding utf8` writes — the command this harness's own error messages
    /// recommend. Without stripping it, the first SPEAKER record fails the type
    /// match and is SKIPPED, silently shortening the reference.
    #[test]
    fn a_utf8_bom_does_not_swallow_the_first_record() {
        let body = "SPEAKER m 1 0.000 5.000 <NA> <NA> A <NA> <NA>\n\
                    SPEAKER m 1 5.000 5.000 <NA> <NA> B <NA> <NA>\n";
        let plain = parse_rttm(body, None).expect("parsed");
        let bommed = parse_rttm(&format!("\u{feff}{body}"), None).expect("parsed");
        assert_eq!(bommed.turns.len(), 2, "the BOM cost a turn");
        assert_eq!(bommed, plain, "a BOM must change nothing at all");

        // Same for the UEM parser, where a BOM would defeat the file-id filter.
        let uem = parse_uem("\u{feff}m 1 0.0 10.0\n", Some("m")).expect("parsed");
        assert_eq!(uem.len(), 1);
    }

    /// The float→int cast saturates, so without a range check an absurd time
    /// becomes `i64::MAX` and the turn disappears — a shorter reference and a
    /// better-looking score, with no error.
    #[test]
    fn an_out_of_range_time_is_an_error_not_a_vanished_turn() {
        assert!(checked_secs_to_micros(1e30).is_none());
        assert!(checked_secs_to_micros(f64::NAN).is_none());
        assert!(checked_secs_to_micros(f64::INFINITY).is_none());
        assert!(checked_secs_to_micros(3600.0).is_some());

        let e = parse_rttm("SPEAKER m 1 1e30 5.000 <NA> <NA> A <NA> <NA>\n", None)
            .expect_err("must refuse");
        assert!(e.to_string().contains("aralık dışı"), "{e}");
    }

    #[test]
    fn a_negative_start_time_is_an_error() {
        let e = parse_rttm("SPEAKER m 1 -1.000 5.000 <NA> <NA> A <NA> <NA>\n", None)
            .expect_err("must refuse");
        assert!(e.to_string().contains("negatif başlangıç"), "{e}");
    }

    #[test]
    fn a_truncated_line_is_an_error() {
        let text = "SPEAKER m 1 0.000 2.500\n";
        let e = parse_rttm(text, None).expect_err("must refuse");
        assert!(e.to_string().contains("en az 8"), "{e}");
    }

    /// Concatenating two meetings would produce a plausible, meaningless number.
    #[test]
    fn several_file_ids_without_a_selection_is_an_error() {
        let text = "\
SPEAKER meetingA 1 0.000 2.500 <NA> <NA> spk0 <NA> <NA>
SPEAKER meetingB 1 0.000 2.500 <NA> <NA> spk0 <NA> <NA>
";
        let e = parse_rttm(text, None).expect_err("must refuse");
        assert!(e.to_string().contains("farklı dosya kimliği"), "{e}");

        let just_a = parse_rttm(text, Some("meetingA")).expect("selection resolves it");
        assert_eq!(just_a.turns.len(), 1);
        assert_eq!(just_a.file_id.as_deref(), Some("meetingA"));
    }

    /// Rejecting several ids inside ONE file is not enough — two files each
    /// carrying one valid but DIFFERENT id would otherwise be scored happily.
    #[test]
    fn a_reference_and_hypothesis_for_different_recordings_are_refused() {
        let reference = parse_rttm(
            "SPEAKER meetingA 1 0.000 10.000 <NA> <NA> A <NA> <NA>\n",
            None,
        )
        .expect("parsed");
        let hypothesis = parse_rttm(
            "SPEAKER meetingB 1 0.000 10.000 <NA> <NA> X <NA> <NA>\n",
            None,
        )
        .expect("parsed");

        // The timelines line up exactly, so scoring would report a confident 0%.
        assert!(approx(
            der(&reference.turns, &hypothesis.turns, &DerOptions::default())
                .expect("would score")
                .der,
            0.0
        ));
        let e = ensure_same_recording(&reference, &hypothesis).expect_err("must refuse");
        assert!(e.to_string().contains("meetingA"), "{e}");
        assert!(e.to_string().contains("meetingB"), "{e}");

        // Same recording is fine.
        ensure_same_recording(&reference, &reference).expect("same id");
    }

    #[test]
    fn zero_length_turns_are_dropped_not_counted() {
        let text = "SPEAKER m 1 1.000 0.000 <NA> <NA> spk0 <NA> <NA>\n";
        assert!(parse_rttm(text, None).expect("parsed").turns.is_empty());
    }

    // -- Assignment ---------------------------------------------------------

    #[test]
    fn assignment_handles_rectangular_matrices_in_both_directions() {
        // 2 rows, 3 columns
        let w = vec![vec![1, 9, 2], vec![8, 1, 1]];
        let a = max_weight_assignment(&w);
        assert_eq!(a[0], Some(1));
        assert_eq!(a[1], Some(0));

        // 3 rows, 2 columns — one row must go unmatched
        let w = vec![vec![9, 1], vec![1, 8], vec![1, 1]];
        let a = max_weight_assignment(&w);
        assert_eq!(a[0], Some(0));
        assert_eq!(a[1], Some(1));
        assert_eq!(a[2], None);
    }

    /// The strongest evidence available without a third-party implementation:
    /// on hundreds of random matrices the solver must equal the value found by
    /// exhaustive search, which is obviously correct at this size.
    #[test]
    fn hungarian_matches_brute_force_on_random_matrices() {
        // Deterministic LCG — a failure must be reproducible.
        let mut seed: u64 = 0x2026_0808;
        let mut next = move || {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (seed >> 33) as i64
        };

        for case in 0..400 {
            let n = 1 + (case % 5);
            let m = 1 + ((case / 5) % 5);
            let w: Vec<Vec<i64>> = (0..n)
                .map(|_| (0..m).map(|_| next() % 100).collect())
                .collect();

            let got: i64 = max_weight_assignment(&w)
                .iter()
                .enumerate()
                .filter_map(|(r, c)| c.map(|c| w[r][c]))
                .sum();
            let want = brute_force_max(&w);
            assert_eq!(got, want, "case {case}, matrix {w:?}");
        }
    }

    /// Exhaustive maximum over all injective row→column assignments.
    fn brute_force_max(w: &[Vec<i64>]) -> i64 {
        fn go(w: &[Vec<i64>], row: usize, used: &mut Vec<bool>) -> i64 {
            if row == w.len() {
                return 0;
            }
            // A row may also stay unmatched, which matters when rows > columns.
            let mut best = go(w, row + 1, used);
            for c in 0..w[row].len() {
                if !used[c] {
                    used[c] = true;
                    best = best.max(w[row][c] + go(w, row + 1, used));
                    used[c] = false;
                }
            }
            best
        }
        let mut used = vec![false; w.first().map_or(0, |r| r.len())];
        go(w, 0, &mut used)
    }

    #[test]
    fn assignment_of_empty_inputs_is_empty() {
        assert!(max_weight_assignment(&[]).is_empty());
        assert_eq!(max_weight_assignment(&[vec![]]), vec![None]);
    }

    // -- Exactness ----------------------------------------------------------

    /// The reason times are integers: these boundaries are equal in exact
    /// arithmetic but not in f64, and an f64 implementation leaves a phantom
    /// sliver of "confusion" between them.
    #[test]
    fn floating_point_boundary_equality_does_not_leak_error() {
        let reference = vec![
            Turn::from_secs("A", 0.0, 0.1 + 0.2),
            Turn::from_secs("B", 0.3, 1.0),
        ];
        let hypothesis = vec![
            Turn::from_secs("X", 0.0, 0.3),
            Turn::from_secs("Y", 0.1 + 0.2, 1.0),
        ];
        let r = score(&reference, &hypothesis);
        assert!(approx(r.der, 0.0), "der was {}", r.der);
        assert!(approx(r.confusion, 0.0));
    }
}
