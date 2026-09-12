//! The copilot's settings, and the two things I1 is allowed to persist.
//!
//! Everything here is the user's own configuration plus the panel's window
//! geometry. No meeting content, no transcript, no model output: an insight is
//! not persisted at all (ADR-0038 invariant 3), so there is no table, no store
//! key and no file for one to leak into.
//!
//! ## Why the enable flag lives here and not in `betaFeatures.ts`
//!
//! Every other beta flag is a `localStorage` entry the renderer owns. This one
//! cannot be: the dormant-seam rule says that with the copilot off **no global
//! shortcut is registered and no window exists**, and that decision has to be
//! made in Rust at startup — before any renderer has run, and while
//! `localStorage` is still unreadable from this side. Keeping a second copy in
//! `betaFeatures` would be two sources of truth for one switch, and the one the
//! user could see would be the one that did not decide anything. The Settings
//! UI reads and writes this through the copilot commands instead.

use serde::{Deserialize, Serialize};

/// Defaults chosen so that a fresh install behaves exactly as it does today:
/// `enabled: false` means no window, no shortcut, nothing new registered.
pub const DEFAULT_TOGGLE_PANEL_KEYBIND: &str = "CommandOrControl+Shift+M";
pub const DEFAULT_ASK_KEYBIND: &str = "CommandOrControl+Shift+A";
pub const DEFAULT_CAPTURE_SCREEN_KEYBIND: &str = "CommandOrControl+Shift+S";

/// Panel size on first open. Narrow enough to sit beside a meeting window,
/// tall enough for a few transcript lines.
pub const DEFAULT_PANEL_WIDTH: f64 = 380.0;
pub const DEFAULT_PANEL_HEIGHT: f64 = 560.0;
/// Below this the panel stops being readable; the OS enforces it on resize.
pub const MIN_PANEL_WIDTH: f64 = 320.0;
pub const MIN_PANEL_HEIGHT: f64 = 240.0;

/// What a shortcut is *for*.
///
/// `Ask` and `CaptureScreen` are declared here because the settings surface
/// shows them and a user's chosen binding must survive until the feature that
/// uses it exists (I3 and I6). They are **not registered**: taking a system-wide
/// key combination away from every other application and then doing nothing with
/// it is worse than not having the setting. [`KeybindAction::is_registerable`]
/// is the single place that decides, and `shortcuts::apply` derives registration
/// from it rather than repeating the list — the same shape as
/// `inference::backend::select_local_backend`, where a declared-but-unbuilt
/// backend is structurally unselectable (ADR-0033).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum KeybindAction {
    /// Show or hide the panel. The only action I1 implements.
    TogglePanel,
    /// Ask the copilot for an insight — arrives with I3.
    Ask,
    /// Capture one screenshot for context — arrives with I6.
    CaptureScreen,
}

impl KeybindAction {
    pub const ALL: [KeybindAction; 3] = [
        KeybindAction::TogglePanel,
        KeybindAction::Ask,
        KeybindAction::CaptureScreen,
    ];

    /// Whether the action has an implementation behind it *today*. Only
    /// registerable actions ever reach the operating system.
    pub fn is_registerable(self) -> bool {
        match self {
            KeybindAction::TogglePanel => true,
            // I3 — no insight pipeline exists yet.
            KeybindAction::Ask => false,
            // I6 — no screen capture exists yet.
            KeybindAction::CaptureScreen => false,
        }
    }

    /// Stable wire token, identical to the serialized form.
    pub fn wire_name(self) -> &'static str {
        match self {
            KeybindAction::TogglePanel => "togglePanel",
            KeybindAction::Ask => "ask",
            KeybindAction::CaptureScreen => "captureScreen",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            KeybindAction::TogglePanel => "Show or hide the panel",
            KeybindAction::Ask => "Ask the copilot",
            KeybindAction::CaptureScreen => "Capture screen context",
        }
    }

    /// Why an action is not registered, for the Settings row. `None` when it is.
    pub fn unavailable_reason(self) -> Option<&'static str> {
        match self {
            KeybindAction::TogglePanel => None,
            KeybindAction::Ask => Some("Arrives with the copilot's first insights."),
            KeybindAction::CaptureScreen => Some("Arrives with screen context."),
        }
    }
}

/// The user's shortcut choices, one per action.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Keybinds {
    pub toggle_panel: String,
    pub ask: String,
    pub capture_screen: String,
}

