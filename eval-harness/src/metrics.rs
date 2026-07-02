//! WER / CER / jargon term-recall with Turkish-aware normalization.
//!
//! Documented decisions (surfaced in the Phase-0 report):
//! - Normalization pipeline: Unicode NFC → Turkish-aware lowercase (`I`→`ı`,
//!   `İ`→`i`, everything else via `char::to_lowercase`) → apostrophes deleted
//!   (`Ankara'da`→`ankarada`), all other punctuation mapped to a space →
//!   whitespace collapsed.
//! - Diacritic folding is NOT applied to the strict metrics. A folded variant
//!   (`ç`→`c`, `ğ`→`g`, `ı`→`i`, `ö`→`o`, `ş`→`s`, `ü`→`u`, `â`→`a`, … via NFD
//!   combining-mark stripping) is reported alongside, because Turkish diacritic
//!   loss ("konveyor arizali" for "konveyör arızalı") is an orthographic — not
//!   lexical — error. Both numbers go into the report; the human weighs them.
//! - Term recall uses folded matching (term identity matters, not orthography)
//!   and plain substring containment on the normalized text, so a term can
//!   match inside a longer word (e.g. "dok" in "doktor") — acceptable for a
//!   curated jargon list, noted here for transparency.
//! - WER convention: distance / reference-word-count; empty reference scores
//!   0.0 against an empty hypothesis and 1.0 otherwise; values may exceed 1.0
//!   when the hypothesis inserts many extra words.

use unicode_normalization::UnicodeNormalization;

/// Both normalization variants of one text.
pub struct Normalized {
    /// Diacritics preserved (strict metrics).
    pub strict: String,
    /// Diacritics folded (folded metrics + term matching).
    pub folded: String,
}

/// Per-clip scores for one (reference, hypothesis) pair.
pub struct Score {
    pub wer: f64,
    pub wer_folded: f64,
    pub cer: f64,
    pub cer_folded: f64,
    /// `None` when no jargon term occurs in the reference.
    pub term_recall: Option<f64>,
}

const APOSTROPHES: [char; 5] = ['\'', '\u{2019}', '\u{2018}', '\u{02BC}', '\u{00B4}'];

/// Normalize raw text into strict + diacritic-folded forms.
pub fn normalize(text: &str) -> Normalized {
    let nfc: String = text.nfc().collect();
    let lowered = tr_lowercase(&nfc);
    let stripped = strip_punct(&lowered);
    let strict = collapse_ws(&stripped);
    let folded = fold_diacritics(&strict);
    Normalized { strict, folded }
}

/// Score a hypothesis against a human reference. `jargon_folded` must already
/// be normalized+folded (see `normalize(term).folded`).
pub fn score(reference_raw: &str, hypothesis_raw: &str, jargon_folded: &[String]) -> Score {
    let r = normalize(reference_raw);
    let h = normalize(hypothesis_raw);
    Score {
        wer: wer(&r.strict, &h.strict),
        wer_folded: wer(&r.folded, &h.folded),
        cer: cer(&r.strict, &h.strict),
        cer_folded: cer(&r.folded, &h.folded),
        term_recall: term_recall(&h.folded, &r.folded, jargon_folded),
    }
}

/// Word error rate on pre-normalized text.
pub fn wer(reference: &str, hypothesis: &str) -> f64 {
    let r: Vec<&str> = reference.split_whitespace().collect();
    let h: Vec<&str> = hypothesis.split_whitespace().collect();
    ratio(levenshtein(&r, &h), r.len(), h.len())
}

/// Character error rate on pre-normalized text (single spaces included).
pub fn cer(reference: &str, hypothesis: &str) -> f64 {
    let r: Vec<char> = reference.chars().collect();
    let h: Vec<char> = hypothesis.chars().collect();
    ratio(levenshtein(&r, &h), r.len(), h.len())
}

/// Fraction of jargon terms present in the reference that also appear in the
/// hypothesis (folded substring matching). `None` if no term is in the reference.
pub fn term_recall(hyp_folded: &str, ref_folded: &str, terms_folded: &[String]) -> Option<f64> {
    let present: Vec<&String> = terms_folded
        .iter()
        .filter(|t| !t.is_empty() && ref_folded.contains(t.as_str()))
        .collect();
    if present.is_empty() {
        return None;
    }
    let hits = present
        .iter()
        .filter(|t| hyp_folded.contains(t.as_str()))
        .count();
    Some(hits as f64 / present.len() as f64)
}

fn ratio(dist: usize, ref_len: usize, hyp_len: usize) -> f64 {
    if ref_len == 0 {
        return if hyp_len == 0 { 0.0 } else { 1.0 };
    }
    dist as f64 / ref_len as f64
}

/// Turkish-aware lowercasing: `I`→`ı`, `İ`→`i` (explicit map), rest via Unicode.
fn tr_lowercase(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            'I' => out.push('ı'),
            'İ' => out.push('i'),
            _ => out.extend(ch.to_lowercase()),
        }
    }
    out
}

