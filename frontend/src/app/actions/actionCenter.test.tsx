// @vitest-environment jsdom

/**
 * The Action Center's first automated coverage.
 *
 * This is a 300-line screen that had none, and the four things asserted here are the
 * four that cannot be seen by reading it — they only appear when pages arrive in an
 * order or a shape the happy path does not produce:
 *
 * 1. **Backend order is the order.** ADR-0025 makes the backend authoritative for
 *    ordering; a stray sort here would look correct in every screenshot and be wrong in
 *    every dispute.
 * 2. **Pages merge by id.** Offsets shift when rows are added, so the same item can
 *    arrive twice. React would warn about the duplicate key; the user would see the
 *    action twice and believe it.
 * 3. **A stale response never wins.** Tauri invokes are not cancellable, so the guard is
 *    a request nonce. A slow first page resolving after a retry must be dropped.
 * 4. **A failed page leaves a way forward.** The control that loads more used to hide
 *    behind the error, so the only way out of a failure was inside the thing that had
 *    just failed.
 */

import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen, waitFor } from '@testing-library/react';
import { ActionCenter } from '@/components/actions/ActionCenter';
import {
  SidebarContext,
  type SidebarContextType,
} from '@/components/Sidebar/SidebarProvider';
import type {
  ApprovedActionItem,
  ApprovedActionItemsPage,
} from '@/services/actionCenterService';

vi.mock('next/navigation', () => ({
  useRouter: () => ({ push: vi.fn() }),
}));

const sidebar = {
  currentMeeting: null,
  setCurrentMeeting: () => {},
  sidebarItems: [],
  isCollapsed: true,
  toggleCollapse: () => {},
  meetings: [],
  setMeetings: () => {},
  isMeetingActive: false,
  setIsMeetingActive: () => {},
  handleRecordingToggle: () => {},
  ready: true,
  searchTranscripts: () => {},
  searchResults: [],
  isSearching: false,
  searchError: null,
  setServerAddress: () => {},
  serverAddress: '',
  transcriptServerAddress: '',
  setTranscriptServerAddress: () => {},
  activeSummaryPolls: new Map(),
  startSummaryPolling: () => {},
  stopSummaryPolling: () => {},
  refetchMeetings: async () => {},
} as SidebarContextType;

function item(id: string, text: string): ApprovedActionItem {
  return {
    id,
    meetingId: `m-${id}`,
    meetingTitle: `Meeting ${id}`,
    meetingCreatedAt: '2026-09-09T09:30:00Z',
    text,
    assignee: null,
    due: null,
    reviewStatus: 'approved',
    sourceChunkId: `chunk-${id}`,
    sourceTimestamp: '2026-09-09T09:42:11Z',
    audioStartTime: 61,
  };
}

function page(items: ApprovedActionItem[], hasMore = false): ApprovedActionItemsPage {
  return { items, hasMore, nextOffset: hasMore ? items.length : null };
}

function renderScreen(loadPage: (offset: number, limit: number) => Promise<ApprovedActionItemsPage>) {
  return render(
    <SidebarContext.Provider value={sidebar}>
      <ActionCenter loadPage={loadPage} />
    </SidebarContext.Provider>,
  );
}

/** The rendered rows, in DOM order. */
function renderedTexts() {
  const list = screen.getByRole('list', { name: 'Approved actions' });
  return Array.from(list.querySelectorAll('li')).map((li) => li.textContent ?? '');
}

afterEach(cleanup);

describe('Action Center', () => {
  it('renders rows in the order the backend returned them', async () => {
    // Deliberately NOT alphabetical and NOT chronological, so any local sort shows up.
    const loadPage = vi.fn(async () =>
      page([item('c', 'third by id'), item('a', 'first by id'), item('b', 'second by id')]),
    );
    renderScreen(loadPage);

    await screen.findByRole('list', { name: 'Approved actions' });
    const texts = renderedTexts();
    expect(texts[0]).toContain('third by id');
    expect(texts[1]).toContain('first by id');
    expect(texts[2]).toContain('second by id');
  });

  it('merges a second page by id, so a shifted offset cannot duplicate a row', async () => {
    const first = page([item('a', 'alpha'), item('b', 'bravo')], true);
    const second = page([item('b', 'bravo'), item('c', 'charlie')]);
    const loadPage = vi.fn(async (offset: number) => (offset === 0 ? first : second));

    renderScreen(loadPage);
    await screen.findByRole('list', { name: 'Approved actions' });

    screen.getByRole('button', { name: 'Load more' }).click();

    await waitFor(() => expect(renderedTexts()).toHaveLength(3));
    const texts = renderedTexts().join(' | ');
    expect(texts).toContain('alpha');
    expect(texts).toContain('charlie');
    expect(renderedTexts().filter((t) => t.includes('bravo'))).toHaveLength(1);
  });

  it('drops a stale first response that resolves after a retry', async () => {
    // Typed as a plain function rather than `| null` so TypeScript does not narrow it
    // to `never` after the assignment inside the promise executor.
    let resolveSlow: (value: ApprovedActionItemsPage) => void = () => {};
    let call = 0;
    const loadPage = vi.fn(async () => {
      call += 1;
      if (call === 1) {
        return new Promise<ApprovedActionItemsPage>((resolve) => {
          resolveSlow = resolve;
        });
      }
      return page([item('fresh', 'from the retry')]);
    });

    const { unmount } = renderScreen(loadPage);
    // A remount issues a second request; the first is now stale.
    unmount();
    renderScreen(loadPage);
    await screen.findByText('from the retry');

    resolveSlow(page([item('stale', 'from the slow first call')]));
    await waitFor(() => expect(screen.queryByText('from the slow first call')).toBeNull());
    expect(screen.getByText('from the retry')).toBeTruthy();
  });

  it('keeps a way forward when loading more fails', async () => {
    const loadPage = vi.fn(async (offset: number) => {
      if (offset === 0) return page([item('a', 'alpha')], true);
      throw new Error('local read failed');
    });

    renderScreen(loadPage);
    await screen.findByRole('list', { name: 'Approved actions' });
    screen.getByRole('button', { name: 'Load more' }).click();

    // The already-loaded rows survive, the failure is announced, and the control that
    // retries it is still on screen.
    await screen.findByRole('alert');
    expect(screen.getByText('alpha')).toBeTruthy();
    expect(screen.getByRole('button', { name: 'Try again' })).toBeTruthy();
  });

  it('never prints the worker timestamp fallback as a position in the recording', async () => {
    const withoutOffset: ApprovedActionItem = { ...item('a', 'alpha'), audioStartTime: null };
    renderScreen(async () => page([withoutOffset]));

    await screen.findByRole('list', { name: 'Approved actions' });
    expect(screen.queryByText(/2026-09-09T09:42:11Z/)).toBeNull();
    expect(
      screen.getByText(/saved before segment positions were stored/i),
    ).toBeTruthy();
  });

  it('keeps the frozen provenance disclosure', async () => {
    renderScreen(async () => page([item('a', 'alpha')]));
    await screen.findByRole('list', { name: 'Approved actions' });

    const provenance = screen.getByRole('region', { name: 'Action provenance' });
    expect(provenance.textContent).toContain('AI-extracted · human approved');
    expect(provenance.textContent).toContain(
      'Each action links to its transcript source so you can verify it; approval is separate from future work-progress tracking.',
    );
  });
});
