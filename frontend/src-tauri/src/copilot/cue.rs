//! Deterministic conversational cues over transcript text (BACKLOG I2).
//!
//! A *cue* is the copilot noticing that something was just asked or requested
//! — the moment a live assistant might have something useful to offer. In I2
//! the cue is only an event (`copilot-cue`); nothing acts on it. In I3 it will
//! light an on-demand button; in I8 it becomes the **prefilter** in front of an
//! opt-in "should I offer?" judge, which is why it must be cheap, boring and
//! predictable rather than clever.
//!
//! Three properties are load-bearing and each has a test:
//!
//! - **Deterministic.** Same text, same answer, every time, on every machine.
//!   No model, no randomness, no clock, no locale lookup. A cue that fires
//!   differently on the reviewer's laptop than on the user's is a cue nobody
//!   can debug.
//! - **Offline and side-effect free.** Pure functions over a `&str`. Nothing
//!   here reads a file, touches the network or logs the text it was given —
//!   transcript text never reaches logs (ADR-0038 invariant 3).
//! - **Biased toward silence.** A missed question costs a button that did not
//!   light up. A false cue, once I8 exists, costs an interruption in a meeting.
//!   Signals therefore add up to a confidence and only a clear one crosses
//!   [`CUE_THRESHOLD`]; a lone weak hint does not.
//!
//! ## Why Turkish *and* English, and only those
//!
//! They are the two languages the product is validated against (PHASE0, A5).
//! A question is marked very differently in each: English inverts the verb
//! ("can you…", "is it…") or fronts a wh-word; Turkish attaches the particle
//! *mı/mi/mu/mü* — written as its own word, conjugated ("misiniz", "mıyız") —
//! and keeps the wh-word wherever it likes ("bunu **ne zaman** teslim
//! edersiniz"). Whisper drops the question mark often enough that punctuation
//! alone would miss too much, so both grammars are read explicitly. A third
//! language is a lexicon, not a redesign.
//!
//! ## What this deliberately is not
//!
//! Not intent classification, not sentiment, not "coaching", not a speaker
//! model. It reads words. ADR-0034 keeps voice-based inference out of the
//! product and ADR-0038 keeps emotion/engagement scoring out of the copilot;
//! neither is approached here.

use serde::{Deserialize, Serialize};

/// A cue is emitted only when the combined signal reaches this. Chosen so that
/// a bare question mark, a Turkish question particle, or a sentence-initial
/// wh-word each *just* qualifies on its own, while weaker single hints (a
/// sentence-initial auxiliary, a request verb) need a second signal.
pub const CUE_THRESHOLD: f32 = 0.40;

/// What kind of moment the text marks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CueKind {
    /// Someone asked something.
    Question,
    /// Someone asked *for* something — "can you send", "lütfen gönderin".
    Request,
}

/// The detector's verdict on one piece of text.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Detection {
    pub kind: CueKind,
    /// In `0.0..=1.0`. Always `>= CUE_THRESHOLD` when a detection is returned.
    pub confidence: f32,
}

/// Read one utterance and decide whether it is a question or a request.
///
/// Returns `None` for anything that does not clear [`CUE_THRESHOLD`] — the
/// caller must treat that as "nothing happened", not as a weak cue.
pub fn detect(text: &str) -> Option<Detection> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }

    let normalised = normalise(text);
    let tokens: Vec<&str> = tokens(&normalised);
    if tokens.is_empty() {
        return None;
    }

    // Each signal is a fact about the sentence; the score is their sum.
    let question_mark = ends_with_question_mark(text);
    let tr_wh = has_turkish_interrogative(&tokens);
    let tr_particle = tokens.iter().any(|t| is_turkish_question_particle(t));
    let (en_wh, en_inversion) = english_sentence_opening(&tokens);
    let request = has_request_cue(&tokens, &normalised);

    let mut score = 0.0_f32;
    if question_mark {
        score += 0.50;
    }
    if tr_wh {
        score += 0.40;
    }
    if tr_particle {
        score += 0.40;
    }
    if en_wh {
        score += 0.40;
    }
    if en_inversion {
        score += 0.40;
    }
    if request {
        score += 0.40;
    }
    let confidence = score.min(1.0);

    if confidence < CUE_THRESHOLD {
        return None;
    }

    // A request that is *phrased* as a question ("can you send it?") is still a
    // request: the useful reaction is to the thing asked for. Only a wh-word or
    // a Turkish particle — an actual information question — outranks it.
    let kind = if request && !tr_wh && !en_wh && !tr_particle {
        CueKind::Request
    } else {
        CueKind::Question
    };

    Some(Detection { kind, confidence })
}

