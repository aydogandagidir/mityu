/**
 * Unit tests for `renderExportMarkdown` (BACKLOG C4.1).
 *
 * Run with `pnpm test` (Vitest, `environment: 'node'` — see `vitest.config.ts`).
 * These are pure-logic assertions: no DOM, no mocks, no I/O.
 *
 * Coverage: deterministic output (same in → same out), the provenance header,
 * heading/text/bullet mapping, `[MM:SS]` presence, the Action Items section, and
 * the excluded-action-items footer note.
 */

import { describe, it, expect } from 'vitest';
import { renderExportMarkdown } from './exportMarkdown';
import type { ExportDoc } from './exportModel';

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

describe('renderExportMarkdown', () => {
  const md = renderExportMarkdown(FULL_DOC);

  it('is deterministic (same in → same out)', () => {
    expect(renderExportMarkdown(FULL_DOC)).toBe(md);
  });

  describe('header + provenance', () => {
    it('renders the title as the H1 header', () => {
      expect(md.startsWith('# Weekly Sync\n')).toBe(true);
    });

    it('names the approver and approval time in the provenance line', () => {
      expect(md).toContain(
        '> AI-generated · reviewed & approved by ada@bluedev.dev on 2026-07-05T09:30:00Z',
      );
    });
  });

  describe('metadata line', () => {
    it('carries the meeting date', () => {
      expect(md).toContain('**Meeting date:** Jul 5, 2026');
    });

    it('carries the model', () => {
      expect(md).toContain('**Model:** claude-3-5');
    });

    it('carries the exported time', () => {
      expect(md).toContain('**Exported:** 2026-07-05 10:00');
    });
  });

  describe('section + item mapping', () => {
    it('renders the section title as H2', () => {
      expect(md).toContain('## Decisions');
    });

    it('maps heading1 to "#" with a leading [MM:SS]', () => {
      expect(md).toContain('# [00:10] Budget');
    });

    it('gives a text line its [MM:SS]', () => {
      expect(md).toContain('[00:12] We approved the Q3 budget.');
    });

    it('puts a bullet\'s [MM:SS] after the marker', () => {
      expect(md).toContain('- [01:05] Hire two engineers');
    });

    it('renders a bullet without a timestamp plainly', () => {
      expect(md).toContain('- Ship export feature');
    });
  });

  describe('action items section', () => {
    it('is present', () => {
      expect(md).toContain('## Action Items');
    });

    it('renders ts + text + assignee/due suffix', () => {
      expect(md).toContain('- [02:00] Draft the JD (Assignee: Ada, Due: Fri)');
    });

    it('renders an item without ts/meta plainly', () => {
      expect(md).toContain('- No-timestamp task');
    });
  });

  describe('excluded-action-items footer (transparency)', () => {
    it('discloses the excluded action-item count', () => {
      expect(md).toContain('_Note: 2 action items not yet approved — not included._');
    });

    it('uses the singular "item" for a count of one', () => {
      const singular = renderExportMarkdown({ ...FULL_DOC, excludedActionItemCount: 1 });
      expect(singular).toContain('_Note: 1 action item not yet approved — not included._');
    });
  });

  describe('graceful degradation when approver/date are absent', () => {
    const noProvenance = renderExportMarkdown({
      ...FULL_DOC,
      meta: { meetingId: 'm', title: 'T', exportedAt: 'now' },
      excludedActionItemCount: 0,
    });

    it('falls back to a generic human-approval provenance line', () => {
      expect(noProvenance).toContain('> AI-generated · reviewed & approved by a human before export');
    });

    it('omits the footer note when nothing is excluded', () => {
      expect(noProvenance).not.toContain('_Note:');
    });
  });
});
