//! Live Copilot — the private in-meeting panel (BACKLOG **EPIC I**, ADR-0038).
//!
//! Mityu helps *after* a meeting: source-linked summaries, action items,
//! evidence search, export. This module is the first half of helping *during*
//! one. **I1 is the shell only** — a window, a global shortcut, an honest
//! screen-sharing posture. There is no AI here: no provider call, no prompt, no
//! retrieval, no model output, and nothing that could produce an insight to
//! persist. I2 adds the deterministic live-context service; I3 the first
//! grounded insights.
//!
//! ## The four invariants, and where each is actually enforced
//!
//! 1. **Private, not covert.** [`policy`] decides what the OS will really do
//!    about screen capture *and returns the sentence to show about it*, so the
//!    UI cannot promise more than the platform delivers — Windows enforces
//!    capture exclusion, macOS 15+ ignores it for ScreenCaptureKit, Linux has
//!    no mechanism. [`window`] never hides the app from the taskbar or dock,
//!    never renames the process, and there is no keyboard hook anywhere in this
//!    module. "Undetectable" is not a thing Mityu does, and a test in
//!    [`policy`] fails if that word reaches the user-facing strings.
//! 2. **The copilot never captures.** Nothing here starts, stops or reads
//!    audio, and no file in `audio/` is touched by this epic. The panel shows
//!    live content only while a recording session — one whose consent ticket
//!    was already consumed by `recording_consent` — is running.
//! 3. **Draft-only, and I1 persists nothing a model made.** The only two things
//!    written to disk are the user's settings and the panel's last position
//!    ([`store`]). When insights arrive in I3 they will be non-persisted
//!    drafts; "Pin to notes" will write a `draft` block through the existing
//!    C1 repository, which forces that status.
//! 4. **Dormant by default.** [`config::CopilotConfig::enabled`] is `false`, and
//!    with it off [`shortcuts::apply`] registers nothing and [`window::open`]
//!    refuses. The flag may default *on* only after the I9 gate (A5 GO plus the
//!    live smoke) — mechanics are quality-independent, claims are not.
//!
//! ## Layout
//!
//! - [`config`] — settings, the [`config::KeybindAction`] set, geometry.
//! - [`policy`] — the screen-capture verdict per platform, and its wording.
//! - [`keybind`] — shortcut grammar: parsing, validation, canonical form.
//! - [`store`] — the `copilot.json` settings/geometry file.
//! - [`window`] — the panel window's lifecycle.
//! - [`shortcuts`] — registration with the OS.
//! - [`commands`] — the Tauri surface (registered in `lib.rs`).

pub mod commands;
pub mod config;
pub mod keybind;
pub mod policy;
pub mod shortcuts;
pub mod store;
pub mod window;

pub use config::{CopilotConfig, KeybindAction, Keybinds, PanelGeometry};
pub use policy::{CaptureProtection, ProtectionVerdict};
pub use window::COPILOT_WINDOW_LABEL;

use tauri::{AppHandle, Runtime};

/// Startup hook, called from `lib.rs`'s `setup`.
///
/// Reads the stored settings and registers the global shortcut **only** if the
/// copilot is enabled. With the default settings this does nothing observable:
/// no window, no shortcut, no listener — the build behaves exactly as one
/// compiled without this module (the `sync/`, `agents/` and `inference/`
/// dormant-seam precedent).
pub fn init<R: Runtime>(app: &AppHandle<R>) {
    let config = store::load_config(app);
    if !config.enabled {
        log::debug!("copilot: disabled; no shortcut registered and no panel created");
        return;
    }
    if let Err(e) = shortcuts::apply_config(app, &config) {
        log::warn!("copilot: shortcuts could not be applied at startup: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The seam's whole promise in one assertion: a fresh install registers
    /// nothing, because the config it reads is the disabled one.
    #[test]
    fn a_fresh_install_has_the_copilot_off() {
        assert!(!CopilotConfig::default().enabled);
    }
}