/// Lower-case the text in a way that works for both alphabets.
///
/// `str::to_lowercase` maps the Turkish dotted capital `İ` to `i` followed by
/// U+0307 (combining dot above), which would make "İyi misiniz" fail to match
/// the token "misiniz"'s neighbours and, worse, make any lexicon entry that
/// happens to start with `i` unreachable from capitalised speech. Stripping the
/// combining mark restores the plain `i`. The dotless `ı` lower-cases to
/// itself and needs no help.
fn normalise(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .filter(|c| *c != '\u{0307}')
        .collect()
}

/// Whitespace tokens with punctuation trimmed off both ends. Apostrophes stay
/// inside a token ("what's", "Ankara'ya") but are trimmed at the edges.
fn tokens(normalised: &str) -> Vec<&str> {
    normalised
        .split(char::is_whitespace)
        .map(|t| t.trim_matches(|c: char| !c.is_alphanumeric()))
        .filter(|t| !t.is_empty())
        .collect()
}

/// `?` at the very end, tolerating a closing quote, bracket or `!` after it.
fn ends_with_question_mark(text: &str) -> bool {
    text.trim_end_matches(['!', ')', '"', '\'', '”', '’'])
        .ends_with('?')
}

// --- Turkish -----------------------------------------------------------------

/// Turkish wh-words. Position does not matter in Turkish, so these are matched
/// anywhere. `ne` alone is deliberately handled with an exception list because
/// it also opens idioms that are not questions.
const TR_INTERROGATIVES: &[&str] = &[
    "neden",
    "niye",
    "niçin",
    "nasıl",
    "nerede",
    "nereye",
    "nereden",
    "neresi",
    "nereli",
    "kim",
    "kimi",
    "kime",
    "kimin",
    "kimle",
    "kiminle",
    "hangi",
    "hangisi",
    "hangisini",
    "kaç",
    "kaçta",
    "kaça",
    "kaçıncı",
];

/// `ne` followed by one of these is an idiom or an exclamation, not a question:
/// "ne yazık ki", "ne güzel", "ne de olsa", "ne var ki".
const TR_NE_NOT_A_QUESTION: &[&str] = &["yazık", "güzel", "de", "var", "hoş", "kadar", "olsa"];

fn has_turkish_interrogative(tokens: &[&str]) -> bool {
    for (i, token) in tokens.iter().enumerate() {
        if TR_INTERROGATIVES.contains(token) {
            return true;
        }
        if *token == "ne" {
            match tokens.get(i + 1) {
                // "ne zaman", "ne kadar sürer", "ne için": questions.
                Some(next) if matches!(*next, "zaman" | "için" | "zamandır") => return true,
                // Idioms and exclamations.
                Some(next) if TR_NE_NOT_A_QUESTION.contains(next) => continue,
                // "ne" as the last word ("bu ne") or before an ordinary word
                // ("ne düşünüyorsun"): a question.
                _ => return true,
            }
        }
    }
    false
}

