/**
 * Summary Draft Service (BACKLOG C1.6)
 *
 * Typed 1-to-1 wrappers over the C1.5 HITL Tauri commands for source-linked
 * summary drafts. Mirrors the `services/search.ts` / `services/configService.ts`
 * pattern: this is the SINGLE typed definition of the draft wire shapes and the
 * only place these `invoke()` calls live — no raw invoke for these commands
 * anywhere else in the UI.
 *
 * ## Wire contract (matches `src-tauri/src/summary/draft.rs` + `summary/commands.rs`)
 * - The draft types are snake_case (serde default) and are reproduced verbatim
 *   here so the review UI renders the exact shapes the Rust core stores.
 * - Every mutation command returns a plain `boolean`. `false` is NOT an error:
 *   it means an illegal status transition, not-found, stale evidence, or a
 *   cross-workspace no-op. Callers treat `false` as "couldn't apply" (a soft
 *   inline note / toast), and only a thrown error as a real failure.
 * - Command errors are content-free strings (never block or transcript text).
 * - All args are camelCase over `invoke` (Tauri converts to snake_case Rust args).
 */

import { invoke } from '@tauri-apps/api/core';

/**
 * Review lifecycle of a single summary block or extracted action item.
 * Matches `BlockStatus` (`draft.rs`): nothing is ever auto-approved (HITL);
 * `approved`/`edited`/`rejected` are all the result of an explicit human action.
 */
export type BlockStatus = 'draft' | 'approved' | 'edited' | 'rejected';

/**
 * Review lifecycle of a whole summary. Matches `SummaryStatus` (`draft.rs`):
 * `draft` until a human approves it, then `approved`.
 */
export type SummaryStatus = 'draft' | 'approved';

/**
 * The visual kind of a summary block. Matches `BlockType` (`draft.rs`); the
 * trailing digit stays attached (`heading1`/`heading2`).
 */
export type DraftBlockType = 'text' | 'bullet' | 'heading1' | 'heading2';

/**
 * One summary block, always anchored to transcript evidence.
 * Mirror of the Rust `DraftBlock` (§4 `Block`).
 */
export interface DraftBlock {
  /** Block id (uuid). */
  id: string;
  /** Visual kind (serialized under the §4 wire key `type`). */
  type: DraftBlockType;
  /** The block text as currently displayed (generated, or human-edited). */
  content: string;
  /**
   * REQUIRED evidence anchor: the transcript chunk id this block is grounded
   * in. Immutable after creation. Drives jump-to-source.
   */
  source_chunk_id: string;
  /** Review status; a draft-generated block starts as `draft`. */
  status: BlockStatus;
  /**
   * The originally generated text, preserved on the FIRST human edit so an
   * `edited` block stays auditable against what the model produced. Absent
   * (`undefined`) until the block is edited.
   */
  original_content?: string;
}

/**
 * A titled group of blocks, in display order. Mirror of the Rust `DraftSection`
 * (§4 `Section`).
 */
export interface DraftSection {
  /** Section heading shown in the review surface. */
  title: string;
  /** The section's blocks, in display order. */
  blocks: DraftBlock[];
}

/**
 * A whole generated summary for one meeting — a DRAFT until a human approves it.
 * Mirror of the Rust `MeetingNotesDraft` (§4 `MeetingNotesDraft`).
 */
export interface MeetingNotesDraft {
  /** The meeting these notes belong to. */
  meeting_id: string;
  /** Summary-level review status. */
  status: SummaryStatus;
  /** The summary body, in display order. */
  sections: DraftSection[];
}

/**
 * An extracted action item, evidence-anchored and human-reviewed. Mirror of the
 * Rust `ActionItemDraft` (§4 `ActionItemDraft`).
 */
export interface ActionItemDraft {
  /** Action item id (uuid). */
  id: string;
  /** The action text. */
  text: string;
  /** Suggested owner, if inferred. Absent (`undefined`) when not set. */
  assignee?: string;
  /** Suggested due date (free text / ISO-8601), if inferred. Absent when unset. */
  due?: string;
  /** Review status; a draft-extracted item starts as `draft`. */
  status: BlockStatus;
  /** REQUIRED evidence anchor (same identifier space as `DraftBlock`). */
  source_chunk_id: string;
}

/**
 * The read-side payload from `api_get_summary_draft`.
 *
 * `draft` is `null` when the meeting has no (live) summary row — never
 * summarized, or the summary was soft-deleted. In that case the lifecycle
 * fields are `null`/`draft` and `action_items` may still be non-empty (action
 * items live in their own table).
 */
export interface SummaryDraftResponse {
  /** The hydrated §4 draft, or `null` when there is no summary row. */
  draft: MeetingNotesDraft | null;
  /** Summary-level HITL status (`draft` when there is no row). */
  status: SummaryStatus;
  /** Provider/model that generated the draft, if recorded. */
  model: string | null;
  /** Summary template used, if recorded. */
  template_id: string | null;
  /** When the draft was (re)generated (RFC 3339), if recorded. */
  generated_at: string | null;
  /** When a human approved the summary (RFC 3339), if approved. */
  approved_at: string | null;
  /** Who approved the summary, if approved. */
  approved_by: string | null;
  /** The meeting's live action-item drafts, in display order. */
  action_items: ActionItemDraft[];
}

