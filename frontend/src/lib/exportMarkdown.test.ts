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

import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { describe, it, expect } from 'vitest';
import { renderExportMarkdown } from './exportMarkdown';
import {
  ExportApprovalError,
  ExportSourceLinkError,
  type ExportDoc,
} from './exportModel';

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
  summaryStatus: 'approved',
  sections: [
    {
      title: 'Decisions',
      items: [
        {
          id: 'b1',
          kind: 'heading1',
          text: 'Budget',
          sourceChunkId: 't10',
          sourceTs: '[00:10]',
        },
        {
          id: 'b2',
          kind: 'text',
          text: 'We approved the Q3 budget.',
          sourceChunkId: 't12',
          sourceTs: '[00:12]',
        },
        {
          id: 'b3',
          kind: 'bullet',
          text: 'Hire two engineers',
          sourceChunkId: 't65',
          sourceTs: '[01:05]',
        },
        {
          id: 'b4',
          kind: 'bullet',
          text: 'Ship export feature',
          sourceChunkId: 't90',
          sourceTs: '[01:30]',
        },
      ],
    },
  ],
  actionItems: [
    {
      id: 'a1',
      text: 'Draft the JD',
      assignee: 'Ada',
      due: 'Fri',
      sourceChunkId: 't120',
      sourceTs: '[02:00]',
    },
    {
      id: 'a2',
      text: 'Second task',
      sourceChunkId: 't130',
      sourceTs: '[02:10]',
    },
  ],
  excludedActionItemCount: 2,
};

describe('renderExportMarkdown', () => {
  const md = renderExportMarkdown(FULL_DOC);

  it('is deterministic (same in → same out)', () => {
    expect(renderExportMarkdown(FULL_DOC)).toBe(md);
  });

  it('embeds deterministic machine-readable AI provenance', () => {
    expect(md).toContain('schema: "com.bluedev.mityu.export-provenance/v1"');
    expect(md).toContain('ai_generated: true');
    expect(md).toContain('human_reviewed: true');
    expect(md).toContain('source_linked: true');
    expect(md).toContain('source_chunk_id');
    expect(md).toContain('timestamp');
    expect(md).toContain('meeting_id: "m1"');

    const sourceLinksLine = md
      .split('\n')
      .find((line) => line.startsWith('source_links: '));
    expect(sourceLinksLine).toBeDefined();
    const sourceLinks = JSON.parse(
      sourceLinksLine!.slice('source_links: '.length),
    ) as Array<{ source_chunk_id: string; timestamp: string }>;
    expect(sourceLinks[0]).toMatchObject({
      source_chunk_id: 't10',
      timestamp: '[00:10]',
    });
  });

  describe('header + provenance', () => {
    it('renders the title as the H1 header', () => {
      expect(md).toContain('\n# Weekly Sync\n');
    });

    it('names the approver and approval time in the provenance line', () => {
      expect(md).toContain(
        '> AI-generated · reviewed & approved by ada @ bluedev.dev on 2026-07-05T09:30:00Z',
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

    it('renders every approved bullet with a verified timestamp', () => {
      expect(md).toContain('- [01:30] Ship export feature');
    });
  });

  describe('action items section', () => {
    it('is present', () => {
      expect(md).toContain('## Action Items');
    });

    it('renders ts + text + assignee/due suffix', () => {
      expect(md).toContain('- [02:00] Draft the JD (Assignee: Ada, Due: Fri)');
    });

    it('renders every approved action item with a verified timestamp', () => {
      expect(md).toContain('- [02:10] Second task');
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

  it('fails closed when a hand-built document omits an AI source timestamp', () => {
    const malformed: ExportDoc = {
      ...FULL_DOC,
      sections: [
        {
          title: 'Invalid',
          items: [
            {
              id: 'broken',
              kind: 'text',
              text: 'Must not export',
              sourceChunkId: 'missing',
              sourceTs: '',
            },
          ],
        },
      ],
      actionItems: [],
    };

    expect(() => renderExportMarkdown(malformed)).toThrowError(
      ExportSourceLinkError,
    );
  });

  it('fails closed before rendering a document whose whole summary is draft', () => {
    expect(() =>
      renderExportMarkdown({ ...FULL_DOC, summaryStatus: 'draft' }),
    ).toThrowError(ExportApprovalError);
  });

  it('renders AI-controlled Markdown/HTML/URL payloads only as passive text', () => {
    const attack = [
      '<img src="https://evil.example/pixel.png" onerror="alert(1)">',
      '<script>alert(1)</script>',
      '![pixel](https://evil.example/image.png)',
      '[run](javascript:alert(1))',
      '<https://evil.example/autolink>',
      'https://evil.example/bare',
      'www.evil.example',
      'attacker@evil.example',
      '\n[00:10]: https://evil.example/reference',
    ].join(' ');
    const hostile: ExportDoc = {
      ...FULL_DOC,
      meta: {
        ...FULL_DOC.meta,
        title: attack,
        approvedBy: attack,
        model: attack,
      },
      sections: [
        {
          title: attack,
          items: [
            {
              ...FULL_DOC.sections[0].items[0],
              kind: 'text',
              text: attack,
            },
          ],
        },
      ],
      actionItems: [
        {
          ...FULL_DOC.actionItems[0],
          text: attack,
          assignee: attack,
          due: attack,
        },
      ],
    };

    const hostileMarkdown = renderExportMarkdown(hostile);
    const renderedHtml = renderToStaticMarkup(
      createElement(
        ReactMarkdown,
        { remarkPlugins: [remarkGfm] },
        hostileMarkdown,
      ),
    );

    expect(hostileMarkdown).toContain('&lt;img');
    expect(hostileMarkdown).toContain('!\\[pixel\\]');
    expect(hostileMarkdown).toContain('https ://evil.example/bare');
    expect(hostileMarkdown).toContain('www .evil.example');
    expect(hostileMarkdown).toContain('attacker @ evil.example');
    expect(hostileMarkdown).not.toMatch(/<\/?(?:img|script)\b/i);
    expect(hostileMarkdown).not.toMatch(/!\[[^\]]*\]\s*(?:\(|\[)/i);
    expect(hostileMarkdown).not.toMatch(
      /\[[^\]]*\]\s*\((?:https?|javascript|data|file):/i,
    );
    expect(hostileMarkdown).not.toMatch(/<(?:https?|mailto):/i);
    expect(hostileMarkdown).not.toMatch(/\b(?:https?|ftp):\/\//i);
    expect(hostileMarkdown).not.toMatch(/\bwww\.[a-z0-9]/i);
    expect(renderedHtml).not.toMatch(/<(?:a|img|script)\b/i);
    expect(renderedHtml).toContain('&lt;img');
    expect(renderedHtml).toContain('[00:10]');
  });
});
