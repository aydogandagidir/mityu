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
 * Every emitted item carries a verified source stamp; it is a
 * transparency + evidence affordance (EU AI Act Art. 50 / HITL source linkage),
 * so it is prefixed on paragraph/heading lines and inside the bullet marker.
 */

import {
  buildVerifiedExportProvenance,
  type ExportDoc,
  type ExportItem,
} from './exportModel';

/** Markdown heading prefix for each item kind ('' = not a heading). */
const HEADING_HASHES: Record<ExportItem['kind'], string> = {
  heading1: '# ',
  heading2: '## ',
  text: '',
  bullet: '',
};

/**
 * Emit untrusted meeting/AI text as one passive Markdown inline.
 *
 * Newlines are collapsed so content cannot introduce a new block, raw HTML is
 * entity-encoded, autolink punctuation is visibly separated, and link/image
 * label delimiters are escaped.
 * The rendered text remains readable, but it cannot create HTML, an image fetch,
 * or a clickable link in CommonMark/GFM renderers.
 */
function renderPassiveMarkdownText(value: string): string {
  return value
    .replace(/[\r\n\u2028\u2029\t]+/g, ' ')
    .replace(/[\u0000-\u0008\u000b\u000c\u000e-\u001f\u007f]/g, '')
    .replace(/\s+/g, ' ')
    .trim()
    .replace(/\\/g, '\\\\')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/\[/g, '\\[')
    .replace(/\]/g, '\\]')
    // Deliberately visible separators remain readable in plain text and cannot
    // be normalized by a Markdown parser into an active remote destination.
    .replace(/\b(https?|ftp|mailto|javascript|data):/gi, '$1 :')
    .replace(/\bwww\.(?=[a-z0-9-])/gi, 'www .')
    .replace(/@/g, ' @ ');
}

/** Keep canonical source stamps readable while neutralizing legacy free text. */
function renderSourceTimestamp(value: string): string {
  const normalized = value.trim();
  return /^\[\d+:[0-5]\d\]$/.test(normalized)
    ? normalized
    : renderPassiveMarkdownText(normalized);
}

/**
 * JSON-compatible YAML scalar rendering for machine provenance.
 * Only characters inside JSON strings are Unicode-escaped; array/object syntax
 * remains machine-readable while embedded values cannot become Markdown links,
 * images, raw HTML, or GFM autolinks when front matter is shown as plain text.
 */
function stringifyProvenanceValue(value: unknown): string {
  const json = JSON.stringify(value);
  if (json === undefined) return 'null';
  const escapedCharacters: Record<string, string> = {
    '<': '\\u003c',
    '>': '\\u003e',
    '[': '\\u005b',
    ']': '\\u005d',
    '(': '\\u0028',
    ')': '\\u0029',
    '!': '\\u0021',
    ':': '\\u003a',
    '@': '\\u0040',
  };
  let inString = false;
  let escaped = false;
  let stringTail = '';
  let output = '';

  for (const character of json) {
    if (!inString) {
      output += character;
      if (character === '"') {
        inString = true;
        stringTail = '';
      }
      continue;
    }
    if (escaped) {
      output += character;
      escaped = false;
      stringTail = '';
      continue;
    }
    if (character === '\\') {
      output += character;
      escaped = true;
      stringTail = '';
      continue;
    }
    if (character === '"') {
      output += character;
      inString = false;
      stringTail = '';
      continue;
    }
    output +=
      character === '.' && stringTail.toLowerCase().endsWith('www')
        ? '\\u002e'
        : (escapedCharacters[character] ?? character);
    stringTail = `${stringTail}${character}`.slice(-3);
  }

  return output;
}

/** Render one section item line, prefixing its verified source timestamp. */
function renderItem(item: ExportItem): string {
  const ts = item.sourceTs ? `${renderSourceTimestamp(item.sourceTs)} ` : '';
  const text = renderPassiveMarkdownText(item.text);
  if (item.kind === 'bullet') {
    return `- ${ts}${text}`;
  }
  // heading1 / heading2 / text
  return `${HEADING_HASHES[item.kind]}${ts}${text}`;
}

/** Render the `(Assignee: …, Due: …)` suffix for an action item, if any. */
function renderActionMeta(assignee?: string, due?: string): string {
  const parts: string[] = [];
  if (assignee && assignee.trim()) {
    parts.push(`Assignee: ${renderPassiveMarkdownText(assignee)}`);
  }
  if (due && due.trim()) {
    parts.push(`Due: ${renderPassiveMarkdownText(due)}`);
  }
  return parts.length > 0 ? ` (${parts.join(', ')})` : '';
}

/**
 * Render an {@link ExportDoc} to a Markdown string. Deterministic.
 */
export function renderExportMarkdown(doc: ExportDoc): string {
  const { meta, sections, actionItems, excludedActionItemCount } = doc;
  const lines: string[] = [];

  // YAML front matter preserves the same transparency contract for machines
  // that consume an export without rendering its visible approval banner.
  const provenance = buildVerifiedExportProvenance(doc);
  lines.push('---');
  for (const [key, value] of Object.entries(provenance)) {
    if (value !== undefined) lines.push(`${key}: ${stringifyProvenanceValue(value)}`);
  }
  lines.push('---');
  lines.push('');

  // --- Header + provenance -------------------------------------------------
  lines.push(`# ${renderPassiveMarkdownText(meta.title) || 'Meeting Summary'}`);
  lines.push('');

  // The AI-generated + human-approval provenance line (transparency, non-negotiable).
  const approver = meta.approvedBy?.trim();
  const approvedOn = meta.approvedAt?.trim();
  if (approver && approvedOn) {
    lines.push(
      `> AI-generated · reviewed & approved by ${renderPassiveMarkdownText(approver)} on ${renderPassiveMarkdownText(approvedOn)}`,
    );
  } else if (approvedOn) {
    lines.push(
      `> AI-generated · reviewed & approved on ${renderPassiveMarkdownText(approvedOn)}`,
    );
  } else {
    lines.push('> AI-generated · reviewed & approved by a human before export');
  }
  lines.push('');

  // Optional metadata line (only the parts we actually know).
  const metaBits: string[] = [];
  if (meta.meetingDate?.trim()) {
    metaBits.push(`**Meeting date:** ${renderPassiveMarkdownText(meta.meetingDate)}`);
  }
  if (meta.model?.trim()) {
    metaBits.push(`**Model:** ${renderPassiveMarkdownText(meta.model)}`);
  }
  if (meta.exportedAt?.trim()) {
    metaBits.push(`**Exported:** ${renderPassiveMarkdownText(meta.exportedAt)}`);
  }
  if (metaBits.length > 0) {
    lines.push(metaBits.join('  •  '));
    lines.push('');
  }

  lines.push('---');
  lines.push('');

  // --- Sections ------------------------------------------------------------
  for (const section of sections) {
    lines.push(`## ${renderPassiveMarkdownText(section.title)}`);
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
      const ts = ai.sourceTs ? `${renderSourceTimestamp(ai.sourceTs)} ` : '';
      const suffix = renderActionMeta(ai.assignee, ai.due);
      lines.push(`- ${ts}${renderPassiveMarkdownText(ai.text)}${suffix}`);
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
