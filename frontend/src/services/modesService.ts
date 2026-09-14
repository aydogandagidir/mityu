/**
 * The renderer's only door to the modes commands (BACKLOG I4b).
 *
 * `docs/CONVENTIONS.md`: no raw `invoke` in a component. Everything a
 * component needs is a method here, and the browser fallbacks below are what
 * let `/design/*` render without a backend.
 *
 * Validation is deliberately **not** mirrored here. A second copy of the
 * collision and rejected-category rules would drift from
 * `modes::validator`, and the one in Rust is the one that actually guards the
 * store — so `preview` asks the backend and shows its sentence.
 */

import { invoke } from '@tauri-apps/api/core';
import { isTauri } from '@/lib/isTauri';
import type { Mode, ModesView } from '@/types/modes';

/**
 * What the design routes see: an empty list rather than invented modes.
 *
 * Fabricating the eight built-ins here would make the design route disagree
 * with the app the day a built-in changes, and the list is the backend's answer
 * — not something the frontend knows.
 */
const BROWSER_STUB: ModesView = { modes: [], activeModeId: 'general' };

class ModesService {
  /** The installed modes and which one answers. */
  async list(): Promise<ModesView> {
    if (!isTauri()) return structuredClone(BROWSER_STUB);
    return invoke<ModesView>('modes_list');
  }

  /** Choose the mode the copilot builds its next insight from. */
  async setActive(modeId: string): Promise<ModesView> {
    if (!isTauri()) return structuredClone(BROWSER_STUB);
    return invoke<ModesView>('modes_set_active', { modeId });
  }

  /**
   * Check a mode file and return what it would install, **without installing**.
   *
   * Rejects with the validator's own sentence — size, collisions, empty fields
   * and the ADR-0038 rejected category all surface here, before anything is
   * written.
   */
  async preview(raw: string): Promise<Mode> {
    if (!isTauri()) throw new Error('Importing a mode needs the desktop app.');
    return invoke<Mode>('modes_preview', { raw });
  }

  /** Install a previewed mode. Re-validated in Rust, never trusted from here. */
  async install(raw: string): Promise<ModesView> {
    if (!isTauri()) return structuredClone(BROWSER_STUB);
    return invoke<ModesView>('modes_install', { raw });
  }

  /**
   * Edit a custom mode's wording.
   *
   * Name, purpose and voice only. What a mode *permits* — its id, actions and
   * sources — is authored in the file, not edited in a text box: widening a
   * permission is an install.
   */
  async update(
    modeId: string,
    fields: { name: string; purpose: string; voice: string }
  ): Promise<ModesView> {
    if (!isTauri()) return structuredClone(BROWSER_STUB);
    return invoke<ModesView>('modes_update', { modeId, ...fields });
  }

  /** Remove a custom mode. If it was active, Rust falls back to the default. */
  async remove(modeId: string): Promise<ModesView> {
    if (!isTauri()) return structuredClone(BROWSER_STUB);
    return invoke<ModesView>('modes_remove', { modeId });
  }
}

export const modesService = new ModesService();
