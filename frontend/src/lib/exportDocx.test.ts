/**
 * Unit tests for the DOCX + PDF renderers (BACKLOG C4.2).
 *
 * Run with `pnpm test` (Vitest, `environment: 'node'` — see `vitest.config.ts`).
 * The renderers dynamically import `docx` / `jspdf`; both produce a `Blob`, which
 * Node provides globally, so no DOM environment is required.
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

import { describe, it, expect } from 'vitest';
import { renderExportDocx } from './exportDocx';
import { renderExportPdf } from './exportPdf';
import type { ExportDoc } from './exportModel';

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

describe('renderExportDocx', () => {
  describe('a fully-populated doc', () => {
    it('returns a Blob', async () => {
      expect(await renderExportDocx(FULL_DOC)).toBeInstanceOf(Blob);
    });

    it('returns a non-empty Blob', async () => {
      expect((await renderExportDocx(FULL_DOC)).size).toBeGreaterThan(0);
    });

    it('sets the OOXML wordprocessing MIME type', async () => {
      expect((await renderExportDocx(FULL_DOC)).type).toBe(DOCX_MIME);
    });
  });

  describe('the empty/legacy doc', () => {
    it('does not throw and still returns a Blob', async () => {
      expect(await renderExportDocx(EMPTY_DOC)).toBeInstanceOf(Blob);
    });

    it('still yields a valid non-empty file', async () => {
      expect((await renderExportDocx(EMPTY_DOC)).size).toBeGreaterThan(0);
    });
  });
});

describe('renderExportPdf', () => {
  describe('a fully-populated doc', () => {
    it('returns a Blob', async () => {
      expect(await renderExportPdf(FULL_DOC)).toBeInstanceOf(Blob);
    });

    it('returns a non-empty Blob', async () => {
      expect((await renderExportPdf(FULL_DOC)).size).toBeGreaterThan(0);
    });

    it('sets the application/pdf MIME type', async () => {
      expect((await renderExportPdf(FULL_DOC)).type).toBe('application/pdf');
    });
  });

  it('still renders a non-empty Blob for a doc long enough to force pagination', async () => {
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
    expect((await renderExportPdf(many)).size).toBeGreaterThan(0);
  });

  describe('the empty/legacy doc', () => {
    it('does not throw and still returns a Blob', async () => {
      expect(await renderExportPdf(EMPTY_DOC)).toBeInstanceOf(Blob);
    });

    it('still yields a valid non-empty single-page PDF', async () => {
      expect((await renderExportPdf(EMPTY_DOC)).size).toBeGreaterThan(0);
    });
  });
});