/// The Turkish question particle in every spelling the vowel harmony and the
/// personal endings produce: `mı mi mu mü`, `mısın misin musun müsün`,
/// `mıyız miyiz muyuz müyüz`, `mısınız misiniz musunuz müsünüz`, `mıdır midir
/// mudur`, `mıydı miydi muydu müydü`, `mıymış miymiş muymuş müymüş`, plus the
/// past/first-person forms (`mıydım`, `miydin`, …).
///
/// `müdür` is deliberately **absent**: as a particle it is formal and rare in
/// speech, while as a noun it means "manager" and turns up in most business
/// meetings. Reading it as a question would fire on every mention of the boss.
fn is_turkish_question_particle(token: &str) -> bool {
    let mut chars = token.chars();
    if chars.next() != Some('m') {
        return false;
    }
    let Some(vowel) = chars.next() else {
        return false;
    };
    if !matches!(vowel, 'ı' | 'i' | 'u' | 'ü') {
        return false;
    }
    let rest: String = chars.collect();
    const ENDINGS: &[&str] = &[
        "", "sın", "sin", "sun", "sün", "yız", "yiz", "yuz", "yüz", "sınız", "siniz", "sunuz",
        "sünüz", "dır", "dir", "dur", "ydı", "ydi", "ydu", "ydü", "ydım", "ydim", "ydum", "ydüm",
        "ydın", "ydin", "ydun", "ydün", "ydık", "ydik", "yduk", "ydük", "ymış", "ymiş", "ymuş",
        "ymüş", "yım", "yim", "yum", "yüm",
    ];
    ENDINGS.contains(&rest.as_str())
}

// --- English -----------------------------------------------------------------

const EN_FILLERS: &[&str] = &[
    "so", "and", "but", "okay", "ok", "well", "um", "uh", "now", "then", "also", "hey", "oh",
    "right", "yeah", "yes", "no", "alright",
];

const EN_WH: &[&str] = &[
    "what", "why", "how", "when", "where", "who", "whom", "whose", "which",
];

const EN_AUXILIARIES: &[&str] = &[
    "can", "could", "would", "will", "should", "shall", "may", "might", "do", "does", "did", "is",
    "are", "was", "were", "have", "has", "had", "am",
];

const EN_SUBJECTS: &[&str] = &[
    "you", "we", "i", "it", "they", "he", "she", "this", "that", "there", "anyone", "anybody",
    "everyone", "someone", "the",
];

/// English marks a question at the front of the sentence, so only the opening
/// is read: a wh-word, or an auxiliary immediately followed by a subject (the
/// inversion "can you", "is it", "did they"). Leading fillers are skipped so
/// "so, can you…" still counts.
fn english_sentence_opening(tokens: &[&str]) -> (bool, bool) {
    let start = tokens
        .iter()
        .position(|t| !EN_FILLERS.contains(t))
        .unwrap_or(tokens.len());
    let Some(first) = tokens.get(start) else {
        return (false, false);
    };
    // "what's", "how's", "where's".
    let first_base = first.split('\'').next().unwrap_or(first);
    let wh = EN_WH.contains(&first_base);
    let inversion = EN_AUXILIARIES.contains(first)
        && tokens
            .get(start + 1)
            .is_some_and(|next| EN_SUBJECTS.contains(next));
    (wh, inversion)
}

// --- Requests (both languages) ------------------------------------------------

/// Phrases that ask for something. Matched on the normalised text so
/// multi-word forms are one entry.
///
/// Verbs on their own ("share", "send") are **not** here: "I will share the
/// notes" is a commitment, the opposite of a request, and the verb cannot tell
/// the two apart. The entries either address the listener ("can you", "send
/// me") or are explicit politeness markers ("please", "lütfen").
const REQUEST_PHRASES: &[&str] = &[
    // English
    "can you",
    "could you",
    "would you",
    "will you",
    "please",
    "let me know",
    "send me",
    "send us",
    "walk me through",
    "tell me",
    "show me",
    "remind me",
    "make sure",
    // Turkish
    "lütfen",
    "rica ederim",
    "rica edeyim",
    "rica etsem",
    "gönderebilir",
    "gönderir misin",
    "gönderir misiniz",
    "paylaşabilir",
    "paylaşır mısın",
    "paylaşır mısınız",
    "iletebilir",
    "bakabilir",
    "söyleyebilir",
    "anlatabilir",
    "açıklayabilir",
    "yapabilir",
    "hatırlatır",
];

