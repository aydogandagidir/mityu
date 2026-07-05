/**
 * Export model (BACKLOG C4.1) — the shared, format-agnostic export document.
 *
 * This is the SEAM for the whole C4 export family. `buildExportDoc` turns an
 * approved, source-linked summary draft into a neutral `ExportDoc` that every
 * renderer (C4.1 Markdown now; C4.2 DOCX / C4.3 PDF later) consumes without
 * knowing anything about draft wire shapes, statuses, or transcript rows. Adding
 * a format = adding a renderer over this model, nothing else.
 *
 * It is PURE — no `invoke`, no DOM, no clock. The caller resolves timestamps and
 * supplies `meta` (including `exportedAt`); this function only filters, maps, and
 * counts. That is what makes it unit-testable and deterministic.
 *
 * ## Rules it enforces (ADR-0019 decision 1 — approved-only)
 * - Blocks: emit only `status === 'approved'`. In an approved summary the backend
 *   gate guarantees every non-rejected block is `approved`, so this is equivalent
 *   to "exclude rejected", but we filter on `approved` explicitly and defensively.
 * - Action items: emit only `status === 'approved'`. Any non-approved item
 *   (`draft` / `edited` / `rejected`) is EXCLUDED and counted in
 *   `excludedActionItemCount` so the UI + the rendered footer can disclose it.
 * - A section with no approved blocks is dropped entirely (no empty headings).
 * - Timestamps: each block/item's `source_chunk_id` is looked up in the caller's
 *   `Map<source_chunk_id, "[MM:SS]">`. An unresolved id yields `sourceTs:
 *   undefined` (the segment is gone / not fetched) — never a fabricated time.
 * - A `draft: null` response (never summarized / soft-deleted) degrades to a doc
 *   with no sections; approved action items (if any) still flow through.
 */

import type {
  SummaryDraftResponse,
  DraftBlockType,
} from '@/services/summaryDraftService';

/** The visual kind of an emitted summary line (subset mirrors DraftBlockType). */
export type ExportItemKind = 'heading1' | 'heading2' | 'text' | 'bullet';

/** One rendered line within a section, optionally carrying its source `[MM:SS]`. */
export interface ExportItem {
  kind: ExportItemKind;
  text: string;
  /** Recording-relative `[MM:SS]` of the backing segment; absent if unresolved. */
  sourceTs?: string;
}

/** A titled group of emitted lines, in display order. */
export interface ExportSection {
  title: string;
  items: ExportItem[];
}

/** One emitted action item, optionally carrying its source `[MM:SS]`. */
export interface ExportActionItem {
  text: string;
  assignee?: string;
  due?: string;
  /** Recording-relative `[MM:SS]` of the backing segment; absent if unresolved. */
  sourceTs?: string;
}

/** Provenance / identity for the exported document (all optional but title). */
export interface ExportMeta {
  meetingId: string;
  title: string;
  /** Human-readable meeting date, if known (already formatted by the caller). */
  meetingDate?: string;
  /** When this export was produced (ISO or human string, caller-supplied). */
  exportedAt: string;
  /** When a human approved the summary (RFC 3339 from the draft), if approved. */
  approvedAt?: string;
  /** Who approved the summary, if recorded. */
  approvedBy?: string;
  /** Provider/model that generated the draft, if recorded. */
  model?: string;
}

/**
 * The neutral, render-ready export document. Every renderer targets THIS shape.
 */
export interface ExportDoc {
  meta: ExportMeta;
  sections: ExportSection[];
  actionItems: ExportActionItem[];
  /** How many action items were dropped because they were not yet approved. */
  excludedActionItemCount: number;
}

/** Map a draft block's `type` onto the neutral item kind (1:1 today). */
function toItemKind(type: DraftBlockType): ExportItemKind {
  // DraftBlockType and ExportItemKind are the same four literals; this keeps the
  // model independent of the wire enum and future-proofs a divergence.
  switch (type) {
    case 'heading1':
      return 'heading1';
    case 'heading2':
      return 'heading2';
    case 'bullet':
      return 'bullet';
    case 'text':
    default:
      return 'text';
  }
}

/**
 * Build the format-agnostic {@link ExportDoc} from an approved summary draft.
 *
 * PURE: it performs no I/O. The caller is responsible for having already
 * resolved `segmentTimestamps` (via the transcript fetch-all) and for approving
 * the summary — this function does not re-check the summary-level status (the
 * Export control gates on that), it only filters item-level statuses.
 *
 * @param draftResponse - the read-side payload from `api_get_summary_draft`
 *   (or `null`, yielding a degraded, empty-section doc).
 * @param segmentTimestamps - `Map<source_chunk_id, "[MM:SS]">`; a missing key
 *   means the timestamp is unresolved and the item is emitted without one.
 * @param meta - identity/provenance for the document (title, dates, model…).
 */
export function buildExportDoc(
  draftResponse: SummaryDraftResponse | null,
  segmentTimestamps: Map<string, string>,
  meta: ExportMeta,
): ExportDoc {
  const sections: ExportSection[] = [];

  const draftSections = draftResponse?.draft?.sections ?? [];
  for (const section of draftSections) {
    const items: ExportItem[] = [];
    for (const block of section.blocks) {
      // Approved-only: exclude draft / edited / rejected blocks.
      if (block.status !== 'approved') continue;
      items.push({
        kind: toItemKind(block.type),
        text: block.content,
        sourceTs: segmentTimestamps.get(block.source_chunk_id),
      });
    }
    // Drop sections that have no approved blocks — no empty headings.
    if (items.length > 0) {
      sections.push({ title: section.title, items });
    }
  }

  const actionItems: ExportActionItem[] = [];
  let excludedActionItemCount = 0;
  for (const item of draftResponse?.action_items ?? []) {
    // Approved-only: any non-approved action item is excluded AND counted.
    if (item.status !== 'approved') {
      excludedActionItemCount += 1;
      continue;
    }
    actionItems.push({
      text: item.text,
      assignee: item.assignee,
      due: item.due,
      sourceTs: segmentTimestamps.get(item.source_chunk_id),
    });
  }

  return { meta, sections, actionItems, excludedActionItemCount };
}
