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
 * - Summary: the top-level response AND its hydrated draft must both be
 *   `status === 'approved'`. Approved child items never imply whole-summary
 *   review; any other state fails closed before an export document exists.
 * - Blocks: emit only `status === 'approved'`. In an approved summary the backend
 *   gate guarantees every non-rejected block is `approved`, so this is equivalent
 *   to "exclude rejected", but we filter on `approved` explicitly and defensively.
 * - Action items: emit only `status === 'approved'`. Any non-approved item
 *   (`draft` / `edited` / `rejected`) is EXCLUDED and counted in
 *   `excludedActionItemCount` so the UI + the rendered footer can disclose it.
 * - A section with no approved blocks is dropped entirely (no empty headings).
 * - Source links: each emitted block/item's `source_chunk_id` MUST resolve to a
 *   non-empty timestamp. Missing evidence fails closed; no export doc is built.
 * - A `draft: null` response cannot prove whole-summary review and therefore
 *   fails closed even when it contains independently approved action items.
 */

import type {
  SummaryDraftResponse,
  DraftBlockType,
  MeetingNotesDraft,
  SummaryStatus,
} from '@/services/summaryDraftService';

/** The visual kind of an emitted summary line (subset mirrors DraftBlockType). */
export type ExportItemKind = 'heading1' | 'heading2' | 'text' | 'bullet';

/** One rendered line within a section with its verified source link. */
export interface ExportItem {
  id: string;
  kind: ExportItemKind;
  text: string;
  sourceChunkId: string;
  /** Recording-relative `[MM:SS]` or legacy wall-clock timestamp. */
  sourceTs: string;
}

/** A titled group of emitted lines, in display order. */
export interface ExportSection {
  title: string;
  items: ExportItem[];
}

