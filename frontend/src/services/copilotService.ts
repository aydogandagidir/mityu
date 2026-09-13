/**
 * Copilot Service (BACKLOG I1, ADR-0038)
 *
 * The only place the copilot's Tauri commands are named — no raw `invoke` in
 * components (docs/CONVENTIONS.md).
 *
 * Outside the Tauri shell (the `/design/*` review routes, a browser preview)
 * `invoke` would throw, so every read resolves to a disabled stub and every
 * write is refused. That keeps the design fixture renderable without pretending
 * a backend answered.
 */

import { invoke } from '@tauri-apps/api/core';
import { isTauri } from '@/lib/isTauri';
import type { CopilotConfig, CopilotStatus } from '@/types/copilot';

/**
 * What the design routes see. Mirrors the Rust defaults: off, protection
 * requested, shortcuts on, and the platform verdict left deliberately vague
 * because no platform answered.
 */
const BROWSER_STUB: CopilotStatus = {
  config: {
    enabled: false,
    contentProtection: true,
    shortcutsEnabled: true,
    keybinds: {
      togglePanel: 'CommandOrControl+Shift+M',
      ask: 'CommandOrControl+Shift+A',
      captureScreen: 'CommandOrControl+Shift+S',
    },
  },
  protection: {
    level: 'unsupported',
    headline: 'Screen-sharing behaviour unknown',
    detail: 'Preview only — no desktop app is running, so the platform has not been asked.',
  },
  panelOpen: false,
  recording: false,
  shortcuts: [],
};

export class CopilotService {
  /** Settings, platform verdict, panel and recording state in one read. */
  async getStatus(): Promise<CopilotStatus> {
    if (!isTauri()) return structuredClone(BROWSER_STUB);
    return invoke<CopilotStatus>('copilot_get_status');
  }

  /**
   * Save settings and apply them to the running app.
   *
   * @throws A human sentence when a shortcut is invalid — the backend
   * validates every binding before storing any of them, so a rejected save
   * changes nothing.
   */
  async setConfig(config: CopilotConfig): Promise<CopilotStatus> {
    if (!isTauri()) return structuredClone(BROWSER_STUB);
    return invoke<CopilotStatus>('copilot_set_config', { config });
  }

  /** Show or hide the panel — the same action as the global shortcut. */
  async togglePanel(): Promise<void> {
    if (!isTauri()) return;
    await invoke('copilot_toggle_panel');
  }

  /** Close the panel (its own close button). */
  async closePanel(): Promise<void> {
    if (!isTauri()) return;
    await invoke('copilot_close_panel');
  }

  /**
   * Bring the main window forward. The panel uses this instead of duplicating
   * controls that belong to the main window — stopping a recording above all.
   */
  async focusMainWindow(): Promise<void> {
    if (!isTauri()) return;
    await invoke('copilot_focus_main_window');
  }
}

export const copilotService = new CopilotService();
