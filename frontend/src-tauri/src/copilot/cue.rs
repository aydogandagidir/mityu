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
//! ## Known gaps, recorded rather than papered over
//!
//! A lexicon cannot parse, and two classes of error survive on purpose because
//! every cheap fix for them costs a true positive:
//!
//! - **English subjunctive conditionals read as inversions**: "Had we known the
//!   budget was cut we would have planned differently", "Should the vendor miss
//!   the deadline we escalate", "Were it not for the delay we would be done"
//!   each score 0.40. Suppressing `had`/`were`/`should` openings would also
//!   silence "Had you seen the report", "Were you able to finish" and "Should
//!   we start" — real questions Whisper routinely emits without a question
//!   mark.
//! - **A bare-noun or proper-noun subject silences a plain question**: "Is
//!   Ahmet joining the call", "Has marketing approved this". [`EN_SUBJECTS`] is
//!   a closed list; opening it to any token would make every "is the" statement
//!   a question.
//!
//! Both are left missing rather than guessed, which is the same trade the
//! "biased toward silence" property makes everywhere else. I8's judge sees the
//! utterance itself and is the right place to resolve them.
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
    let request = has_request_cue(&tokens);

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
    // "nasıl" is handled in `has_turkish_interrogative` (it needs a look-ahead
    // for "nasıl olsa"), deliberately not listed here.
    "nerede",
    "nereye",
    "nereden",
    "neresi",
    "nereli",
    "kim",
    // "kimi" is deliberately absent: as a determiner it means "some" ("kimi
    // günler geç kalıyoruz" — "some days we run late"), and that reading is
    // commoner in speech than the accusative "whom". The unambiguous cases
    // ("kime", "kimin", "kimle", "kiminle", "kim") carry the question.
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

/// `ne` followed by one of these is an idiom, an exclamation or a concessive,
/// not a question: "ne yazık ki", "ne güzel", "ne de olsa", "ne var ki",
/// "ne olursa olsun", "ne yaparsak yapalım".
///
/// `"kadar"` is deliberately **not** here — see [`has_turkish_interrogative`].
const TR_NE_NOT_A_QUESTION: &[&str] = &[
    "yazık", "güzel", "de", "var", "hoş", "olsa", "olursa", "yaparsak",
];

/// "ne kadar" + one of these two tokens later is an exclamation, not a
/// quantity question: "ne kadar güzel bir sunum" — "what a beautiful
/// presentation".
const TR_NE_KADAR_EXCLAMATION: &[&str] = &["güzel", "hoş", "çok", "yazık", "komik", "iyi"];

