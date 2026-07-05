/**
 * Pure unit tests for `renderExportMarkdown` (BACKLOG C4.1).
 *
 * NO TEST RUNNER EXISTS in `frontend/` (no jest/vitest in package.json). These
 * are framework-free assertions: type-checked by `pnpm exec tsc --noEmit` and
 * runnable ad-hoc via any TS runner (e.g.
 * `pnpm dlx tsx src/lib/exportMarkdown.test.ts`). `runExportMarkdownTests` throws
 * on the first failed assertion.
 *
 * Coverage: deterministic output (same in → same out), the provenance header,
 * heading/text/bullet mapping, `[MM:SS]` presence, the Action Items section, and
 * the excluded-action-items footer note.
 */

import { renderExportMarkdown } from './exportMarkdown';
import type { ExportDoc } from './exportModel';

function assert(cond: boolean, msg: string): void {
  if (!cond) {
    throw new Error(`renderExportMarkdown test failed: ${msg}`);
  }
}

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

export function runExportMarkdownTests(): number {
  let passed = 0;

  const md = renderExportMarkdown(FULL_DOC);

  // 1) Determinism: identical input renders byte-identical output.
  {
    const again = renderExportMarkdown(FULL_DOC);
    assert(md === again, 'render is deterministic (same in → same out)');
    passed += 1;
  }

  // 2) Header title + provenance line (AI-generated + approver + date).
  {
    assert(md.startsWith('# Weekly Sync\n'), 'title is the H1 header');
    assert(
      md.includes('> AI-generated · reviewed & approved by ada@bluedev.dev on 2026-07-05T09:30:00Z'),
      'provenance line names approver + approval time',
    );
    passed += 1;
  }

  // 3) Metadata line carries date + model + exported.
  {
    assert(md.includes('**Meeting date:** Jul 5, 2026'), 'meeting date in meta line');
    assert(md.includes('**Model:** claude-3-5'), 'model in meta line');
    assert(md.includes('**Exported:** 2026-07-05 10:00'), 'exported time in meta line');
    passed += 1;
  }

  // 4) Section + heading/text/bullet mapping with timestamps.
  {
    assert(md.includes('## Decisions'), 'section title as H2');
    assert(md.includes('# [00:10] Budget'), 'heading1 -> "#" with leading [MM:SS]');
    assert(md.includes('[00:12] We approved the Q3 budget.'), 'text line carries its [MM:SS]');
    assert(md.includes('- [01:05] Hire two engineers'), 'bullet carries [MM:SS] after marker');
    assert(md.includes('- Ship export feature'), 'bullet without ts renders plainly');
    passed += 1;
  }

  // 5) Action Items section + per-item ts + (Assignee, Due) suffix.
  {
    assert(md.includes('## Action Items'), 'action items section present');
    assert(
      md.includes('- [02:00] Draft the JD (Assignee: Ada, Due: Fri)'),
      'action item: ts + text + assignee/due suffix',
    );
    assert(md.includes('- No-timestamp task'), 'action item without ts/meta renders plainly');
    passed += 1;
  }

  // 6) Excluded-action-items footer note (transparency).
  {
    assert(
      md.includes('_Note: 2 action items not yet approved — not included._'),
      'footer discloses the excluded action-item count',
    );
    passed += 1;
  }

  // 7) Graceful provenance fallback when approver/date are absent.
  {
    const noProvenance = renderExportMarkdown({
      ...FULL_DOC,
      meta: { meetingId: 'm', title: 'T', exportedAt: 'now' },
      excludedActionItemCount: 0,
    });
    assert(
      noProvenance.includes('> AI-generated · reviewed & approved by a human before export'),
      'provenance degrades gracefully with no approver/date',
    );
    assert(!noProvenance.includes('_Note:'), 'no footer note when nothing is excluded');
    passed += 1;
  }

  // 8) Singular wording for exactly one excluded item.
  {
    const singular = renderExportMarkdown({ ...FULL_DOC, excludedActionItemCount: 1 });
    assert(
      singular.includes('_Note: 1 action item not yet approved — not included._'),
      'singular "item" for a count of one',
    );
    passed += 1;
  }

  return passed;
}
