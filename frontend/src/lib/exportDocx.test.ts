/**
 * Pure(ish) unit tests for the DOCX + PDF renderers (BACKLOG C4.2).
 *
 * NO TEST RUNNER EXISTS in `frontend/` (no jest/vitest in package.json). Like the
 * C4.1 tests these are framework-free: type-checked by `pnpm exec tsc --noEmit`
 * and runnable ad-hoc with any TS runner that provides a DOM-ish global (the
 * renderers dynamically import `docx` / `jspdf`, which are browser-oriented), e.g.
 * `pnpm dlx tsx src/lib/exportDocx.test.ts`. Each `run…Tests` throws on the first
 * failed assertion and returns the passed-count otherwise.
 *
 * WHAT CAN BE ASSERTED: a .docx is a binary zip and a .pdf is a binary stream, so
 * their *content* is not meaningfully greppable the way Markdown is. These tests
 * therefore assert the renderer CONTRACT that the export pipeline relies on:
 *   - returns a real, NON-EMPTY `Blob` (bytes present) for a representative doc;
 *   - the `Blob.type` is the expected MIME type;
 *   - does NOT throw on the empty doc or the legacy `draft: null`-shaped doc
 *     (zero sections / zero action items) — the pipeline guards emptiness before
 *     calling these, but the renderers must still be total.
 * (The structural correctness of the mapping is covered indirectly by the shared
 * `ExportDoc` model + the Markdown renderer's exhaustive C4.1 assertions.)
 */

import { renderExportDocx } from './exportDocx';
import { renderExportPdf } from './exportPdf';
import type { ExportDoc } from './exportModel';

function assert(cond: boolean, msg: string): void {
  if (!cond) {
    throw new Error(`export renderer test failed: ${msg}`);
  }
}

/** A representative, fully-populated doc exercising every item kind + footer. */
const FULL_DOC: ExportDoc = {
  meta: {
    meetingId: 'm1',
    title: 'Weekly Sync',
    meetingDate: 'Jul 5, 2026',
    exportedAt: '2026-07-05 10:00',
    approvedAt: '2026-07-05T09:30:00Z',
    approvedBy: 'ada@bluedev.dev',
    model: 'claude-3-5',
  },
  sections: [
    {
      title: 'Decisions',
      items: [
        { kind: 'heading1', text: 'Budget', sourceTs: '[00:10]' },
        { kind: 'heading2', text: 'Q3 allocation', sourceTs: '[00:11]' },
        { kind: 'text', text: 'We approved the Q3 budget.', sourceTs: '[00:12]' },
        { kind: 'bullet', text: 'Hire two engineers', sourceTs: '[01:05]' },
        { kind: 'bullet', text: 'Ship export feature' },
      ],
    },
  ],
  actionItems: [
    { text: 'Draft the JD', assignee: 'Ada', due: 'Fri', sourceTs: '[02:00]' },
    { text: 'No-timestamp task' },
  ],
  excludedActionItemCount: 2,
};

/** The degraded doc a legacy `draft: null` / nothing-approved response yields. */
const EMPTY_DOC: ExportDoc = {
  meta: {
    meetingId: 'm2',
    title: '',
    exportedAt: '2026-07-05 10:00',
  },
  sections: [],
  actionItems: [],
  excludedActionItemCount: 0,
};

const DOCX_MIME =
  'application/vnd.openxmlformats-officedocument.wordprocessingml.document';

export async function runExportDocxTests(): Promise<number> {
  let passed = 0;

  // 1) Full doc → a non-empty .docx Blob of the right MIME type.
  {
    const blob = await renderExportDocx(FULL_DOC);
    assert(blob instanceof Blob, 'returns a Blob');
    assert(blob.size > 0, 'DOCX blob is non-empty');
    // docx sets the OOXML wordprocessing MIME type on the packed Blob.
    assert(blob.type === DOCX_MIME, `DOCX MIME is set (got "${blob.type}")`);
    passed += 1;
  }

  // 2) Empty/legacy doc → must not throw, still yields a non-empty Blob.
  {
    const blob = await renderExportDocx(EMPTY_DOC);
    assert(blob instanceof Blob, 'empty doc still returns a Blob');
    assert(blob.size > 0, 'empty-doc DOCX blob is still a valid non-empty file');
    passed += 1;
  }

  return passed;
}

export async function runExportPdfTests(): Promise<number> {
  let passed = 0;

  // 1) Full doc (multi-item, forces at least one page) → non-empty application/pdf.
  {
    const blob = await renderExportPdf(FULL_DOC);
    assert(blob instanceof Blob, 'returns a Blob');
    assert(blob.size > 0, 'PDF blob is non-empty');
    assert(blob.type === 'application/pdf', `PDF MIME is application/pdf (got "${blob.type}")`);
    passed += 1;
  }

  // 2) A doc large enough to force PAGINATION (many bullets) → still non-empty.
  {
    const many: ExportDoc = {
      ...FULL_DOC,
      sections: [
        {
          title: 'Long section',
          items: Array.from({ length: 120 }, (_, i) => ({
            kind: 'bullet' as const,
            text: `Line ${i} — a reasonably long bullet that exercises wrapping and page overflow so pagination runs`,
            sourceTs: '[00:30]',
          })),
        },
      ],
    };
    const blob = await renderExportPdf(many);
    assert(blob.size > 0, 'paginated PDF blob is non-empty');
    passed += 1;
  }

  // 3) Empty/legacy doc → must not throw, still yields a non-empty single-page PDF.
  {
    const blob = await renderExportPdf(EMPTY_DOC);
    assert(blob instanceof Blob, 'empty doc still returns a Blob');
    assert(blob.size > 0, 'empty-doc PDF blob is still a valid non-empty file');
    passed += 1;
  }

  return passed;
}

/** Run both suites (for an ad-hoc `tsx` invocation). */
export async function runExportBinaryRendererTests(): Promise<number> {
  const d = await runExportDocxTests();
  const p = await runExportPdfTests();
  return d + p;
}
