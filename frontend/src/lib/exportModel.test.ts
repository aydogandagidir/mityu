/**
 * Pure unit tests for `buildExportDoc` (BACKLOG C4.1).
 *
 * NO TEST RUNNER EXISTS in `frontend/` (no jest/vitest in package.json). These
 * are written as pure, framework-free assertions so they are (a) fully
 * type-checked by `pnpm exec tsc --noEmit` and (b) runnable ad-hoc with any TS
 * runner (e.g. `pnpm dlx tsx src/lib/exportModel.test.ts`). `runExportModelTests`
 * throws on the first failed assertion and returns the passed-count otherwise.
 *
 * Coverage: approved-only block filtering, approved-only action-item filtering +
 * excluded-count, timestamp mapping (resolved + unresolved), empty-section drop,
 * and the legacy `draft: null` degraded doc.
 */

import { buildExportDoc, type ExportMeta } from './exportModel';
import type {
  SummaryDraftResponse,
  DraftBlock,
  ActionItemDraft,
} from '@/services/summaryDraftService';

function assert(cond: boolean, msg: string): void {
  if (!cond) {
    throw new Error(`buildExportDoc test failed: ${msg}`);
  }
}

function assertEq<T>(actual: T, expected: T, msg: string): void {
  const a = JSON.stringify(actual);
  const e = JSON.stringify(expected);
  if (a !== e) {
    throw new Error(`buildExportDoc test failed: ${msg}\n  expected ${e}\n  actual   ${a}`);
  }
}

const META: ExportMeta = {
  meetingId: 'm1',
  title: 'Weekly Sync',
  exportedAt: '2026-07-05 10:00',
};

function block(overrides: Partial<DraftBlock>): DraftBlock {
  return {
    id: overrides.id ?? 'b',
    type: overrides.type ?? 'text',
    content: overrides.content ?? 'text',
    source_chunk_id: overrides.source_chunk_id ?? 'c',
    status: overrides.status ?? 'approved',
    original_content: overrides.original_content,
  };
}

function actionItem(overrides: Partial<ActionItemDraft>): ActionItemDraft {
  return {
    id: overrides.id ?? 'a',
    text: overrides.text ?? 'do it',
    assignee: overrides.assignee,
    due: overrides.due,
    status: overrides.status ?? 'approved',
    source_chunk_id: overrides.source_chunk_id ?? 'c',
  };
}

function response(overrides: Partial<SummaryDraftResponse>): SummaryDraftResponse {
  return {
    draft: overrides.draft ?? null,
    status: overrides.status ?? 'approved',
    model: overrides.model ?? null,
    template_id: overrides.template_id ?? null,
    generated_at: overrides.generated_at ?? null,
    approved_at: overrides.approved_at ?? null,
    approved_by: overrides.approved_by ?? null,
    action_items: overrides.action_items ?? [],
  };
}

export function runExportModelTests(): number {
  let passed = 0;

  // 1) Approved-only block filtering + empty-section drop.
  {
    const resp = response({
      draft: {
        meeting_id: 'm1',
        status: 'approved',
        sections: [
          {
            title: 'Decisions',
            blocks: [
              block({ id: 'b1', content: 'kept', status: 'approved', source_chunk_id: 't1' }),
              block({ id: 'b2', content: 'dropped-draft', status: 'draft', source_chunk_id: 't2' }),
              block({ id: 'b3', content: 'dropped-rejected', status: 'rejected', source_chunk_id: 't3' }),
              block({ id: 'b4', content: 'dropped-edited', status: 'edited', source_chunk_id: 't4' }),
            ],
          },
          {
            // Section with zero approved blocks must be dropped entirely.
            title: 'Empty',
            blocks: [block({ id: 'b5', status: 'draft', source_chunk_id: 't5' })],
          },
        ],
      },
    });
    const ts = new Map<string, string>([['t1', '[01:05]']]);
    const doc = buildExportDoc(resp, ts, META);
    assertEq(doc.sections.length, 1, 'only one section survives (empty dropped)');
    assertEq(doc.sections[0].items.length, 1, 'only the approved block is emitted');
    assertEq(doc.sections[0].items[0].text, 'kept', 'the surviving block is the approved one');
    assertEq(doc.sections[0].items[0].sourceTs, '[01:05]', 'resolved timestamp attached');
    passed += 1;
  }

  // 2) Unresolved source_chunk_id => sourceTs undefined (never fabricated).
  {
    const resp = response({
      draft: {
        meeting_id: 'm1',
        status: 'approved',
        sections: [
          {
            title: 'S',
            blocks: [block({ id: 'b1', content: 'x', status: 'approved', source_chunk_id: 'gone' })],
          },
        ],
      },
    });
    const doc = buildExportDoc(resp, new Map(), META);
    assert(doc.sections[0].items[0].sourceTs === undefined, 'unresolved id yields undefined ts');
    passed += 1;
  }

  // 3) Action items: approved-only + excluded count + assignee/due + ts mapping.
  {
    const resp = response({
      action_items: [
        actionItem({ id: 'a1', text: 'ship', status: 'approved', assignee: 'Ada', due: 'Fri', source_chunk_id: 't1' }),
        actionItem({ id: 'a2', text: 'draft', status: 'draft', source_chunk_id: 't2' }),
        actionItem({ id: 'a3', text: 'edited', status: 'edited', source_chunk_id: 't3' }),
        actionItem({ id: 'a4', text: 'rejected', status: 'rejected', source_chunk_id: 't4' }),
      ],
    });
    const ts = new Map<string, string>([['t1', '[00:42]']]);
    const doc = buildExportDoc(resp, ts, META);
    assertEq(doc.actionItems.length, 1, 'only the approved action item is emitted');
    assertEq(doc.actionItems[0].text, 'ship', 'approved item text');
    assertEq(doc.actionItems[0].assignee, 'Ada', 'assignee carried');
    assertEq(doc.actionItems[0].due, 'Fri', 'due carried');
    assertEq(doc.actionItems[0].sourceTs, '[00:42]', 'action item ts resolved');
    assertEq(doc.excludedActionItemCount, 3, 'three non-approved items counted as excluded');
    passed += 1;
  }

  // 4) Legacy draft:null => degraded doc: no sections; approved items still flow.
  {
    const resp = response({
      draft: null,
      action_items: [actionItem({ id: 'a1', text: 'still here', status: 'approved', source_chunk_id: 't1' })],
    });
    const doc = buildExportDoc(resp, new Map([['t1', '[00:01]']]), META);
    assertEq(doc.sections.length, 0, 'null draft yields zero sections');
    assertEq(doc.actionItems.length, 1, 'action items (own table) still flow with null draft');
    assertEq(doc.excludedActionItemCount, 0, 'no excluded items here');
    passed += 1;
  }

  // 5) Fully null response (defensive) => empty doc.
  {
    const doc = buildExportDoc(null, new Map(), META);
    assertEq(doc.sections.length, 0, 'null response: no sections');
    assertEq(doc.actionItems.length, 0, 'null response: no action items');
    assertEq(doc.excludedActionItemCount, 0, 'null response: zero excluded');
    assertEq(doc.meta.title, 'Weekly Sync', 'meta is passed through verbatim');
    passed += 1;
  }

  return passed;
}
