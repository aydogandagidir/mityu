'use client';

/**
 * PDF renderer (BACKLOG C4.2) — the PDF sibling of `renderExportMarkdown`.
 *
 * Consumes the SAME format-agnostic {@link ExportDoc} the Markdown/DOCX renderers
 * do (approved-only filtering already happened in `buildExportDoc`; do NOT
 * re-filter) and mirrors its structure:
 *
 *   <title>                                              (large bold)
 *   AI-generated · reviewed & approved by <who> on <when>  (provenance, muted)
 *   Meeting date: … • Model: … • Exported: …             (meta, when known)
 *   <section title>                                      (bold heading per section)
 *     heading1/heading2 -> bold, sized
 *     text              -> wrapped body via splitTextToSize
 *     bullet            -> "• …" wrapped body
 *   each line prefixes its "[MM:SS]" sourceTs when the model resolved one.
 *   Action Items                                         (bold heading)
 *     • [MM:SS] text — assignee — due                    (one wrapped row per item)
 *   Note: N action item(s) not yet approved — not included.  (footer, when >0)
 *
 * Layout is intentionally simple and robust: a single text column with a running
 * `y` cursor and manual pagination — when `y` would overflow the bottom margin we
 * `addPage()` and reset. Long lines wrap with `splitTextToSize`. No tables, no
 * embedded fonts (core Helvetica only) — this maximizes reliability in the
 * offline webview and keeps output deterministic in structure.
 *
 * OFFLINE + CLIENT-ONLY: `jspdf` is pure JavaScript, bundled into the static
 * build with ZERO network at runtime. It is imported DYNAMICALLY inside the
 * async render function: jspdf initializes browser-oriented globals on load, so
 * this guarantees it is never evaluated during Next's `output: 'export'`
 * prerender — it only loads in the Tauri webview when a user clicks Export.
 */

import type { ExportDoc, ExportItem, ExportActionItem } from './exportModel';

/** Page geometry in points (jsPDF default unit for the 'a4' format). */
const PAGE = {
  marginX: 48,
  marginTop: 56,
  marginBottom: 56,
  width: 595.28, // A4 width in pt
  height: 841.89, // A4 height in pt
} as const;

const CONTENT_WIDTH = PAGE.width - PAGE.marginX * 2;

/** Prefix an item's text with its `[MM:SS]` when present. */
function itemText(item: ExportItem): string {
  const ts = item.sourceTs ? `${item.sourceTs} ` : '';
  return `${ts}${item.text.trim()}`;
}

/** Join the parts of an action-item line with an em-dash, dropping blanks. */
function actionItemLine(item: ExportActionItem): string {
  const parts: string[] = [];
  if (item.sourceTs) parts.push(item.sourceTs);
  parts.push(item.text.trim());
  if (item.assignee && item.assignee.trim()) parts.push(item.assignee.trim());
  if (item.due && item.due.trim()) parts.push(item.due.trim());
  return parts.join(' — ');
}

/**
 * Render an {@link ExportDoc} to a PDF {@link Blob}.
 *
 * Async only because it dynamically imports `jspdf` (see module doc); the render
 * itself is synchronous once the library is loaded.
 *
 * @param doc - the approved-only, source-linked export document.
 * @returns a `Blob` of MIME type `application/pdf`.
 */
