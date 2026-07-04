//! Opt-in PII / keyword redaction (BACKLOG C6 — redaction half only).
//!
//! Local-first privacy boundary (docs/SECURITY_PRIVACY.md "Retention/redaction" +
//! OWASP LLM02 sensitive-info-disclosure). When a workspace **opts in**, transcript
//! text is scrubbed of common PII and user-supplied keyword terms at two seams:
//!   1. **before** it is persisted to SQLite (the transcript write boundary), and
//!   2. **before** it is handed to a summary LLM provider (the key seam that stops
//!      PII reaching a cloud provider).
//!
//! ## Design invariants
//! - **OFF by default** ([`RedactionConfig::default`] is disabled). When disabled,
//!   [`redact`] is a verbatim, allocation-cheap no-op — existing local data and
//!   flows are unchanged unless the user turns this on.
//! - **Pure & DB-free.** This module knows nothing about the database, the
//!   [`crate::context::AuthContext`], or Tauri. Config is loaded by the caller
//!   (from `SettingsRepository`) and passed in, so the redactor stays trivially
//!   testable and reusable at every boundary.
//! - **Conservative by design.** Patterns favour *precision over recall*: it is
//!   better to miss an exotic PII shape than to shred ordinary numbers/words in a
//!   transcript. See each pattern's note below. Redaction changes stored text but
//!   does **not** touch segment ids, timestamps, or `source_chunk_id` linkage, so
//!   the HITL / source-linking flow (docs/CONTRACTS.md §4) is preserved.
//! - **Never logs content.** No raw text and no redacted-away value is ever logged
//!   from this module (docs/SECURITY_PRIVACY.md "Secrets" / LLM02).
//!
//! ## Placeholders (typed, so a reviewer can see *what kind* of thing was removed)
//! | match | placeholder |
//! |-------|-------------|
//! | email address                     | `[EMAIL]`    |
//! | phone number                      | `[PHONE]`    |
//! | credit-card number (Luhn-valid)   | `[CARD]`     |
//! | IBAN                              | `[IBAN]`     |
//! | Turkish TC Kimlik No (checksummed) | `[ID]`       |
//! | custom keyword term               | `[REDACTED]` |

use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};

/// Per-workspace redaction policy (docs/CONTRACTS.md §7). Serialized as a small
/// JSON blob into the `settings.redactionConfig` column (see
/// `SettingsRepository::{get,set}_redaction_config`).
///
/// [`Default`] is **disabled** with default PII patterns pre-selected, so a
/// workspace that has never configured redaction behaves exactly as before, and a
/// user who flips `enabled` on gets the built-in PII patterns without extra setup.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RedactionConfig {
    /// Master switch. When `false`, [`redact`] returns its input unchanged.
    pub enabled: bool,
    /// Apply the built-in PII patterns (email/phone/card/IBAN/TC). When `false`,
    /// only [`custom_terms`](Self::custom_terms) are redacted.
    pub use_default_patterns: bool,
    /// Extra case-insensitive literal terms to redact (e.g. a project codename or a
    /// customer name). Each occurrence becomes `[REDACTED]`. Empty by default.
    pub custom_terms: Vec<String>,
}

impl Default for RedactionConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            use_default_patterns: true,
            custom_terms: Vec::new(),
        }
    }
}

impl RedactionConfig {
    /// True when redaction would actually do work: enabled AND at least one source
    /// of matches is active (built-in patterns, or at least one *non-blank* custom
    /// term). Lets callers skip cloning/allocating on the (default) no-op path — and
    /// treats an `enabled` config whose only custom terms are whitespace as a no-op
    /// (there is nothing to match).
    pub fn is_active(&self) -> bool {
        self.enabled
            && (self.use_default_patterns || self.custom_terms.iter().any(|t| !t.trim().is_empty()))
    }
}

// --- Built-in PII patterns (compiled once) -----------------------------------
//
// Each pattern is deliberately narrow. Where a pure regex would over-match
// (credit cards, TC Kimlik No) a second Rust-side validator (Luhn / TC checksum)
// gates the replacement so ordinary digit runs are left intact.

/// Email: standard local@domain.tld shape. `{2,}` TLD avoids matching `a@b`.
static EMAIL_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}\b")
        .expect("EMAIL_RE is a valid literal regex")
});

/// IBAN: 2 country letters + 2 check digits + 11..30 alphanumerics. Bounded so it
/// does not swallow an arbitrary alnum run; run BEFORE card/phone.
static IBAN_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)\b[A-Z]{2}[0-9]{2}[A-Z0-9]{11,30}\b")
        .expect("IBAN_RE is a valid literal regex")
});

