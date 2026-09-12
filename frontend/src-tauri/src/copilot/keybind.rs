//! Parsing and canonicalising the panel's global shortcut.
//!
//! A global shortcut is registered with the *operating system*, not with the
//! app: while Mityu runs, the chosen combination stops reaching every other
//! program on the machine. Two consequences shape this module.
//!
//! **At least one modifier is required.** A bare `M` would swallow the letter M
//! system-wide — in the user's editor, their browser, their terminal — and the
//! only visible symptom would be "my keyboard is broken". [`parse`] rejects it
//! rather than letting the setting exist.
//!
//! **Validation happens where the user is typing, not at registration.** The
//! shortcut plugin parses its own strings and fails at `register()`, by which
//! point the user has left Settings and the panel has silently lost its hotkey.
//! So the same string is checked here first, and the canonical form this
//! produces (`CommandOrControl+Shift+M`) is deliberately the spelling the plugin
//! accepts.
//!
//! Pure: no plugin type appears in this file, so the whole grammar is testable
//! without a running app.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Modifiers we accept, in canonical order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Modifier {
    /// ⌘ on macOS, Ctrl elsewhere — the portable default.
    CommandOrControl,
    Control,
    Alt,
    Shift,
    /// The Windows/Super/Command key when specifically that key is wanted.
    Super,
}

impl Modifier {
    fn canonical(self) -> &'static str {
        match self {
            Modifier::CommandOrControl => "CommandOrControl",
            Modifier::Control => "Control",
            Modifier::Alt => "Alt",
            Modifier::Shift => "Shift",
            Modifier::Super => "Super",
        }
    }

    fn from_token(token: &str) -> Option<Self> {
        match token.to_ascii_lowercase().as_str() {
            "commandorcontrol" | "cmdorctrl" | "commandorctrl" | "mod" => {
                Some(Modifier::CommandOrControl)
            }
            "control" | "ctrl" => Some(Modifier::Control),
            "alt" | "option" => Some(Modifier::Alt),
            "shift" => Some(Modifier::Shift),
            "super" | "meta" | "command" | "cmd" | "win" => Some(Modifier::Super),
            _ => None,
        }
    }
}

/// Why a shortcut string was refused. Each variant maps to a sentence the
/// Settings form can show next to the field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeybindError {
    Empty,
    /// No modifier — see the module note on why this is not allowed.
    NoModifier,
    /// Modifiers only, no key to press with them.
    NoKey,
    /// More than one non-modifier key (`Ctrl+A+B`).
    MultipleKeys {
        first: String,
        second: String,
    },
    DuplicateModifier(Modifier),
    UnknownKey(String),
}

impl fmt::Display for KeybindError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KeybindError::Empty => write!(f, "Enter a shortcut."),
            KeybindError::NoModifier => write!(
                f,
                "Add at least one modifier (Ctrl, Alt, Shift or Command). A shortcut without one \
                 would capture that key in every app on this computer."
            ),
            KeybindError::NoKey => write!(f, "Add a key to press with the modifiers."),
            KeybindError::MultipleKeys { first, second } => {
                write!(f, "Use one key, not two ('{first}' and '{second}').")
            }
            KeybindError::DuplicateModifier(m) => {
                write!(f, "'{}' is listed twice.", m.canonical())
            }
            KeybindError::UnknownKey(key) => write!(f, "'{key}' is not a key Mityu can register."),
        }
    }
}

/// A validated shortcut. Construct with [`parse`]; render with
/// [`Keybind::canonical`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Keybind {
    modifiers: Vec<Modifier>,
    key: String,
}

impl Keybind {
    /// The spelling to store and to hand the shortcut plugin: modifiers in a
    /// fixed order, then the key. Two users who typed `shift+ctrl+m` and
    /// `Ctrl+Shift+M` end up with one string, so comparing bindings for
    /// collisions is string equality rather than a second parser.
    pub fn canonical(&self) -> String {
        let mut parts: Vec<&str> = self.modifiers.iter().map(|m| m.canonical()).collect();
        parts.push(&self.key);
        parts.join("+")
    }

    pub fn modifiers(&self) -> &[Modifier] {
        &self.modifiers
    }

    pub fn key(&self) -> &str {
        &self.key
    }
}

impl fmt::Display for Keybind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.canonical())
    }
}

/// Named keys accepted beside letters, digits and F1–F24.
///
/// A deliberately short list. Anything outside it is refused at the form rather
/// than accepted here and rejected later by the plugin, where the failure would
/// be invisible.
const NAMED_KEYS: &[&str] = &[
    "Space",
    "Enter",
    "Tab",
    "Escape",
    "Backspace",
    "Delete",
    "Insert",
    "Home",
    "End",
    "PageUp",
    "PageDown",
    "Up",
    "Down",
    "Left",
    "Right",
    "Comma",
    "Period",
    "Slash",
    "Backslash",
    "Semicolon",
    "Quote",
    "BracketLeft",
    "BracketRight",
    "Minus",
    "Equal",
    "Backquote",
];

