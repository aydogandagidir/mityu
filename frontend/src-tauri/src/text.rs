//! Turkish-aware text normalization — THE single definition.
//!
//! Moved here from `eval-harness/src/metrics.rs` (which now imports it) when the
//! learning miner needed the same folding. Two consumers, one implementation: a
//! second Turkish lowercase would be a second answer to "are these the same
//! word?", and the two would drift the first time either was touched. The
//! direction is forced anyway — `eval-harness` depends on this crate, never the
//! reverse.
//!
//! Pipeline: Unicode NFC → Turkish-aware lowercase (`I`→`ı`, `İ`→`i`, the rest
//! via `char::to_lowercase`) → apostrophes deleted (`Ankara'da` → `ankarada`),
//! other punctuation → space → whitespace collapsed.
//!
//! **`strict` vs `folded` is a real choice, and the two consumers make it
//! differently.** The eval harness reports both, because Turkish diacritic loss
//! ("konveyor arizali" for "konveyör arızalı") is an orthographic error, not a
//! lexical one, and a human weighs which number matters. The learning miner
//! (ADR-0024 §8) must use `strict` ONLY: a user who corrects "konveyor" to
//! "konveyör" is teaching exactly the distinction folding erases, and a folded
//! miner would see no change at all and learn nothing.

use unicode_normalization::UnicodeNormalization;

/// Both normalization variants of one text.
pub struct Normalized {
    /// Diacritics preserved. Use this when the diacritic is part of the meaning
    /// — including any comparison the learning miner makes.
    pub strict: String,
    /// Diacritics folded. Use this when identity matters and orthography does
    /// not (term matching, the folded WER/CER variants).
    pub folded: String,
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
///
/// Here beside `normalize` for the same reason it is: two consumers, one
/// implementation. The eval harness measures how far a transcript is from a
/// human reference; the learning system measures how far a draft is from what
/// the human made of it (ADR-0024 §9). Same question, same answer.
pub fn levenshtein<T: PartialEq>(a: &[T], b: &[T]) -> usize {
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

/// Word-level distance between two texts, normalized to `0.0..=1.0` against the
/// LONGER side.
///
/// `0.0` = identical, `1.0` = nothing in common. Dividing by the longer side —
/// rather than by the reference, as WER does — is what bounds this at 1.0, and
/// the bound is the point: this feeds an average that a user reads as a
/// percentage (ADR-0024 §9), and WER's ability to exceed 1.0 when the hypothesis
/// runs long would let one verbose edit drag a whole average past 100%.
pub fn normalized_word_distance(a: &str, b: &str) -> f64 {
    let aw: Vec<&str> = a.split_whitespace().collect();
    let bw: Vec<&str> = b.split_whitespace().collect();
    let longer = aw.len().max(bw.len());
    if longer == 0 {
        return 0.0;
    }
    levenshtein(&aw, &bw) as f64 / longer as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn turkish_casing_is_not_ascii_casing() {
        assert_eq!(
            normalize("İSTANBUL Iğdır ILIK").strict,
            "istanbul ığdır ılık"
        );
    }

    #[test]
    fn the_suffix_apostrophe_disappears_and_other_punctuation_becomes_space() {
        assert_eq!(normalize("Ankara'da, hızlı!").strict, "ankarada hızlı");
        assert_eq!(normalize("  a   b  ").strict, "a b");
    }

    #[test]
    fn folding_erases_the_diacritics_that_strict_keeps() {
        let n = normalize("konveyör arızalı");
        assert_eq!(n.strict, "konveyör arızalı");
        assert_eq!(n.folded, "konveyor arizali");
    }

    /// The property the learning miner depends on: a diacritic-only correction is
    /// INVISIBLE to `folded` and visible to `strict`. If this ever stops holding,
    /// the miner silently stops learning that whole class of correction.
    #[test]
    fn a_diacritic_only_correction_survives_strict_and_vanishes_when_folded() {
        let model = normalize("konveyor");
        let human = normalize("konveyör");
        assert_ne!(model.strict, human.strict, "strict must see the correction");
        assert_eq!(
            model.folded, human.folded,
            "folded cannot see it — hence strict"
        );
    }
}