export async function renderExportPdf(doc: ExportDoc): Promise<Blob> {
  const { jsPDF } = await import('jspdf');
  const pdf = new jsPDF({ unit: 'pt', format: 'a4' });

  const { meta, sections, actionItems, excludedActionItemCount } = doc;

  // Running vertical cursor. A tiny closure toolkit keeps the flow readable and
  // the pagination in one place.
  let y = PAGE.marginTop;

  /** Ensure `needed` pt of vertical space remain, adding a page otherwise. */
  const ensureSpace = (needed: number): void => {
    if (y + needed > PAGE.height - PAGE.marginBottom) {
      pdf.addPage();
      y = PAGE.marginTop;
    }
  };

  /**
   * Write `text` as one paragraph (wrapped), advancing `y`. `indent` shifts the
   * left edge (used for bullet hanging text). Sets font per call.
   */
  const writeParagraph = (
    text: string,
    opts: {
      size: number;
      style?: 'normal' | 'bold' | 'italic';
      color?: [number, number, number];
      gapAfter?: number;
      indent?: number;
    },
  ): void => {
    const { size, style = 'normal', color = [17, 24, 39], gapAfter = 6, indent = 0 } = opts;
    pdf.setFont('helvetica', style);
    pdf.setFontSize(size);
    pdf.setTextColor(color[0], color[1], color[2]);

    const lineHeight = size * 1.35;
    const wrapWidth = CONTENT_WIDTH - indent;
    const lines = pdf.splitTextToSize(text, wrapWidth) as string[];
    for (const line of lines) {
      ensureSpace(lineHeight);
      pdf.text(line, PAGE.marginX + indent, y);
      y += lineHeight;
    }
    y += gapAfter;
  };

  // --- Header + provenance -------------------------------------------------
  writeParagraph(meta.title.trim() || 'Meeting Summary', { size: 22, style: 'bold', gapAfter: 8 });

  const approver = meta.approvedBy?.trim();
  const approvedOn = meta.approvedAt?.trim();
  let provenance: string;
  if (approver && approvedOn) {
    provenance = `AI-generated · reviewed & approved by ${approver} on ${approvedOn}`;
  } else if (approvedOn) {
    provenance = `AI-generated · reviewed & approved on ${approvedOn}`;
  } else {
    provenance = 'AI-generated · reviewed & approved by a human before export';
  }
  writeParagraph(provenance, { size: 10, style: 'italic', color: [107, 114, 128], gapAfter: 4 });

  const metaBits: string[] = [];
  if (meta.meetingDate?.trim()) metaBits.push(`Meeting date: ${meta.meetingDate.trim()}`);
  if (meta.model?.trim()) metaBits.push(`Model: ${meta.model.trim()}`);
  if (meta.exportedAt?.trim()) metaBits.push(`Exported: ${meta.exportedAt.trim()}`);
  if (metaBits.length > 0) {
    writeParagraph(metaBits.join('   •   '), {
      size: 9,
      color: [107, 114, 128],
      gapAfter: 12,
    });
  } else {
    y += 6;
  }

  // --- Sections ------------------------------------------------------------
  for (const section of sections) {
    writeParagraph(section.title.trim(), { size: 15, style: 'bold', gapAfter: 8 });
    for (const item of section.items) {
      const text = itemText(item);
      if (item.kind === 'heading1') {
        writeParagraph(text, { size: 14, style: 'bold', gapAfter: 6 });
      } else if (item.kind === 'heading2') {
        writeParagraph(text, { size: 12, style: 'bold', gapAfter: 6 });
      } else if (item.kind === 'bullet') {
        writeParagraph(`•  ${text}`, { size: 11, gapAfter: 4, indent: 12 });
      } else {
        writeParagraph(text, { size: 11, gapAfter: 6 });
      }
    }
    y += 4;
  }

  // --- Action items --------------------------------------------------------
  if (actionItems.length > 0) {
    writeParagraph('Action Items', { size: 15, style: 'bold', gapAfter: 8 });
    for (const ai of actionItems) {
      writeParagraph(`•  ${actionItemLine(ai)}`, { size: 11, gapAfter: 4, indent: 12 });
    }
    y += 4;
  }

  // --- Excluded-action-items footer (transparency) -------------------------
  if (excludedActionItemCount > 0) {
    const plural = excludedActionItemCount === 1 ? 'item' : 'items';
    writeParagraph(
      `Note: ${excludedActionItemCount} action ${plural} not yet approved — not included.`,
      { size: 9, style: 'italic', color: [146, 64, 14], gapAfter: 0 },
    );
  }

  return pdf.output('blob');
}