fn normalise_key(token: &str) -> Result<String, KeybindError> {
    let trimmed = token.trim();
    if trimmed.is_empty() {
        return Err(KeybindError::UnknownKey(token.to_string()));
    }

    // Single letter or digit: KeyA / Digit1 are the plugin's own codes, but the
    // plain uppercase form is accepted too and is what a user recognises.
    if trimmed.len() == 1 {
        let c = trimmed.chars().next().expect("length checked");
        if c.is_ascii_alphanumeric() {
            return Ok(c.to_ascii_uppercase().to_string());
        }
        return Err(KeybindError::UnknownKey(trimmed.to_string()));
    }

    // Function keys.
    if let Some(rest) = trimmed
        .strip_prefix(['F', 'f'])
        .filter(|rest| !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit()))
    {
        if let Ok(n) = rest.parse::<u8>() {
            if (1..=24).contains(&n) {
                return Ok(format!("F{n}"));
            }
        }
        return Err(KeybindError::UnknownKey(trimmed.to_string()));
    }

    NAMED_KEYS
        .iter()
        .find(|named| named.eq_ignore_ascii_case(trimmed))
        .map(|named| (*named).to_string())
        .ok_or_else(|| KeybindError::UnknownKey(trimmed.to_string()))
}

/// Parse a `+`-separated shortcut such as `CommandOrControl+Shift+M`.
///
/// Case-insensitive, tolerant of spacing, and strict about everything that
/// would otherwise surface as a shortcut that silently does not work.
pub fn parse(spec: &str) -> Result<Keybind, KeybindError> {
    let tokens: Vec<&str> = spec
        .split('+')
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .collect();

    if tokens.is_empty() {
        return Err(KeybindError::Empty);
    }

    let mut modifiers: Vec<Modifier> = Vec::new();
    let mut key: Option<String> = None;

    for token in tokens {
        if let Some(modifier) = Modifier::from_token(token) {
            if modifiers.contains(&modifier) {
                return Err(KeybindError::DuplicateModifier(modifier));
            }
            modifiers.push(modifier);
            continue;
        }

        let normalised = normalise_key(token)?;
        if let Some(existing) = key {
            return Err(KeybindError::MultipleKeys {
                first: existing,
                second: normalised,
            });
        }
        key = Some(normalised);
    }

    let Some(key) = key else {
        return Err(KeybindError::NoKey);
    };
    if modifiers.is_empty() {
        return Err(KeybindError::NoModifier);
    }

    modifiers.sort();
    Ok(Keybind { modifiers, key })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonicalises_order_and_case() {
        let a = parse("shift+ctrl+m").expect("valid");
        let b = parse("Control+Shift+M").expect("valid");
        assert_eq!(a.canonical(), "Control+Shift+M");
        assert_eq!(a, b);
    }

    #[test]
    fn accepts_the_portable_modifier_spelling() {
        assert_eq!(
            parse("CommandOrControl+Shift+M")
                .expect("valid")
                .canonical(),
            "CommandOrControl+Shift+M"
        );
        assert_eq!(
            parse("mod+shift+m").expect("valid").canonical(),
            "CommandOrControl+Shift+M"
        );
    }

    /// The rule this module exists for: a bare key would be taken from every
    /// other application on the machine.
    #[test]
    fn refuses_a_shortcut_with_no_modifier() {
        assert_eq!(parse("M"), Err(KeybindError::NoModifier));
        assert_eq!(parse("F5"), Err(KeybindError::NoModifier));
        assert_eq!(parse("Space"), Err(KeybindError::NoModifier));
    }

    #[test]
    fn refuses_modifiers_with_no_key() {
        assert_eq!(parse("Control+Shift"), Err(KeybindError::NoKey));
    }

    #[test]
    fn refuses_two_keys() {
        assert!(matches!(
            parse("Control+A+B"),
            Err(KeybindError::MultipleKeys { .. })
        ));
    }

    #[test]
    fn refuses_a_repeated_modifier() {
        assert_eq!(
            parse("Ctrl+Control+M"),
            Err(KeybindError::DuplicateModifier(Modifier::Control))
        );
    }

    #[test]
    fn refuses_an_empty_string() {
        assert_eq!(parse(""), Err(KeybindError::Empty));
        assert_eq!(parse("   "), Err(KeybindError::Empty));
        assert_eq!(parse("+++"), Err(KeybindError::Empty));
    }

    #[test]
    fn refuses_a_key_the_plugin_would_not_take() {
        assert_eq!(
            parse("Control+Hyperspace"),
            Err(KeybindError::UnknownKey("Hyperspace".into()))
        );
        assert_eq!(
            parse("Control+F25"),
            Err(KeybindError::UnknownKey("F25".into()))
        );
    }

    #[test]
    fn accepts_function_and_named_keys() {
        assert_eq!(parse("Alt+f7").expect("valid").canonical(), "Alt+F7");
        assert_eq!(
            parse("Control+Alt+space").expect("valid").canonical(),
            "Control+Alt+Space"
        );
    }

    #[test]
    fn every_error_says_something_a_user_can_act_on() {
        let errors = [
            KeybindError::Empty,
            KeybindError::NoModifier,
            KeybindError::NoKey,
            KeybindError::MultipleKeys {
                first: "A".into(),
                second: "B".into(),
            },
            KeybindError::DuplicateModifier(Modifier::Shift),
            KeybindError::UnknownKey("Hyperspace".into()),
        ];
        for error in errors {
            let message = error.to_string();
            assert!(!message.is_empty());
            assert!(message.ends_with('.'), "not a sentence: {message}");
        }
    }
}
