import { describe, expect, it, vi } from 'vitest';
import {
  ExportApprovalError,
  ExportSourceLinkError,
  type ExportMeta,
} from '@/lib/exportModel';
import type { SummaryDraftResponse } from '@/services/summaryDraftService';
import {
  prepareExportDoc,
  resolveExportTimestamps,
} from './useExportOperations';

const META: ExportMeta = {
  meetingId: 'meeting-1',
  title: 'Meeting',
  exportedAt: '2026-07-15T10:00:00Z',
};

function reviewResponse(
  topStatus: SummaryDraftResponse['status'],
  draftStatus: NonNullable<SummaryDraftResponse['draft']>['status'],
): SummaryDraftResponse {
  return {
    status: topStatus,
    draft: {
      meeting_id: 'meeting-1',
      status: draftStatus,
      sections: [
        {
          title: 'AI output',
          blocks: [
            {
              id: 'approved-child',
              type: 'text',
              content: '<img src="https://evil.example/pixel">',
              source_chunk_id: 'source-1',
              status: 'approved',
            },
          ],
        },
      ],
    },
    model: null,
    template_id: null,
    generated_at: null,
    approved_at: null,
    approved_by: null,
    action_items: [],
  };
}

describe('resolveExportTimestamps', () => {
  it('fails closed when the transcript read fails', async () => {
    await expect(
      resolveExportTimestamps('meeting-1', async () => {
        throw new Error('database unavailable');
      }),
    ).rejects.toBeInstanceOf(ExportSourceLinkError);
  });

  it('returns an empty map only for a successfully read empty transcript', async () => {
    const timestamps = await resolveExportTimestamps('meeting-1', async () => []);
    expect(timestamps).toEqual(new Map());
  });
});

describe('prepareExportDoc approval boundary', () => {
  it('rejects approved child content while the whole summary is draft, before transcript I/O', async () => {
    const fetcher = vi.fn(async () => []);

    await expect(
      prepareExportDoc(
        'meeting-1',
        reviewResponse('draft', 'draft'),
        META,
        fetcher,
      ),
    ).rejects.toBeInstanceOf(ExportApprovalError);
    expect(fetcher).not.toHaveBeenCalled();
  });

  it('rejects a malformed response whose top-level and hydrated statuses disagree', async () => {
    const fetcher = vi.fn(async () => []);

    await expect(
      prepareExportDoc(
        'meeting-1',
        reviewResponse('approved', 'draft'),
        META,
        fetcher,
      ),
    ).rejects.toBeInstanceOf(ExportApprovalError);
    expect(fetcher).not.toHaveBeenCalled();
  });
});