/// Credit-card *candidate*: 13..19 digits, optionally in groups separated by a
/// single space or hyphen. The regex only finds candidates; [`is_luhn_valid`]
/// decides, so long non-card digit strings are not redacted as cards.
static CARD_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\b(?:\d[ -]?){12,18}\d\b").expect("CARD_RE is a valid literal regex")
});

/// TC Kimlik No *candidate*: exactly 11 digits, first non-zero. Gated by the
/// official checksum in [`is_valid_tc_kimlik`] so random 11-digit numbers (e.g. a
/// long timestamp) are not redacted.
static TC_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\b[1-9]\d{10}\b").expect("TC_RE is a valid literal regex"));

/// Phone *candidate*: optional leading `+`, then a run of digits and phone
/// separators (space / `.` / `-` / `(` `)`). The regex is deliberately loose; the
/// [`is_phone_like`] guard then enforces the real constraints (7..=15 digits AND a
/// leading `+` or at least one separator) so a bare integer run — e.g. a
/// checksum-failing 11-digit TC candidate or a 16-digit non-card group — is NOT
/// mistaken for a phone number. Run LAST of the numeric patterns so IBAN / card /
/// TC claim their digits first.
static PHONE_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\+?\d[\d .()\-]{5,}\d").expect("PHONE_RE is a valid literal regex"));

/// Guard for [`PHONE_RE`] candidates: a real phone number has 7..=15 digits and is
/// either internationally prefixed (`+`) or written with grouping separators. This
/// keeps ordinary long integers (ids, non-card digit groups) from being redacted as
/// `[PHONE]`, honouring the conservative/precision-over-recall stance.
fn is_phone_like(m: &str) -> bool {
    let digits = m.chars().filter(|c| c.is_ascii_digit()).count();
    if !(7..=15).contains(&digits) {
        return false;
    }
    let has_plus = m.starts_with('+');
    let has_separator = m.chars().any(|c| matches!(c, ' ' | '.' | '-' | '(' | ')'));
    has_plus || has_separator
}

/// Luhn (mod-10) checksum used for credit-card validation.
fn is_luhn_valid(digits: &str) -> bool {
    let mut sum = 0u32;
    let mut alt = false;
    // Iterate right-to-left over ASCII digits only.
    for ch in digits.chars().rev() {
        let Some(d) = ch.to_digit(10) else { continue };
        let mut d = d;
        if alt {
            d *= 2;
            if d > 9 {
                d -= 9;
            }
        }
        sum += d;
        alt = !alt;
    }
    sum % 10 == 0
}

/// Validate an 11-character digit string against the Turkish TC Kimlik No
/// algorithm (also implicitly enforces `d1 != 0`).
fn is_valid_tc_kimlik(s: &str) -> bool {
    let d: Vec<u32> = s.chars().filter_map(|c| c.to_digit(10)).collect();
    if d.len() != 11 || d[0] == 0 {
        return false;
    }
    let odd_sum = d[0] + d[2] + d[4] + d[6] + d[8]; // positions 1,3,5,7,9
    let even_sum = d[1] + d[3] + d[5] + d[7]; //         positions 2,4,6,8
                                              // 10th digit = ((odd*7) - even) mod 10.
                                              // Add a multiple of 10 (70) before the
                                              // subtraction so the u32 math never
                                              // underflows (even_sum <= 36 < 70).
    let tenth = ((odd_sum * 7) + 70 - even_sum) % 10;
    if tenth != d[9] {
        return false;
    }
    // 11th digit = sum of first 10, mod 10
    let sum_first_ten: u32 = d[..10].iter().sum();
    sum_first_ten % 10 == d[10]
}

/// Escaped, case-insensitive union of the configured custom terms, compiled once
/// per call (custom terms are per-workspace config, not a global constant).
/// Returns `None` when there are no usable (non-blank) terms.
fn custom_terms_regex(terms: &[String]) -> Option<Regex> {
    let alternation: Vec<String> = terms
        .iter()
        .map(|t| t.trim())
        .filter(|t| !t.is_empty())
        .map(regex::escape)
        .collect();
    if alternation.is_empty() {
        return None;
    }
    // `(?i)` case-insensitive; longer alternatives first so the engine prefers the
    // most specific match. No `\b` wrapper: custom terms may be non-word (codes,
    // punctuated names), and callers expect substring redaction.
    let mut sorted = alternation;
    sorted.sort_by_key(|s| std::cmp::Reverse(s.len()));
    Regex::new(&format!("(?i)(?:{})", sorted.join("|"))).ok()
}

