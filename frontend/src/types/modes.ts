/**
 * Meeting modes (BACKLOG I4a/I4b, ADR-0038).
 *
 * A mode answers what only matters *during* a conversation: who the user is in
 * it, who the other side is, which of the copilot's actions are appropriate,
 * how strictly an answer must cite, and what it may read. Summary templates
 * answer the separate question of how the notes are written afterwards; a mode
 * points at one by id.
 *
 * These mirror `src-tauri/src/modes/types.rs` and `modes/commands.rs`. Every
 * rule about them is enforced in Rust — the UI renders what it is handed.
 */

import type { LiveAction } from './copilot';

/** How strictly an answer must rest on retrieved evidence. */
export type EvidencePolicy = 'sourceFirst' | 'open';

/**
 * Where an answer's evidence may come from.
 *
 * `knowledge` (I5) and `screen` (I6) can be *declared* by a mode written today
 * but cannot be read from yet; Rust builds prompts from the usable subset, so a
 * mode listing them reads the transcript rather than resting on nothing.
 */
export type AllowedSource = 'transcript' | 'knowledge' | 'screen';

export interface LivePolicy {
  allowedActions: LiveAction[];
  evidencePolicy: EvidencePolicy;
  citeRequired: boolean;
}

export interface Mode {
  id: string;
  name: string;
  purpose: string;
  userRole: string;
  counterpartRole: string;
  voice: string;
  summaryTemplateId: string;
  live: LivePolicy;
  allowedSources: AllowedSource[];
}

/**
 * One row of the Settings list: a mode plus the two things the UI cannot
 * derive.
 */
export interface ModeRow extends Mode {
  /** Built-ins are neither editable nor removable. */
  builtin: boolean;
  active: boolean;
}

/**
 * What every mutating command returns.
 *
 * `activeModeId` is the **resolved** id, not the stored one: a stored pointer
 * at a deleted mode resolves to the default, and the list must agree with what
 * Rust will actually answer with.
 */
export interface ModesView {
  modes: ModeRow[];
  activeModeId: string;
}
