'use client';

/**
 * DOCX renderer (BACKLOG C4.2) — the Word sibling of `renderExportMarkdown`.
 *
 * Consumes the SAME format-agnostic {@link ExportDoc} the Markdown renderer does
 * (approved-only filtering already happened in `buildExportDoc`; do NOT re-filter
 * here) and mirrors its structure line-for-line:
 *
 *   <title>                                              (Title)
 *   AI-generated · reviewed & approved by <who> on <when>  (provenance)
 *   Meeting date: … • Model: … • Exported: …             (meta, when known)
 *   ── section title ──                                  (HEADING_1 per section)
 *     heading1 -> HEADING_1, heading2 -> HEADING_2
 *     text     -> paragraph
 *     bullet   -> bulleted paragraph
 *   each line prefixes its "[MM:SS]" sourceTs when the model resolved one.
 *   Action Items                                         (HEADING_1)
 *     • text — assignee — due — [MM:SS]                  (one row per item)
 *   Note: N action item(s) not yet approved — not included.  (footer, when >0)
 *
 * The `[MM:SS]` stamp is a transparency + evidence affordance (EU AI Act Art. 50
 * / HITL source linkage), emitted whenever `buildExportDoc` resolved one.
 *
 * OFFLINE + CLIENT-ONLY: `docx` is pure JavaScript, bundled into the static build
 * with ZERO network at runtime. It is imported DYNAMICALLY inside the async
 * render function so it is never evaluated during Next's `output: 'export'`
 * prerender pass — it only loads in the Tauri webview when a user clicks Export.
 *
 * Deterministic structure: same `ExportDoc` in → same document tree out (the
 * only non-determinism a .docx carries is the zip's internal timestamps, which
 * `docx` owns; the logical content is stable).
 */

import type { ExportDoc, ExportItem, ExportActionItem } from './exportModel';

/** Join the parts of an action-item line with an em-dash, dropping blanks. */
function actionItemLine(item: ExportActionItem): string {
  const parts: string[] = [];
  if (item.sourceTs) parts.push(item.sourceTs);
  parts.push(item.text.trim());
  if (item.assignee && item.assignee.trim()) parts.push(item.assignee.trim());
  if (item.due && item.due.trim()) parts.push(item.due.trim());
  return parts.join(' — ');
}

/** Prefix an item's text with its `[MM:SS]` when present (heading/text/bullet). */
function itemText(item: ExportItem): string {
  const ts = item.sourceTs ? `${item.sourceTs} ` : '';
  return `${ts}${item.text.trim()}`;
}

/**
 * Render an {@link ExportDoc} to a Word (.docx) {@link Blob}.
 *
 * @param doc - the approved-only, source-linked export document.
 * @returns a `Blob` of MIME type
 *   `application/vnd.openxmlformats-officedocument.wordprocessingml.document`.
 */
export async function renderExportDocx(doc: ExportDoc): Promise<Blob> {
  // Dynamic import keeps `docx` out of any SSR/prerender evaluation and code-
  // splits it out of the initial bundle; it resolves from the local install
  // (no network) the first time a user exports.
  const { Document, Packer, Paragraph, TextRun, HeadingLevel } = await import('docx');

  const { meta, sections, actionItems, excludedActionItemCount } = doc;
  const children: import('docx').Paragraph[] = [];

  // --- Header + provenance -------------------------------------------------
  children.push(
    new Paragraph({
      heading: HeadingLevel.TITLE,
      children: [new TextRun(meta.title.trim() || 'Meeting Summary')],
    }),
  );

  // The AI-generated + human-approval provenance line (transparency, non-negotiable).
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
  children.push(
    new Paragraph({
      children: [new TextRun({ text: provenance, italics: true, color: '666666' })],
    }),
  );

  // Optional metadata line (only the parts we actually know).
  const metaBits: string[] = [];
  if (meta.meetingDate?.trim()) metaBits.push(`Meeting date: ${meta.meetingDate.trim()}`);
  if (meta.model?.trim()) metaBits.push(`Model: ${meta.model.trim()}`);
  if (meta.exportedAt?.trim()) metaBits.push(`Exported: ${meta.exportedAt.trim()}`);
  if (metaBits.length > 0) {
    children.push(
      new Paragraph({
        children: [new TextRun({ text: metaBits.join('  •  '), color: '666666', size: 18 })],
      }),
    );
  }

  // --- Sections ------------------------------------------------------------
  for (const section of sections) {
    children.push(
      new Paragraph({
        heading: HeadingLevel.HEADING_1,
        children: [new TextRun(section.title.trim())],
      }),
    );
    for (const item of section.items) {
      const text = itemText(item);
      if (item.kind === 'heading1') {
        children.push(
          new Paragraph({ heading: HeadingLevel.HEADING_1, children: [new TextRun(text)] }),
        );
      } else if (item.kind === 'heading2') {
        children.push(
          new Paragraph({ heading: HeadingLevel.HEADING_2, children: [new TextRun(text)] }),
        );
      } else if (item.kind === 'bullet') {
        children.push(
          new Paragraph({ bullet: { level: 0 }, children: [new TextRun(text)] }),
        );
      } else {
        // text
        children.push(new Paragraph({ children: [new TextRun(text)] }));
      }
    }
  }

  // --- Action items --------------------------------------------------------
  if (actionItems.length > 0) {
    children.push(
      new Paragraph({
        heading: HeadingLevel.HEADING_1,
        children: [new TextRun('Action Items')],
      }),
    );
    for (const ai of actionItems) {
      children.push(
        new Paragraph({ bullet: { level: 0 }, children: [new TextRun(actionItemLine(ai))] }),
      );
    }
  }

  // --- Excluded-action-items footer (transparency) -------------------------
  if (excludedActionItemCount > 0) {
    const plural = excludedActionItemCount === 1 ? 'item' : 'items';
    children.push(
      new Paragraph({
        children: [
          new TextRun({
            text: `Note: ${excludedActionItemCount} action ${plural} not yet approved — not included.`,
            italics: true,
            color: '92400E',
            size: 18,
          }),
        ],
      }),
    );
  }

  const document = new Document({ sections: [{ children }] });
  return Packer.toBlob(document);
}