/// Is this an interrogative use of a Turkish wh-word?
///
/// Turkish puts the wh-word wherever the sentence wants it, so position carries
/// no information and every token is checked. The three exceptions are bigrams
/// where the same word opens something that is not a question:
///
/// - **"ne kadar"** is the commonest quantity/duration question there is ("bu
///   ne kadar sürer"), so it must fire — but "her ne kadar …" is the concessive
///   "although", and "ne kadar güzel …" is an exclamation. Both are excluded by
///   looking one token back and two tokens forward.
/// - **"ne olursa olsun" / "ne yaparsak yapalım"** are "whatever happens" /
///   "whatever we do": concessives, listed above.
/// - **"nasıl olsa"** is "in any case", not "how".
fn has_turkish_interrogative(tokens: &[&str]) -> bool {
    for (i, token) in tokens.iter().enumerate() {
        if *token == "nasıl" {
            // "nasıl olsa hallederiz" — "we'll manage anyway".
            if tokens.get(i + 1) == Some(&"olsa") {
                continue;
            }
            return true;
        }
        if TR_INTERROGATIVES.contains(token) {
            return true;
        }
        if *token == "ne" {
            match tokens.get(i + 1) {
                Some(&"kadar") => {
                    // "her ne kadar …": although.
                    if i > 0 && tokens[i - 1] == "her" {
                        continue;
                    }
                    // "ne kadar güzel …": an exclamation, not a quantity.
                    if tokens
                        .get(i + 2)
                        .is_some_and(|w| TR_NE_KADAR_EXCLAMATION.contains(w))
                    {
                        continue;
                    }
                    return true;
                }
                // "ne zaman", "ne için": questions.
                Some(next) if matches!(*next, "zaman" | "için" | "zamandır") => return true,
                // Idioms, exclamations and concessives.
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

/// Subjects an inverted auxiliary can take. Closed on purpose: opening it to
/// any token would make "is the report late" indistinguishable from a
/// statement. The determiners are what let "is the/these/your …" be read, and
/// they are also why the conditional openings in the module doc's known-gap
/// list score — a trade taken knowingly.
const EN_SUBJECTS: &[&str] = &[
    "you", "we", "i", "it", "they", "he", "she", "this", "that", "these", "those", "there",
    "anyone", "anybody", "everyone", "someone", "the", "your", "our", "my", "their",
];

/// "can/could you believe …" is an exclamation wearing a question's clothes,
/// and it trips **both** English signals: the inversion and the request phrase.
/// One rule, consulted by both, so they cannot disagree.
fn is_rhetorical_believe(tokens: &[&str], at: usize) -> bool {
    matches!(tokens.get(at), Some(&"can") | Some(&"could"))
        && tokens.get(at + 1) == Some(&"you")
        && tokens.get(at + 2) == Some(&"believe")
}

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
            .is_some_and(|next| EN_SUBJECTS.contains(next))
        && !is_rhetorical_believe(tokens, start);
    (wh, inversion)
}

// --- Requests (both languages) ------------------------------------------------

/// Phrases that ask for something, as token windows.
///
/// Verbs on their own ("share", "send", "make sure") are **not** here: "I will
/// share the notes" and "I will make sure the numbers are right" are
/// commitments — the opposite of a request — and the verb alone cannot tell the
/// two apart. Every entry either addresses the listener ("can you", "send me")
/// or is an explicit politeness marker ("please", "lütfen").
///
/// Turkish requests are **not** in this list at all: they are built from a
/// potential/aorist stem plus the question particle, which needs a positional
/// rule rather than a phrase — see [`TR_POTENTIAL_REQUEST_STEMS`].
const REQUEST_PHRASES: &[&[&str]] = &[
    // English
    &["can", "you"],
    &["could", "you"],
    &["would", "you"],
    &["will", "you"],
    &["please"],
    &["let", "me", "know"],
    &["send", "me"],
    &["send", "us"],
    &["walk", "me", "through"],
    &["tell", "me"],
    &["show", "me"],
    &["remind", "me"],
    // Turkish politeness markers, which stand alone
    &["lütfen"],
    &["rica", "ederim"],
    &["rica", "edeyim"],
    &["rica", "etsem"],
];

/// Turkish verb stems that become a request **only** when the question particle
/// follows them.
///
/// `-abilir` is the third-person aorist potential, and on its own it states a
/// capability: "Bunu Ahmet yapabilir" is "Ahmet *can* do this" — a work
/// assignment, one of the commonest sentences in a status meeting, and the
/// opposite of asking for something. "Yapabilir **misiniz**" is the request.
/// The particle one token later is the whole difference, so the rule reads it
/// instead of matching the stem alone.
const TR_POTENTIAL_REQUEST_STEMS: &[&str] = &[
    "gönderebilir",
    "gönderir",
    "paylaşabilir",
    "paylaşır",
    "iletebilir",
    "iletir",
    "bakabilir",
    "söyleyebilir",
    "anlatabilir",
    "açıklayabilir",
    "yapabilir",
    "hatırlatır",
];

/// Words that negate whatever follows them in the same utterance.
///
/// `tokens` keeps apostrophes inside a token, so "don't" arrives whole and is
/// matched by the `n't` suffix rather than by a separate entry.
const EN_NEGATORS: &[&str] = &["not", "never", "no", "none", "nothing", "nobody"];

/// Verbs that turn what follows into reported speech: "he **said** can you
/// believe it" is someone quoting, not someone asking.
const EN_REPORTING_VERBS: &[&str] = &["said", "says", "asked", "asks", "told", "wrote", "quoted"];

/// Where a request phrase matched, or `None`.
///
/// The index matters: every guard in [`has_request_cue`] is about what sits
/// around the match, and a bare boolean throws that away.
fn find_request_phrase(tokens: &[&str]) -> Option<usize> {
    // Turkish: a potential stem followed by the question particle.
    for (i, token) in tokens.iter().enumerate() {
        if TR_POTENTIAL_REQUEST_STEMS.contains(token)
            && tokens
                .get(i + 1)
                .is_some_and(|next| is_turkish_question_particle(next))
        {
            return Some(i);
        }
    }
    // Both languages: a phrase as a window of whole tokens. Whole tokens rather
    // than a substring search, so "please" cannot fire on "pleased".
    for start in 0..tokens.len() {
        for phrase in REQUEST_PHRASES {
            if tokens[start..].starts_with(phrase) {
                return Some(start);
            }
        }
    }
    None
}

/// Did someone ask for something — and did they mean it?
///
/// Three guards, each for a shape that reaches the lexicon but reverses or
/// removes its meaning. All three were found by running the detector over
/// ordinary meeting sentences, and each has a regression test.
fn has_request_cue(tokens: &[&str]) -> bool {
    let Some(at) = find_request_phrase(tokens) else {
        return false;
    };

    // 1. Negation, anywhere before the match: "I did not ask you to send me
    //    anything", "no need to tell me", "Don't tell me you forgot the deck".
    //    The whole utterance is scanned rather than one token back, because a
    //    negator sits three or four tokens away in all three of those.
    if tokens[..at]
        .iter()
        .any(|t| EN_NEGATORS.contains(t) || t.ends_with("n't"))
    {
        return false;
    }

    // 2. Reported speech: "he said can you believe it".
    if at > 0 && EN_REPORTING_VERBS.contains(&tokens[at - 1]) {
        return false;
    }

    // 3. Discourse markers that borrow the words without asking for anything:
    //    "please note that the numbers are preliminary", "yes please".
    if tokens[at] == "please" {
        let next = tokens.get(at + 1).copied();
        if matches!(next, Some("note") | Some("be") | Some("find") | Some("see")) {
            return false;
        }
        // "Yes please" / "please." — an acceptance, not a request. A real
        // request always says what is wanted, so something must follow.
        if next.is_none() {
            return false;
        }
    }
    if is_rhetorical_believe(tokens, at) {
        return false;
    }

    true
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
    fn a_lone_politeness_marker_just_clears_the_threshold() {
        // Pinned on purpose: this is the weakest thing that fires. If the
        // threshold or the request weight moves, this test says so.
        assert_eq!(
            detect("Lütfen gönderin."),
            Some(Detection {
                kind: CueKind::Request,
                confidence: CUE_THRESHOLD
            })
        );
    }

    #[test]
    fn a_turkish_capability_statement_is_not_a_request() {
        // `-abilir` alone is "X *can* do it" — a work assignment, the commonest
        // shape in a Turkish status meeting, and the opposite of asking.
        for statement in [
            "Bunu Ahmet yapabilir",
            "Bu işe Ayşe bakabilir.",
            "Raporu ekip gönderebilir",
            "Sunumu o paylaşabilir",
            "Bu konuyu Ahmet açıklayabilir.",
            "Detayları müdür anlatabilir.",
            "Bu bana onu hatırlatır.",
        ] {
            assert_eq!(detect(statement), None, "{statement}");
        }
    }

    #[test]
    fn the_same_stem_with_the_particle_is_a_request() {
        // The fix must not cost the true positive it was protecting.
        let d = detect("Raporu bana gönderebilir misiniz").expect("cue");
        assert_eq!(
            d.confidence, 0.80,
            "stem+particle scores request AND particle"
        );
        // The particle makes it an information question by the kind rule.
        assert_eq!(d.kind, CueKind::Question);
        assert!(detect("Sunumu paylaşır mısın").is_some());
        assert!(detect("Bu işe bakabilir misiniz").is_some());
    }

    #[test]
    fn ne_kadar_is_a_question_but_her_ne_kadar_and_exclamations_are_not() {
        for question in [
            "Bu ne kadar sürer",
            "Ne kadar bütçemiz kaldı",
            "Proje ne kadar gecikti",
        ] {
            assert_eq!(kind_of(question), Some(CueKind::Question), "{question}");
        }
        // Concessive "although", and an exclamation.
        assert_eq!(detect("Her ne kadar geç olsa da başlıyoruz"), None);
        assert_eq!(detect("Ne kadar güzel bir sunum"), None);
    }

    #[test]
    fn turkish_concessives_are_not_questions() {
        assert_eq!(detect("Ne olursa olsun cuma günü teslim ediyoruz"), None);
        assert_eq!(detect("Ne yaparsak yapalım bütçe yetmiyor"), None);
        assert_eq!(detect("Nasıl olsa hallederiz"), None);
        // "kimi" as the determiner "some", not the accusative "whom".
        assert_eq!(detect("Kimi günler geç kalıyoruz"), None);
    }

    #[test]
    fn a_negated_or_quoted_request_is_not_a_request() {
        for statement in [
            "I did not ask you to send me anything",
            "no need to tell me",
            "Don't tell me you forgot the deck",
            "He said can you believe it",
            "I will make sure the numbers are right",
            "We never asked you to send us the file",
        ] {
            assert_eq!(detect(statement), None, "{statement}");
        }
    }

    #[test]
    fn please_as_a_discourse_marker_is_not_a_request() {
        assert_eq!(detect("Please note that the numbers are preliminary"), None);
        assert_eq!(detect("Yes please"), None);
        assert_eq!(detect("Please be aware of the deadline"), None);
        // …but a real one still fires.
        assert_eq!(kind_of("Please send the deck"), Some(CueKind::Request));
    }

    #[test]
    fn a_rhetorical_can_you_believe_it_is_not_a_request() {
        // It used to score 0.80 — the highest band — on the inversion plus the
        // request phrase.
        assert_eq!(detect("Can you believe it they cancelled again"), None);
    }

    #[test]
    fn a_determiner_subject_no_longer_silences_a_plain_question() {
        for question in [
            "Are these the final numbers",
            "Are those the right figures",
            "Is your team ready",
            "Is our budget approved",
        ] {
            assert_eq!(kind_of(question), Some(CueKind::Question), "{question}");
        }
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
