/**
 * Unit tests for `buildExportDoc` (BACKLOG C4.1).
 *
 * Run with `pnpm test` (Vitest, `environment: 'node'` — see `vitest.config.ts`).
 * These are pure-logic assertions: no DOM, no mocks, no I/O.
 *
 * Coverage: approved-only block filtering, approved-only action-item filtering +
 * excluded-count, timestamp mapping (resolved + unresolved), empty-section drop,
 * and the legacy `draft: null` degraded doc.
 */

import { describe, it, expect } from 'vitest';
import { buildExportDoc, type ExportMeta } from './exportModel';
import type {
  SummaryDraftResponse,
  DraftBlock,
  ActionItemDraft,
} from '@/services/summaryDraftService';

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

describe('buildExportDoc', () => {
  describe('approved-only block filtering', () => {
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
    const doc = buildExportDoc(resp, new Map<string, string>([['t1', '[01:05]']]), META);

    it('drops a section whose blocks are all non-approved', () => {
      expect(doc.sections).toHaveLength(1);
    });

    it('emits only the approved block', () => {
      expect(doc.sections[0].items).toHaveLength(1);
    });

    it('the surviving block is the approved one', () => {
      expect(doc.sections[0].items[0].text).toBe('kept');
    });

    it('attaches the resolved timestamp to the surviving block', () => {
      expect(doc.sections[0].items[0].sourceTs).toBe('[01:05]');
    });
  });

  it('leaves sourceTs undefined for an unresolved source_chunk_id (never fabricated)', () => {
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
    expect(doc.sections[0].items[0].sourceTs).toBeUndefined();
  });

  describe('action items', () => {
    const resp = response({
      action_items: [
        actionItem({ id: 'a1', text: 'ship', status: 'approved', assignee: 'Ada', due: 'Fri', source_chunk_id: 't1' }),
        actionItem({ id: 'a2', text: 'draft', status: 'draft', source_chunk_id: 't2' }),
        actionItem({ id: 'a3', text: 'edited', status: 'edited', source_chunk_id: 't3' }),
        actionItem({ id: 'a4', text: 'rejected', status: 'rejected', source_chunk_id: 't4' }),
      ],
    });
    const doc = buildExportDoc(resp, new Map<string, string>([['t1', '[00:42]']]), META);

    it('emits only the approved action item', () => {
      expect(doc.actionItems).toHaveLength(1);
    });

    it('carries the approved item text', () => {
      expect(doc.actionItems[0].text).toBe('ship');
    });

    it('carries the assignee', () => {
      expect(doc.actionItems[0].assignee).toBe('Ada');
    });

    it('carries the due date', () => {
      expect(doc.actionItems[0].due).toBe('Fri');
    });

    it('resolves the action-item timestamp', () => {
      expect(doc.actionItems[0].sourceTs).toBe('[00:42]');
    });

    it('counts the three non-approved items as excluded', () => {
      expect(doc.excludedActionItemCount).toBe(3);
    });
  });

  describe('legacy `draft: null` degraded doc', () => {
    const resp = response({
      draft: null,
      action_items: [actionItem({ id: 'a1', text: 'still here', status: 'approved', source_chunk_id: 't1' })],
    });
    const doc = buildExportDoc(resp, new Map([['t1', '[00:01]']]), META);

    it('yields zero sections', () => {
      expect(doc.sections).toHaveLength(0);
    });

    it('still flows action items (they live in their own table)', () => {
      expect(doc.actionItems).toHaveLength(1);
    });

    it('reports no excluded items', () => {
      expect(doc.excludedActionItemCount).toBe(0);
    });
  });

  describe('fully null response (defensive)', () => {
    const doc = buildExportDoc(null, new Map(), META);

    it('has no sections', () => {
      expect(doc.sections).toHaveLength(0);
    });

    it('has no action items', () => {
      expect(doc.actionItems).toHaveLength(0);
    });

    it('reports zero excluded items', () => {
      expect(doc.excludedActionItemCount).toBe(0);
    });

    it('passes meta through verbatim', () => {
      expect(doc.meta.title).toBe('Weekly Sync');
    });
  });
});