/// Replace every match of `re` for which `keep_match(matched) == true` with
/// `placeholder`. A single left-to-right pass, so adjacent/overlapping candidate
/// spans cannot corrupt the output: `regex` yields non-overlapping matches and we
/// copy the gaps between them verbatim. `keep_match` lets a checksum validator veto
/// a candidate (its original text is then preserved untouched).
fn replace_validated(
    text: &str,
    re: &Regex,
    placeholder: &str,
    keep: impl Fn(&str) -> bool,
) -> String {
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    for m in re.find_iter(text) {
        if !keep(m.as_str()) {
            continue;
        }
        out.push_str(&text[last..m.start()]);
        out.push_str(placeholder);
        last = m.end();
    }
    out.push_str(&text[last..]);
    out
}

/// Redact PII / keyword terms from `text` per `cfg`.
///
/// **No-op when disabled:** if `cfg.enabled` is `false` (the default), the input is
/// returned unchanged via a cheap early return — this is the local-first,
/// non-breaking path. When enabled, the built-in PII patterns (if
/// `cfg.use_default_patterns`) and the case-insensitive `cfg.custom_terms` are
/// applied, each match replaced with its typed placeholder.
///
/// Pass ordering is significant and fixed: email → IBAN → card → TC → phone →
/// custom. Numeric PII with distinctive structure (IBAN, Luhn-valid card, valid
/// TC) claims its digits before the looser phone pattern runs, so a card is never
/// mangled into a `[PHONE]`. Placeholders are bracketed tokens and are not
/// re-matched by later passes.
pub fn redact(text: &str, cfg: &RedactionConfig) -> String {
    // Cheap, allocation-free early return on the default/no-op path.
    if !cfg.is_active() {
        return text.to_string();
    }

    let mut out = text.to_string();

    if cfg.use_default_patterns {
        out = replace_validated(&out, &EMAIL_RE, "[EMAIL]", |_| true);
        out = replace_validated(&out, &IBAN_RE, "[IBAN]", |_| true);
        out = replace_validated(&out, &CARD_RE, "[CARD]", is_luhn_valid);
        out = replace_validated(&out, &TC_RE, "[ID]", is_valid_tc_kimlik);
        out = replace_validated(&out, &PHONE_RE, "[PHONE]", is_phone_like);
    }

    if let Some(re) = custom_terms_regex(&cfg.custom_terms) {
        out = replace_validated(&out, &re, "[REDACTED]", |_| true);
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Config with default PII patterns on and no custom terms.
    fn pii_only() -> RedactionConfig {
        RedactionConfig {
            enabled: true,
            use_default_patterns: true,
            custom_terms: Vec::new(),
        }
    }

    #[test]
    fn default_config_is_disabled_and_verbatim() {
        let cfg = RedactionConfig::default();
        assert!(!cfg.enabled, "default must be OFF (non-breaking)");
        assert!(!cfg.is_active());
        let input = "Call me at +90 532 123 45 67, email a@b.com, TC 10000000146.";
        // Disabled == byte-for-byte identical output.
        assert_eq!(redact(input, &cfg), input);
    }

    #[test]
    fn enabled_but_nothing_selected_is_noop() {
        // enabled, but no default patterns and no custom terms => nothing to do.
        let cfg = RedactionConfig {
            enabled: true,
            use_default_patterns: false,
            custom_terms: vec!["   ".to_string()], // blank-only, filtered out
        };
        assert!(!cfg.is_active());
        let input = "email a@b.com stays because nothing is selected";
        assert_eq!(redact(input, &cfg), input);
    }

    #[test]
    fn email_positive_and_negative() {
        let cfg = pii_only();
        assert_eq!(
            redact("Reach jane.doe+tag@example.co.uk today", &cfg),
            "Reach [EMAIL] today"
        );
        // Negative: not an email (no domain TLD), must be untouched.
        assert_eq!(redact("the price is 3@ each", &cfg), "the price is 3@ each");
    }

    #[test]
    fn phone_positive_and_negative() {
        let cfg = pii_only();
        assert_eq!(
            redact("ring +90 532 123 45 67 now", &cfg),
            "ring [PHONE] now"
        );
        // Negative: a short bare integer in prose is not a phone number.
        assert_eq!(redact("we ordered 42 units", &cfg), "we ordered 42 units");
    }

    #[test]
    fn credit_card_positive_and_negative() {
        let cfg = pii_only();
        // 4242 4242 4242 4242 is a well-known Luhn-valid test card.
        assert_eq!(
            redact("card 4242 4242 4242 4242 on file", &cfg),
            "card [CARD] on file"
        );
        // Negative: 16 digits that FAIL Luhn are left as-is (not a real card).
        let not_card = "id 1234 1234 1234 1234 here";
        assert_eq!(redact(not_card, &cfg), not_card);
    }

    #[test]
    fn iban_positive_and_negative() {
        let cfg = pii_only();
        // A well-formed Turkish IBAN shape.
        assert_eq!(
            redact("pay to TR330006100519786457841326 please", &cfg),
            "pay to [IBAN] please"
        );
        // Negative: two letters + short code, not IBAN length.
        assert_eq!(redact("code AB12 short", &cfg), "code AB12 short");
    }

    #[test]
    fn tc_kimlik_positive_and_negative() {
        let cfg = pii_only();
        // 10000000146 is a valid TC Kimlik No per the official checksum.
        assert_eq!(
            redact("TC No 10000000146 verified", &cfg),
            "TC No [ID] verified"
        );
        // Negative: 11 digits that fail the checksum are NOT redacted as an ID.
        let bad = "ref 12345678901 only";
        assert_eq!(redact(bad, &cfg), bad);
    }

    #[test]
    fn custom_terms_are_case_insensitive() {
        let cfg = RedactionConfig {
            enabled: true,
            use_default_patterns: false,
            custom_terms: vec!["Project Falcon".to_string(), "acme".to_string()],
        };
        assert_eq!(
            redact("PROJECT FALCON ships for Acme Corp", &cfg),
            "[REDACTED] ships for [REDACTED] Corp"
        );
        // A term not present leaves text untouched.
        assert_eq!(redact("nothing secret here", &cfg), "nothing secret here");
    }

    #[test]
    fn custom_terms_ignore_blank_entries() {
        let cfg = RedactionConfig {
            enabled: true,
            use_default_patterns: false,
            custom_terms: vec!["".to_string(), "  ".to_string(), "secret".to_string()],
        };
        assert_eq!(redact("this is secret", &cfg), "this is [REDACTED]");
    }

    #[test]
    fn adjacent_matches_do_not_corrupt_output() {
        let cfg = pii_only();
        // Two emails separated by a comma+space: both redacted, gap preserved.
        assert_eq!(redact("a@x.com, b@y.com", &cfg), "[EMAIL], [EMAIL]");
        // Email immediately followed by punctuation then a phone.
        let out = redact("mail a@x.com; tel +90 212 000 00 00.", &cfg);
        assert_eq!(out, "mail [EMAIL]; tel [PHONE].");
    }

    #[test]
    fn overlapping_number_types_prefer_specific() {
        let cfg = pii_only();
        // A Luhn-valid card must be redacted as [CARD], never split into [PHONE].
        let out = redact("pay 4242424242424242 today", &cfg);
        assert_eq!(out, "pay [CARD] today");
        assert!(!out.contains("[PHONE]"));
    }

    #[test]
    fn mixed_pii_and_custom_together() {
        let cfg = RedactionConfig {
            enabled: true,
            use_default_patterns: true,
            custom_terms: vec!["Falcon".to_string()],
        };
        let out = redact("Falcon lead jane@acme.com called +90 555 111 22 33", &cfg);
        assert_eq!(out, "[REDACTED] lead [EMAIL] called [PHONE]");
    }

    #[test]
    fn non_ascii_text_is_preserved_around_matches() {
        let cfg = pii_only();
        // Multibyte characters around a match must survive char-boundary-safe slicing.
        // (The email itself is ASCII — the conservative email pattern intentionally
        // does not target internationalized/IDN domains; the point here is that the
        // Turkish text on either side is copied through verbatim.)
        let out = redact("Müşteri e-posta: ali@ornek.com — teşekkürler", &cfg);
        assert!(out.contains("Müşteri"));
        assert!(out.contains("teşekkürler"));
        assert!(out.contains("[EMAIL]"));
        assert!(out.contains(" — "));
    }

    #[test]
    fn luhn_validator_basic() {
        assert!(is_luhn_valid("4242424242424242"));
        assert!(!is_luhn_valid("4242424242424241"));
    }

    #[test]
    fn tc_validator_basic() {
        assert!(is_valid_tc_kimlik("10000000146"));
        assert!(!is_valid_tc_kimlik("12345678901"));
        assert!(!is_valid_tc_kimlik("00000000000"));
        assert!(!is_valid_tc_kimlik("1234567890")); // too short
    }
}