impl Default for Keybinds {
    fn default() -> Self {
        Self {
            toggle_panel: DEFAULT_TOGGLE_PANEL_KEYBIND.to_string(),
            ask: DEFAULT_ASK_KEYBIND.to_string(),
            capture_screen: DEFAULT_CAPTURE_SCREEN_KEYBIND.to_string(),
        }
    }
}

impl Keybinds {
    pub fn get(&self, action: KeybindAction) -> &str {
        match action {
            KeybindAction::TogglePanel => &self.toggle_panel,
            KeybindAction::Ask => &self.ask,
            KeybindAction::CaptureScreen => &self.capture_screen,
        }
    }

    pub fn set(&mut self, action: KeybindAction, spec: String) {
        match action {
            KeybindAction::TogglePanel => self.toggle_panel = spec,
            KeybindAction::Ask => self.ask = spec,
            KeybindAction::CaptureScreen => self.capture_screen = spec,
        }
    }
}

/// Copilot settings. `serde(default)` on every field so a config written by an
/// older build — or a truncated one — still loads, with the missing half taking
/// the safe default rather than failing the whole read.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CopilotConfig {
    /// The `liveCopilot` switch. **Off until the I9 gate** (A5 GO + the live
    /// smoke): with it off nothing here registers a shortcut or creates a
    /// window, so the build behaves exactly as one without this module.
    pub enabled: bool,
    /// Ask the OS to keep the panel out of screen captures. On by default —
    /// this is a privacy default, and `policy` decides what it actually buys on
    /// this platform.
    pub content_protection: bool,
    /// The "turn off every shortcut" switch, kept separate from `enabled` so a
    /// user can run the panel from the tray without giving up a system-wide key
    /// combination.
    pub shortcuts_enabled: bool,
    pub keybinds: Keybinds,
}

impl Default for CopilotConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            content_protection: true,
            shortcuts_enabled: true,
            keybinds: Keybinds::default(),
        }
    }
}

