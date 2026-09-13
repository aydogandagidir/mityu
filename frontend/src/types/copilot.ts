/**
 * Live Copilot wire types (BACKLOG I1, ADR-0038).
 *
 * These mirror the Rust shapes in `src-tauri/src/copilot/` exactly — the Rust
 * side serialises camelCase, so field names here are the contract. Change one
 * side and you must change the other.
 */

/**
 * What the operating system will actually do when asked to keep the panel out
 * of a screen capture.
 *
 * Three states rather than a boolean on purpose: `bestEffort` is the honest
 * answer on macOS 15+, where ScreenCaptureKit — which Zoom, Meet and Teams use
 * — ignores the request entirely. Rendering it as "protected" would promise
 * invisibility the platform does not deliver; rendering it as "unsupported"
 * would hide that it still blocks legacy capture and screenshot tools.
 */
export type CaptureProtectionLevel = 'enforced' | 'bestEffort' | 'unsupported';

/** The posture plus the wording to show for it. The backend owns both. */
export interface ProtectionVerdict {
  level: CaptureProtectionLevel;
  /** Short chip text for the panel header. */
  headline: string;
  /** The full sentence, for Settings. */
  detail: string;
}

/** One shortcut per action. Canonicalised by the backend on save. */
export interface Keybinds {
  togglePanel: string;
  ask: string;
  captureScreen: string;
}

export interface CopilotConfig {
  /**
   * The live-copilot switch. Default OFF: with it off the backend registers no
   * global shortcut and creates no window. It may default on only after the I9
   * gate (A5 GO + the live smoke).
   */
  enabled: boolean;
  /** Ask the OS to keep the panel out of screen captures. */
  contentProtection: boolean;
  /** Master switch for every global shortcut, separate from `enabled`. */
  shortcutsEnabled: boolean;
  keybinds: Keybinds;
}

/**
 * A row in the Settings shortcut list. `registered` is the truth about the OS,
 * not about the setting: an action with no implementation yet (`ask`,
 * `captureScreen` — I3 and I6) is never registered, and says why.
 */
export interface ShortcutInfo {
  action: string;
  label: string;
  keybind: string;
  registered: boolean;
  unavailableReason: string | null;
}

export interface CopilotStatus {
  config: CopilotConfig;
  protection: ProtectionVerdict;
  panelOpen: boolean;
  /**
   * Is a recording session running? The panel shows live content only while
   * this is true — the copilot has no capture of its own; it follows a session
   * the user already consented to (ADR-0038 invariant 2).
   */
  recording: boolean;
  shortcuts: ShortcutInfo[];
}