/** One emitted action item with its verified source link. */
export interface ExportActionItem {
  id: string;
  text: string;
  assignee?: string;
  due?: string;
  sourceChunkId: string;
  /** Recording-relative `[MM:SS]` or legacy wall-clock timestamp. */
  sourceTs: string;
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

export interface ExportSourceLink {
  item_type: 'summary_block' | 'action_item';
  item_id: string;
  source_chunk_id: string;
  timestamp: string;
}

/** Stable, machine-readable provenance embedded in every export format. */
export interface ExportProvenance {
  schema: 'com.bluedev.mityu.export-provenance/v1';
  ai_generated: true;
  /** Derived from the export document's verified whole-summary status. */
  human_reviewed: boolean;
  source_linked: boolean;
  source_links: ExportSourceLink[];
  meeting_id: string;
  exported_at: string;
  approved_at?: string;
  approved_by?: string;
  model?: string;
}

/** A content-free, user-safe failure for an incomplete AI evidence chain. */
export class ExportSourceLinkError extends Error {
  constructor() {
    super('Export blocked because an approved AI item has no verified source timestamp.');
    this.name = 'ExportSourceLinkError';
  }
}

/** A content-free, user-safe failure for a missing whole-summary approval. */
export class ExportApprovalError extends Error {
  constructor() {
    super('Export blocked because the whole summary is not approved.');
    this.name = 'ExportApprovalError';
  }
}

type ApprovedSummaryResponse = SummaryDraftResponse & {
  status: 'approved';
  draft: MeetingNotesDraft & { status: 'approved' };
};

/**
 * Prove the whole-summary HITL boundary independently of child-item statuses.
 * Both copies of the lifecycle state are checked so a malformed/stale hydrated
 * payload cannot be promoted merely because its blocks happen to be approved.
 */
export function assertSummaryApprovedForExport(
  draftResponse: SummaryDraftResponse | null,
): asserts draftResponse is ApprovedSummaryResponse {
  if (
    draftResponse?.status !== 'approved' ||
    draftResponse.draft?.status !== 'approved'
  ) {
    throw new ExportApprovalError();
  }
}

function exportedItemCount(doc: ExportDoc): number {
  return (
    doc.sections.reduce((count, section) => count + section.items.length, 0) +
    doc.actionItems.length
  );
}

function collectSourceLinks(doc: ExportDoc): ExportSourceLink[] {
  const blockLinks = doc.sections.flatMap((section) =>
    section.items.map((item) => ({
      item_type: 'summary_block' as const,
      item_id: item.id,
      source_chunk_id: item.sourceChunkId,
      timestamp: item.sourceTs,
    })),
  );
  const actionLinks = doc.actionItems.map((item) => ({
    item_type: 'action_item' as const,
    item_id: item.id,
    source_chunk_id: item.sourceChunkId,
    timestamp: item.sourceTs,
  }));
  return [...blockLinks, ...actionLinks];
}

function nonEmpty(value: unknown): value is string {
  return typeof value === 'string' && value.trim().length > 0;
}

/** Build the shared provenance payload used by Markdown, DOCX, and PDF. */
export function buildExportProvenance(doc: ExportDoc): ExportProvenance {
  const { meta } = doc;
  const optional = (value: string | undefined): string | undefined => {
    const normalized = value?.trim();
    return normalized ? normalized : undefined;
  };

  const sourceLinks = collectSourceLinks(doc);
  const itemCount = exportedItemCount(doc);
  const sourceLinked =
    itemCount > 0 &&
    sourceLinks.length === itemCount &&
    sourceLinks.every(
      (link) =>
        nonEmpty(link.item_id) &&
        nonEmpty(link.source_chunk_id) &&
        nonEmpty(link.timestamp),
    );

  return {
    schema: 'com.bluedev.mityu.export-provenance/v1',
    ai_generated: true,
    human_reviewed: doc.summaryStatus === 'approved',
    source_linked: sourceLinked,
    source_links: sourceLinks,
    meeting_id: meta.meetingId,
    exported_at: meta.exportedAt,
    approved_at: optional(meta.approvedAt),
    approved_by: optional(meta.approvedBy),
    model: optional(meta.model),
  };
}

/** Renderer-side defence in depth for hand-built or malformed export docs. */
export function buildVerifiedExportProvenance(doc: ExportDoc): ExportProvenance {
  const provenance = buildExportProvenance(doc);
  if (!provenance.human_reviewed) {
    throw new ExportApprovalError();
  }
  if (exportedItemCount(doc) > 0 && !provenance.source_linked) {
    throw new ExportSourceLinkError();
  }
  return provenance;
}

/**
 * The neutral, render-ready export document. Every renderer targets THIS shape.
 */
export interface ExportDoc {
  meta: ExportMeta;
  /** Whole-summary lifecycle state; renderers verify this before emitting. */
  summaryStatus: SummaryStatus;
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
 * resolved `segmentTimestamps` (via the transcript fetch-all). This function
 * authoritatively re-checks whole-summary approval before filtering child items;
 * UI gating is only a convenience and is never the trust boundary.
 *
 * @param draftResponse - the read-side payload from `api_get_summary_draft`.
 *   A null, missing, stale, or non-approved summary blocks the export.
 * @param segmentTimestamps - `Map<source_chunk_id, "[MM:SS]">`; a missing or
 *   blank source id/timestamp blocks the export.
 * @param meta - identity/provenance for the document (title, dates, model…).
 */
export function buildExportDoc(
  draftResponse: SummaryDraftResponse | null,
  segmentTimestamps: Map<string, string>,
  meta: ExportMeta,
): ExportDoc {
  assertSummaryApprovedForExport(draftResponse);

  const sections: ExportSection[] = [];

  const verifiedSource = (
    itemId: string,
    sourceChunkId: string,
  ): { id: string; sourceChunkId: string; sourceTs: string } => {
    const id = itemId.trim();
    const normalizedSourceChunkId = sourceChunkId.trim();
    const sourceTs = segmentTimestamps.get(normalizedSourceChunkId)?.trim();
    if (!id || !normalizedSourceChunkId || !sourceTs) {
      throw new ExportSourceLinkError();
    }
    return { id, sourceChunkId: normalizedSourceChunkId, sourceTs };
  };

  const draftSections = draftResponse.draft.sections;
  for (const section of draftSections) {
    const items: ExportItem[] = [];
    for (const block of section.blocks) {
      // Approved-only: exclude draft / edited / rejected blocks.
      if (block.status !== 'approved') continue;
      const source = verifiedSource(block.id, block.source_chunk_id);
      items.push({
        ...source,
        kind: toItemKind(block.type),
        text: block.content,
      });
    }
    // Drop sections that have no approved blocks — no empty headings.
    if (items.length > 0) {
      sections.push({ title: section.title, items });
    }
  }

  const actionItems: ExportActionItem[] = [];
  let excludedActionItemCount = 0;
  for (const item of draftResponse.action_items) {
    // Approved-only: any non-approved action item is excluded AND counted.
    if (item.status !== 'approved') {
      excludedActionItemCount += 1;
      continue;
    }
    const source = verifiedSource(item.id, item.source_chunk_id);
    actionItems.push({
      ...source,
      text: item.text,
      assignee: item.assignee,
      due: item.due,
    });
  }

  return {
    meta,
    summaryStatus: draftResponse.status,
    sections,
    actionItems,
    excludedActionItemCount,
  };
}