/**
 * A tri-state patch for one nullable action-item field.
 * Mirror of the Rust `FieldPatch` (`commands.rs`) — the enum exists so "clear"
 * stays distinct from "leave unchanged" across JSON:
 * - `{ op: 'keep' }`  — leave the stored value unchanged,
 * - `{ op: 'clear' }` — set the stored value to null,
 * - `{ op: 'set', value }` — replace the stored value.
 */
export type FieldPatch =
  | { op: 'keep' }
  | { op: 'clear' }
  | { op: 'set'; value: string };

/**
 * The `req` payload for `api_edit_action_item`. Omit a field to leave it
 * unchanged (the backend defaults an absent `assignee`/`due` to `{ op: 'keep' }`
 * and an absent `text` to no-op). Mirror of the Rust `EditActionItemRequest`.
 */
/** Wire shape of `api_get_open_action_items` (Home dashboard). */
export interface OpenActionItem {
  id: string;
  meeting_id: string;
  meeting_title: string;
  text: string;
  assignee: string | null;
  due: string | null;
  status: BlockStatus;
  source_chunk_id: string;
}

export interface EditActionItemRequest {
  /** New action text; omit to leave unchanged. */
  text?: string;
  /** Assignee patch (keep / clear / set); omit = keep. */
  assignee?: FieldPatch;
  /** Due-date patch (keep / clear / set); omit = keep. */
  due?: FieldPatch;
}

/**
 * Summary Draft Service
 * Singleton service for the C1.5 HITL / source-linked draft commands.
 */
export class SummaryDraftService {
  /**
   * Read the meeting's structured summary draft (§4) and its live action items.
   * A meeting with no summary row yields `draft: null` (not an error).
   * @param meetingId - The meeting to read.
   */
  async getSummaryDraft(meetingId: string): Promise<SummaryDraftResponse> {
    return invoke<SummaryDraftResponse>('api_get_summary_draft', { meetingId });
  }

  /**
   * Human APPROVE of one summary block. Re-validates the block's evidence NOW.
   * @returns `false` for an illegal transition / unknown block / stale evidence
   *   (a soft no-op, not an error).
   */
  async approveBlock(meetingId: string, blockId: string): Promise<boolean> {
    return invoke<boolean>('api_approve_summary_block', { meetingId, blockId });
  }

  /**
   * Human REJECT of one summary block.
   * @returns `false` for an illegal transition / unknown block (soft no-op).
   */
  async rejectBlock(meetingId: string, blockId: string): Promise<boolean> {
    return invoke<boolean>('api_reject_summary_block', { meetingId, blockId });
  }

  /**
   * Human EDIT of one summary block's content. Never touches `source_chunk_id`;
   * the first edit preserves the generated text and drops the summary back to
   * draft.
   * @returns `false` for an unknown block, or a rejected block (restore first).
   */
  async editBlock(meetingId: string, blockId: string, content: string): Promise<boolean> {
    return invoke<boolean>('api_edit_summary_block', { meetingId, blockId, content });
  }

  /**
   * Human RESTORE of one rejected summary block back to draft.
   * @returns `false` for an illegal transition / unknown block (soft no-op).
   */
  async restoreBlock(meetingId: string, blockId: string): Promise<boolean> {
    return invoke<boolean>('api_restore_summary_block', { meetingId, blockId });
  }

  /**
   * The explicit HUMAN approval of the whole summary. The backend enforces the
   * gate NOW: at least one non-rejected block, every non-rejected block
   * approved, and each such block's evidence still resolving.
   * @returns `false` when the gate is not met (soft no-op).
   */
  async approveSummary(meetingId: string): Promise<boolean> {
    return invoke<boolean>('api_approve_summary', { meetingId });
  }

  /**
   * Cross-meeting open action items for the Home dashboard (Phase C): every
   * non-rejected, non-deleted item in the workspace, newest first, capped.
   */
  async getOpenActionItems(limit = 20): Promise<OpenActionItem[]> {
    return invoke<OpenActionItem[]>('api_get_open_action_items', { limit });
  }

  /**
   * Human APPROVE of one action item. Re-validates the item's evidence NOW.
   * @returns `false` for an illegal transition / unknown item / stale evidence.
   */
  async approveActionItem(itemId: string): Promise<boolean> {
    return invoke<boolean>('api_approve_action_item', { itemId });
  }

  /**
   * Human REJECT of one action item.
   * @returns `false` for an illegal transition / unknown item (soft no-op).
   */
  async rejectActionItem(itemId: string): Promise<boolean> {
    return invoke<boolean>('api_reject_action_item', { itemId });
  }

  /**
   * Human RESTORE of one rejected action item back to draft.
   * @returns `false` for an illegal transition / unknown item (soft no-op).
   */
  async restoreActionItem(itemId: string): Promise<boolean> {
    return invoke<boolean>('api_restore_action_item', { itemId });
  }

  /**
   * Human EDIT of one action item (patch semantics). Never touches
   * `source_chunk_id`; the first text edit preserves the generated text and
   * flips status to `edited`.
   * @param itemId - The action item to edit.
   * @param req - The field patch (send only what you want to change).
   * @returns `false` for nothing-to-change / unknown item / a rejected item.
   */
  async editActionItem(itemId: string, req: EditActionItemRequest): Promise<boolean> {
    return invoke<boolean>('api_edit_action_item', { itemId, req });
  }
}

/** Export singleton instance. */
export const summaryDraftService = new SummaryDraftService();