fn has_request_cue(tokens: &[&str], normalised: &str) -> bool {
    // Single-word entries must match a whole token, not a substring of one:
    // "please" must not fire on "pleased", "lütfen" has no such neighbour but
    // gets the same treatment for consistency.
    REQUEST_PHRASES.iter().any(|phrase| {
        if phrase.contains(' ') {
            contains_phrase(normalised, phrase)
        } else {
            tokens.contains(phrase)
        }
    })
}

/// Whole-word phrase containment on the normalised sentence.
fn contains_phrase(haystack: &str, phrase: &str) -> bool {
    let mut from = 0;
    while let Some(pos) = haystack[from..].find(phrase) {
        let start = from + pos;
        let end = start + phrase.len();
        let before_ok = start == 0
            || !haystack[..start]
                .chars()
                .next_back()
                .is_some_and(char::is_alphanumeric);
        let after_ok = end == haystack.len()
            || !haystack[end..]
                .chars()
                .next()
                .is_some_and(char::is_alphanumeric);
        if before_ok && after_ok {
            return true;
        }
        from = start + phrase.len();
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kind_of(text: &str) -> Option<CueKind> {
        detect(text).map(|d| d.kind)
    }

    // --- Turkish -------------------------------------------------------------

    #[test]
    fn a_turkish_question_with_a_question_mark_is_a_question() {
        let d = detect("Bunu ne zaman teslim edebilirsiniz?").expect("cue");
        assert_eq!(d.kind, CueKind::Question);
        assert!(d.confidence > 0.8, "{}", d.confidence);
    }

    #[test]
    fn the_turkish_particle_is_enough_without_punctuation() {
        // Whisper often drops the question mark; the particle alone must carry it.
        assert_eq!(
            kind_of("toplantıyı cuma günü yapabilir miyiz"),
            Some(CueKind::Question)
        );
        assert_eq!(kind_of("Bu doğru mu"), Some(CueKind::Question));
        assert_eq!(kind_of("Hazır mısınız"), Some(CueKind::Question));
    }

    #[test]
    fn a_turkish_wh_word_anywhere_in_the_sentence_counts() {
        assert_eq!(kind_of("Bu işi nasıl çözeceğiz"), Some(CueKind::Question));
        assert_eq!(kind_of("Denetimi kim yapacak"), Some(CueKind::Question));
        assert_eq!(kind_of("Ekipte kaç kişi var"), Some(CueKind::Question));
    }

    #[test]
    fn turkish_idioms_built_on_ne_are_not_questions() {
        assert_eq!(kind_of("Ne yazık ki toplantıya gelemedi."), None);
        assert_eq!(kind_of("Ne güzel bir gün."), None);
        assert_eq!(kind_of("Ne de olsa ilk denememiz."), None);
    }

    #[test]
    fn a_turkish_request_is_a_request() {
        assert_eq!(
            kind_of("Raporu lütfen akşama kadar gönderin."),
            Some(CueKind::Request)
        );
    }

    #[test]
    fn a_turkish_statement_is_silence() {
        assert_eq!(detect("Pilot Ankara sahasında başlayacak."), None);
        assert_eq!(detect("İkinci saha denetim kapandıktan sonra gelir."), None);
    }

    #[test]
    fn a_capitalised_dotted_i_still_matches() {
        // `İ`.to_lowercase() is `i` + U+0307; without normalisation "misiniz"
        // is fine but any `i`-initial lexicon entry would not be. This pins the
        // whole sentence, capital and all.
        assert_eq!(kind_of("İYİ MİSİNİZ"), Some(CueKind::Question));
    }

    // --- English -------------------------------------------------------------

    #[test]
    fn an_english_wh_question_is_a_question() {
        let d = detect("What time works for you?").expect("cue");
        assert_eq!(d.kind, CueKind::Question);
        assert!(d.confidence > 0.8);
        assert_eq!(
            kind_of("what's the timeline on this"),
            Some(CueKind::Question)
        );
    }

    #[test]
    fn english_inversion_counts_without_punctuation() {
        assert_eq!(kind_of("is it ready"), Some(CueKind::Question));
        assert_eq!(
            kind_of("So, did they sign the contract"),
            Some(CueKind::Question)
        );
    }

    #[test]
    fn an_english_request_phrased_as_a_question_is_a_request() {
        assert_eq!(
            kind_of("Can you send me the retention policy?"),
            Some(CueKind::Request)
        );
        assert_eq!(
            kind_of("could you share the deck after the call"),
            Some(CueKind::Request)
        );
    }

    #[test]
    fn please_alone_makes_a_request() {
        assert_eq!(
            kind_of("Please share the deck after the call."),
            Some(CueKind::Request)
        );
    }

    #[test]
    fn an_information_question_outranks_a_request_phrase() {
        // "tell me" is a request cue, but the wh-word says information is wanted.
        assert_eq!(
            kind_of("What can you tell me about the audit?"),
            Some(CueKind::Question)
        );
    }

    #[test]
    fn an_english_statement_is_silence() {
        assert_eq!(
            detect("We agreed the pilot starts in the Ankara site first."),
            None
        );
        assert_eq!(
            detect("The second site follows once the audit closes."),
            None
        );
        // "pleased" must not match "please".
        assert_eq!(detect("They were pleased with the results."), None);
        // An auxiliary not at the front is not an inversion.
        assert_eq!(detect("The report is in the shared folder."), None);
    }

    // --- Properties ----------------------------------------------------------

    #[test]
    fn empty_and_punctuation_only_text_is_silence() {
        assert_eq!(detect(""), None);
        assert_eq!(detect("   "), None);
        assert_eq!(detect("..."), None);
        // A bare question mark is a question mark on nothing.
        assert_eq!(detect("?"), None);
    }

    #[test]
    fn detection_is_deterministic() {
        let corpus = [
            "Bunu ne zaman teslim edebilirsiniz?",
            "toplantıyı cuma günü yapabilir miyiz",
            "What time works for you?",
            "We agreed the pilot starts in Ankara.",
        ];
        for text in corpus {
            assert_eq!(detect(text), detect(text), "{text}");
        }
    }

    #[test]
    fn confidence_stays_in_range_and_above_threshold_when_returned() {
        let corpus = [
            "Bunu ne zaman teslim edebilirsiniz?",
            "Hazır mısınız",
            "Can you send me the retention policy?",
            "What can you tell me about the audit? Please?",
            "is it ready",
            "Please share the deck.",
        ];
        for text in corpus {
            let d = detect(text).unwrap_or_else(|| panic!("expected a cue for {text:?}"));
            assert!(
                (CUE_THRESHOLD..=1.0).contains(&d.confidence),
                "{text}: {}",
                d.confidence
            );
        }
    }

    #[test]
    fn a_first_person_commitment_is_silence() {
        // "will" mid-sentence is not a question opener, and "share" is a verb,
        // not a request: this is someone promising, not asking.
        assert_eq!(detect("I will share the notes later."), None);
        assert_eq!(detect("I'll send the deck after the call."), None);
    }

    #[test]
    fn a_bare_request_verb_just_clears_the_threshold() {
        // Pinned on purpose: this is the weakest thing that fires. If the
        // threshold or the request weight moves, this test says so.
        assert_eq!(
            detect("Gönderebilir."),
            Some(Detection {
                kind: CueKind::Request,
                confidence: CUE_THRESHOLD
            })
        );
    }

    #[test]
    fn the_particle_recogniser_is_exact() {
        for ok in [
            "mı",
            "mi",
            "mu",
            "mü",
            "misin",
            "mısınız",
            "muyuz",
            "mudur",
            "miydi",
            "mıymış",
        ] {
            assert!(is_turkish_question_particle(ok), "{ok}");
        }
        // Ordinary words that start like the particle, and the one particle
        // form that is also a common noun.
        for no in [
            "mimar",
            "müdürlük",
            "mide",
            "m",
            "ma",
            "mit",
            "misafir",
            "mum",
            "mısır",
            "müdür",
        ] {
            assert!(!is_turkish_question_particle(no), "{no}");
        }
    }

    #[test]
    fn mentioning_the_manager_is_not_a_question() {
        assert_eq!(detect("Müdür toplantıya katılacak."), None);
        assert_eq!(detect("Bunu müdür onayladı"), None);
    }
}