/// Where the panel was last left. Written when the panel closes, so reopening
/// puts it back where the user put it.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PanelGeometry {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl PanelGeometry {
    /// Build a geometry from a window's own metrics, or refuse to.
    ///
    /// Pure so the refusals can be tested without a window. Two of them:
    ///
    /// - **A maximized frame is never stored.** A bare `data-tauri-drag-region`
    ///   makes double-clicking the panel header send `internal_toggle_maximize`
    ///   (tauri 2.11.1 `src/window/scripts/drag.js`: `e.detail === 2`), and no
    ///   attribute value keeps dragging while disabling that. The builder asks
    ///   for `maximizable(false)`, but that is documented **Unsupported on
    ///   Linux** — so without this guard a Linux user who double-clicks the
    ///   header once has the maximized frame written to `copilot.json`, and the
    ///   panel reopens full-screen every time thereafter. That is a stable trap:
    ///   nothing in the app would ever write a small geometry back.
    /// - **Implausible numbers are dropped** rather than stored — see
    ///   [`PanelGeometry::is_plausible`].
    ///
    /// Coordinates are LOGICAL pixels, matching what
    /// `WebviewWindowBuilder::position` consumes.
    pub fn from_window_metrics(
        maximized: bool,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    ) -> Option<Self> {
        if maximized {
            return None;
        }
        let geometry = PanelGeometry {
            x,
            y,
            width,
            height,
        };
        geometry.is_plausible().then_some(geometry)
    }

    /// Reject a stored geometry that is too small to use or numerically absurd
    /// (a corrupt store, a truncated write).
    ///
    /// It is deliberately **not** a monitor-containment check: tao already
    /// discards a position that lands on no monitor and falls back to the OS
    /// default (`tao-0.35.2/src/platform_impl/windows/window.rs`, the
    /// `available_monitors()` scan before `CreateWindowExW`), and re-deriving
    /// that here would compare these logical coordinates against Tauri's
    /// physical monitor bounds — which misjudges every HiDPI display.
    pub fn is_plausible(&self) -> bool {
        self.width >= MIN_PANEL_WIDTH
            && self.height >= MIN_PANEL_HEIGHT
            && self.width <= 10_000.0
            && self.height <= 10_000.0
            && self.x > -10_000.0
            && self.x < 20_000.0
            && self.y > -10_000.0
            && self.y < 20_000.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The dormant-seam promise, as a test rather than a comment.
    #[test]
    fn the_copilot_is_off_by_default() {
        assert!(!CopilotConfig::default().enabled);
    }

    #[test]
    fn content_protection_defaults_on() {
        assert!(CopilotConfig::default().content_protection);
    }

    /// I1 implements exactly one action; the other two are declared for their
    /// settings row and must not reach the OS.
    #[test]
    fn only_the_toggle_is_registerable_today() {
        assert!(KeybindAction::TogglePanel.is_registerable());
        assert!(!KeybindAction::Ask.is_registerable());
        assert!(!KeybindAction::CaptureScreen.is_registerable());
    }

    #[test]
    fn an_unregisterable_action_explains_itself() {
        for action in KeybindAction::ALL {
            assert_eq!(
                action.is_registerable(),
                action.unavailable_reason().is_none(),
                "{} disagrees with its own reason",
                action.wire_name()
            );
        }
    }

    #[test]
    fn default_keybinds_parse() {
        for action in KeybindAction::ALL {
            let spec = Keybinds::default().get(action).to_string();
            let parsed = crate::copilot::keybind::parse(&spec)
                .unwrap_or_else(|e| panic!("default for {} is invalid: {e}", action.wire_name()));
            assert_eq!(parsed.canonical(), spec, "default is not canonical");
        }
    }

    #[test]
    fn default_keybinds_are_distinct() {
        let k = Keybinds::default();
        let mut specs: Vec<&str> = KeybindAction::ALL.iter().map(|a| k.get(*a)).collect();
        specs.sort_unstable();
        let before = specs.len();
        specs.dedup();
        assert_eq!(before, specs.len(), "two actions share a default shortcut");
    }

    #[test]
    fn get_and_set_address_the_same_slot() {
        let mut k = Keybinds::default();
        for action in KeybindAction::ALL {
            k.set(action, "Alt+F9".into());
            assert_eq!(k.get(action), "Alt+F9");
        }
    }

    #[test]
    fn a_partial_config_keeps_the_safe_defaults() {
        // A config from a build that predates `shortcutsEnabled`.
        let json = r#"{"enabled":true,"contentProtection":false}"#;
        let cfg: CopilotConfig = serde_json::from_str(json).expect("partial config still loads");
        assert!(cfg.enabled);
        assert!(!cfg.content_protection);
        assert!(cfg.shortcuts_enabled);
        assert_eq!(cfg.keybinds, Keybinds::default());
    }

    #[test]
    fn config_round_trips_as_camel_case() {
        let cfg = CopilotConfig::default();
        let json = serde_json::to_string(&cfg).expect("serialises");
        assert!(
            json.contains("contentProtection"),
            "wire shape changed: {json}"
        );
        assert!(json.contains("togglePanel"), "wire shape changed: {json}");
        assert_eq!(
            serde_json::from_str::<CopilotConfig>(&json).expect("deserialises"),
            cfg
        );
    }

    /// The Linux trap: `maximizable(false)` does not apply there, so this
    /// guard is the only thing standing between one double-click on the header
    /// and a panel that reopens full-screen forever.
    #[test]
    fn a_maximized_frame_is_never_stored() {
        assert_eq!(
            PanelGeometry::from_window_metrics(true, 100.0, 100.0, 1920.0, 1080.0),
            None
        );
    }

    #[test]
    fn a_normal_frame_is_stored_verbatim() {
        let geometry = PanelGeometry::from_window_metrics(false, 12.0, 34.0, 380.0, 560.0)
            .expect("a plausible frame is stored");
        assert_eq!(geometry.x, 12.0);
        assert_eq!(geometry.y, 34.0);
        assert_eq!(geometry.width, 380.0);
        assert_eq!(geometry.height, 560.0);
    }

    #[test]
    fn an_implausible_frame_is_refused_even_when_not_maximized() {
        assert_eq!(
            PanelGeometry::from_window_metrics(false, 0.0, 0.0, 10.0, 10.0),
            None
        );
    }

    #[test]
    fn an_off_screen_or_tiny_geometry_is_rejected() {
        let sane = PanelGeometry {
            x: 100.0,
            y: 100.0,
            width: DEFAULT_PANEL_WIDTH,
            height: DEFAULT_PANEL_HEIGHT,
        };
        assert!(sane.is_plausible());

        // Numbers no window ever legitimately reports — a corrupt or truncated
        // store, not an unplugged monitor (tao handles that one itself).
        assert!(!PanelGeometry {
            x: 40_000.0,
            ..sane
        }
        .is_plausible());
        assert!(!PanelGeometry {
            y: -50_000.0,
            ..sane
        }
        .is_plausible());
        // Collapsed to nothing.
        assert!(!PanelGeometry {
            width: 10.0,
            ..sane
        }
        .is_plausible());
        assert!(!PanelGeometry {
            height: 0.0,
            ..sane
        }
        .is_plausible());
    }
}