/// Apostrophes deleted (Turkish suffix apostrophe), other punctuation → space.
fn strip_punct(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        if APOSTROPHES.contains(&ch) {
            // drop entirely: "Ankara'da" → "ankarada"
        } else if ch.is_alphanumeric() || ch.is_whitespace() {
            out.push(ch);
        } else {
            out.push(' ');
        }
    }
    out
}

fn collapse_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Fold diacritics: NFD-decompose, drop combining marks (U+0300–U+036F), and
/// map the dotless `ı` (which has no decomposition) to `i`. Covers Turkish
/// ç/ğ/ı/ö/ş/ü and circumflexed â/î/û.
fn fold_diacritics(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.nfd() {
        match ch {
            '\u{0300}'..='\u{036F}' => {}
            'ı' => out.push('i'),
            _ => out.push(ch),
        }
    }
    out
}

/// Levenshtein distance (two-row DP) over any comparable token slice.
fn levenshtein<T: PartialEq>(a: &[T], b: &[T]) -> usize {
    if a.is_empty() {
        return b.len();
    }
    if b.is_empty() {
        return a.len();
    }
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut curr = vec![0usize; b.len() + 1];
    for (i, ai) in a.iter().enumerate() {
        curr[0] = i + 1;
        for (j, bj) in b.iter().enumerate() {
            let cost = usize::from(ai != bj);
            curr[j + 1] = (prev[j] + cost).min(prev[j + 1] + 1).min(curr[j] + 1);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn turkish_dotted_dotless_i_lowercase() {
        assert_eq!(
            normalize("İSTANBUL Iğdır ILIK").strict,
            "istanbul ığdır ılık"
        );
    }

    #[test]
    fn nfc_unifies_decomposed_forms() {
        // "kök" written with a decomposed o + combining diaeresis
        let decomposed = "ko\u{0308}k";
        assert_eq!(normalize(decomposed).strict, normalize("kök").strict);
    }

    #[test]
    fn punctuation_and_whitespace_rules() {
        assert_eq!(normalize("Merhaba,   dünya!!").strict, "merhaba dünya");
        // apostrophe joins (TR suffix), slash splits
        assert_eq!(
            normalize("Ankara'da AS/RS çalışıyor.").strict,
            "ankarada as rs çalışıyor"
        );
        assert_eq!(
            normalize("pick-to-light\tsistemi\n").strict,
            "pick to light sistemi"
        );
    }

    #[test]
    fn folding_maps_turkish_diacritics() {
        assert_eq!(normalize("çğışöüâî").folded, "cgisouai");
        assert_eq!(normalize("ÇĞİŞÖÜ").folded, "cgisou");
    }

    #[test]
    fn wer_basics() {
        assert!(approx(wer("a b c", "a b c"), 0.0));
        assert!(approx(wer("a b c", "a x c"), 1.0 / 3.0)); // substitution
        assert!(approx(wer("a b c", "a c"), 1.0 / 3.0)); // deletion
        assert!(approx(wer("a b", "a x b y"), 1.0)); // 2 insertions / 2 ref words
        assert!(approx(wer("", ""), 0.0));
        assert!(approx(wer("", "a"), 1.0));
        assert!(approx(wer("a", "a b c"), 2.0)); // WER may exceed 1.0
    }

    #[test]
    fn cer_basics() {
        assert!(approx(cer("abc", "abc"), 0.0));
        assert!(approx(cer("abc", "abd"), 1.0 / 3.0));
    }

    #[test]
    fn known_answer_turkish_diacritic_folding() {
        // Decision under test: strict metrics keep diacritics (count the error),
        // folded metrics neutralize them. "KONVEYÖR arızalı" vs "konveyor arizali".
        let s = score("KONVEYÖR arızalı", "konveyor arizali", &[]);
        assert!(approx(s.wer, 1.0)); // both words differ strictly
        assert!(approx(s.wer_folded, 0.0)); // identical once folded
        assert!(s.cer > 0.0);
        assert!(approx(s.cer_folded, 0.0));
        assert!(s.term_recall.is_none()); // no jargon list supplied
    }

    #[test]
    fn term_recall_uses_folded_matching() {
        let jargon: Vec<String> = ["konveyör", "PLC", "forklift"]
            .iter()
            .map(|t| normalize(t).folded)
            .collect();
        // ref contains konveyör + PLC; hyp keeps konveyor (diacritics lost) but drops PLC
        let s = score(
            "Konveyör hattı durdu, PLC hata verdi",
            "konveyor hatti durdu, hata verdi",
            &jargon,
        );
        assert!(approx(s.term_recall.expect("terms present in ref"), 0.5));
        // no jargon term in ref → None
        let s2 = score("bugün hava güzel", "bugün hava güzel", &jargon);
        assert!(s2.term_recall.is_none());
    }
}
