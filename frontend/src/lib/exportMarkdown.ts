/**
 * Markdown renderer (BACKLOG C4.1) — the first renderer over {@link ExportDoc}.
 *
 * Deterministic and pure: same `ExportDoc` in → byte-identical Markdown out (no
 * clock, no randomness). C4.2 (DOCX) and C4.3 (PDF) are siblings that consume the
 * same model; this file is the reference for how the model maps to an output.
 *
 * Layout:
 *   # <title>
 *   > AI-generated · reviewed & approved by <who> on <when>   (provenance line)
 *   **Meeting date:** …  •  **Model:** …  •  **Exported:** …   (when known)
 *   ---
 *   ## <section title>
 *     #  heading1        (with a leading/trailing [MM:SS] when resolved)
 *     ## heading2
 *     paragraph text
 *     - bullet
 *   ## Action Items
 *     - [MM:SS] text (Assignee: …, Due: …)
 *   ---
 *   _Note: N action item(s) were not yet approved and are not included._
 *
 * The `[MM:SS]` source stamp is emitted whenever the model resolved one; it is a
 * transparency + evidence affordance (EU AI Act Art. 50 / HITL source linkage),
 * so it is prefixed on paragraph/heading lines and inside the bullet marker.
 */

import type { ExportDoc, ExportItem } from './exportModel';

/** Markdown heading prefix for each item kind ('' = not a heading). */
const HEADING_HASHES: Record<ExportItem['kind'], string> = {
  heading1: '# ',
  heading2: '## ',
  text: '',
  bullet: '',
};

/** Render one section item line, prefixing its `[MM:SS]` when present. */
function renderItem(item: ExportItem): string {
  const ts = item.sourceTs ? `${item.sourceTs} ` : '';
  const text = item.text.trim();
  if (item.kind === 'bullet') {
    return `- ${ts}${text}`;
  }
  // heading1 / heading2 / text
  return `${HEADING_HASHES[item.kind]}${ts}${text}`;
}

/** Render the `(Assignee: …, Due: …)` suffix for an action item, if any. */
function renderActionMeta(assignee?: string, due?: string): string {
  const parts: string[] = [];
  if (assignee && assignee.trim()) parts.push(`Assignee: ${assignee.trim()}`);
  if (due && due.trim()) parts.push(`Due: ${due.trim()}`);
  return parts.length > 0 ? ` (${parts.join(', ')})` : '';
}

/**
 * Render an {@link ExportDoc} to a Markdown string. Deterministic.
 */
export function renderExportMarkdown(doc: ExportDoc): string {
  const { meta, sections, actionItems, excludedActionItemCount } = doc;
  const lines: string[] = [];

  // --- Header + provenance -------------------------------------------------
  lines.push(`# ${meta.title.trim() || 'Meeting Summary'}`);
  lines.push('');

  // The AI-generated + human-approval provenance line (transparency, non-negotiable).
  const approver = meta.approvedBy?.trim();
  const approvedOn = meta.approvedAt?.trim();
  if (approver && approvedOn) {
    lines.push(`> AI-generated · reviewed & approved by ${approver} on ${approvedOn}`);
  } else if (approvedOn) {
    lines.push(`> AI-generated · reviewed & approved on ${approvedOn}`);
  } else {
    lines.push('> AI-generated · reviewed & approved by a human before export');
  }
  lines.push('');

  // Optional metadata line (only the parts we actually know).
  const metaBits: string[] = [];
  if (meta.meetingDate?.trim()) metaBits.push(`**Meeting date:** ${meta.meetingDate.trim()}`);
  if (meta.model?.trim()) metaBits.push(`**Model:** ${meta.model.trim()}`);
  if (meta.exportedAt?.trim()) metaBits.push(`**Exported:** ${meta.exportedAt.trim()}`);
  if (metaBits.length > 0) {
    lines.push(metaBits.join('  •  '));
    lines.push('');
  }

  lines.push('---');
  lines.push('');

  // --- Sections ------------------------------------------------------------
  for (const section of sections) {
    lines.push(`## ${section.title.trim()}`);
    lines.push('');
    let prevWasBullet = false;
    for (const item of section.items) {
      const rendered = renderItem(item);
      const isBullet = item.kind === 'bullet';
      // Separate a bullet list from a preceding non-bullet with a blank line.
      if (isBullet && !prevWasBullet && lines[lines.length - 1] !== '') {
        lines.push('');
      }
      lines.push(rendered);
      // Headings/paragraphs get a trailing blank line; consecutive bullets don't.
      if (!isBullet) {
        lines.push('');
      }
      prevWasBullet = isBullet;
    }
    // Ensure a blank line closes the section.
    if (lines[lines.length - 1] !== '') {
      lines.push('');
    }
  }

  // --- Action items --------------------------------------------------------
  if (actionItems.length > 0) {
    lines.push('## Action Items');
    lines.push('');
    for (const ai of actionItems) {
      const ts = ai.sourceTs ? `${ai.sourceTs} ` : '';
      const suffix = renderActionMeta(ai.assignee, ai.due);
      lines.push(`- ${ts}${ai.text.trim()}${suffix}`);
    }
    lines.push('');
  }

  // --- Excluded-action-items footer (transparency) -------------------------
  if (excludedActionItemCount > 0) {
    lines.push('---');
    lines.push('');
    const plural = excludedActionItemCount === 1 ? 'item' : 'items';
    lines.push(
      `_Note: ${excludedActionItemCount} action ${plural} not yet approved — not included._`,
    );
    lines.push('');
  }

  // Collapse any trailing blank lines to a single terminal newline (determinism).
  let out = lines.join('\n');
  out = out.replace(/\n+$/,'\n');
  return out;
}
