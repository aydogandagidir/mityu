'use client';

/**
 * Action Center — DESIGN_SYSTEM.md §6.4.
 *
 * The screen lives here rather than in `app/actions/page.tsx` because Next validates a
 * route component's props against its own `PageProps`, and both the fixture and the test
 * need to inject the loader so they exercise THIS screen instead of a copy of it.
 *
 * This screen was 72 raw palette utilities and zero `dark:` variants: a light island
 * inside a themed app, where opening it after dark switched the window to white. It is
 * now entirely on tokens, and it has its first automated test.
 *
 * 🔒 ADR-0025 — every one of these is a contract, not a style choice:
 * - approved-only, READ-ONLY. No complete, no snooze, no overdue. Approval is a review
 *   state; the moment this page grows a checkbox it claims to know about work it cannot
 *   see.
 * - the backend's order is authoritative and is never re-sorted here;
 * - pages are merged by id so a shifting offset cannot duplicate a row;
 * - `Load more` at `ACTION_CENTER_PAGE_SIZE`;
 * - the deep link `?id=&segment=&source=action-center&jump=<nonce>`;
 * - the provenance section, the loading/error/empty roles and their exact strings;
 * - ZERO analytics. This surface handles meeting content and reports nothing.
 *
 * The filter field added here is client-side and non-mutating: it narrows what is drawn
 * from what is already loaded, never issues a query, and never changes the order.
 */

import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useRouter } from 'next/navigation';
import { ListChecks, RefreshCw, Search, Shield } from 'lucide-react';

import { useSidebar } from '@/components/Sidebar/SidebarProvider';
import {
  ACTION_CENTER_PAGE_SIZE,
  listApprovedActionItems,
  type ApprovedActionItem,
  type ApprovedActionItemsPage,
} from '@/services/actionCenterService';
import { PageHeader } from '@/components/shell/PageHeader';
import { ApprovedActionCard } from '@/components/actions/ApprovedActionCard';
import { Button } from '@/components/ui/button';
import { Card } from '@/components/ui/card';
import { EmptyState } from '@/components/ui/empty-state';
import { Input } from '@/components/ui/input';
import { Notice } from '@/components/ui/notice';
import { Skeleton } from '@/components/ui/skeleton';

const LOAD_ERROR_MESSAGE = 'Approved actions could not be loaded from local storage.';

function mergeUniqueItems(
  current: ApprovedActionItem[],
  incoming: ApprovedActionItem[],
): ApprovedActionItem[] {
  const seen = new Set(current.map((item) => item.id));
  return [...current, ...incoming.filter((item) => !seen.has(item.id))];
}

export interface ActionCenterProps {
  /**
   * Injected by `/design/actions` and by the test, so both exercise this page rather
   * than a copy of it. Product code never passes it.
   */
  loadPage?: (offset: number, limit: number) => Promise<ApprovedActionItemsPage>;
}

export function ActionCenter({ loadPage: injected }: ActionCenterProps = {}) {
  const router = useRouter();
  const { setCurrentMeeting } = useSidebar();
  const requestNonceRef = useRef(0);
  const sourceJumpNonceRef = useRef(0);
  const [items, setItems] = useState<ApprovedActionItem[]>([]);
  const [nextOffset, setNextOffset] = useState<number | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isLoadingMore, setIsLoadingMore] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [filter, setFilter] = useState('');

  const fetchPage = injected ?? listApprovedActionItems;

  const loadPage = useCallback(
    async (offset: number, replace: boolean) => {
      const requestNonce = ++requestNonceRef.current;
      setError(null);
      if (replace) setIsLoading(true);
      else setIsLoadingMore(true);

      try {
        const page = await fetchPage(offset, ACTION_CENTER_PAGE_SIZE);
        if (requestNonce !== requestNonceRef.current) return;

        setItems((current) => (replace ? page.items : mergeUniqueItems(current, page.items)));
        setNextOffset(page.hasMore ? page.nextOffset : null);
      } catch {
        // The query and the item text never reach a log.
        if (requestNonce !== requestNonceRef.current) return;
        setError(LOAD_ERROR_MESSAGE);
      } finally {
        if (requestNonce === requestNonceRef.current) {
          setIsLoading(false);
          setIsLoadingMore(false);
        }
      }
    },
    [fetchPage],
  );

  useEffect(() => {
    void loadPage(0, true);
    return () => {
      requestNonceRef.current += 1;
    };
  }, [loadPage]);

  const openSource = useCallback(
    (item: ApprovedActionItem) => {
      setCurrentMeeting({ id: item.meetingId, title: item.meetingTitle });
      const jumpNonce = ++sourceJumpNonceRef.current;
      const params = new URLSearchParams({
        id: item.meetingId,
        segment: item.sourceChunkId,
        source: 'action-center',
        jump: `${Date.now()}-${jumpNonce}`,
      });
      router.push(`/meeting-details?${params.toString()}`);
    },
    [router, setCurrentMeeting],
  );

  // Client-side narrowing only: the backend order survives, nothing is re-sorted, and
  // no query leaves this process.
  const visible = useMemo(() => {
    const q = filter.trim().toLowerCase();
    if (!q) return items;
    return items.filter(
      (item) =>
        item.text.toLowerCase().includes(q) ||
        item.meetingTitle.toLowerCase().includes(q) ||
        (item.assignee ?? '').toLowerCase().includes(q),
    );
  }, [filter, items]);

  const hasItems = items.length > 0;

  return (
    <div className="flex min-h-0 flex-1 flex-col overflow-y-auto">
      <PageHeader
        eyebrow="Action Center"
        title="Approved actions"
        meta={hasItems ? `${items.length} loaded` : undefined}
        actions={
          hasItems ? (
            <div className="relative w-56">
              <Search
                className="pointer-events-none absolute left-2.5 top-1/2 size-4 -translate-y-1/2 text-subtle-foreground"
                aria-hidden="true"
              />
              <Input
                value={filter}
                onChange={(e) => setFilter(e.target.value)}
                placeholder="Filter loaded actions"
                aria-label="Filter loaded actions"
                className="pl-8"
              />
            </div>
          ) : undefined
        }
      />

      <div className="space-y-4 p-gutter">
        {/* 🔒 Frozen provenance disclosure. */}
        <section aria-label="Action provenance">
          <Notice tone="consent" icon={Shield} title="AI-extracted · human approved">
            This view is read-only. Each action links to its transcript source so you can
            verify it; approval is separate from future work-progress tracking.
          </Notice>
        </section>

        {isLoading ? (
          <Card className="space-y-3 p-4" role="status" aria-live="polite">
            <Skeleton className="h-4 w-1/3" />
            <Skeleton className="h-4 w-2/3" />
            <Skeleton className="h-4 w-1/2" />
            <span className="sr-only">Loading approved actions…</span>
          </Card>
        ) : error && !hasItems ? (
          <Notice
            tone="destructive"
            as="div"
            title="Unable to load Action Center"
            action={
              <Button variant="outline" size="sm" onClick={() => void loadPage(0, true)}>
                <RefreshCw className="size-4" aria-hidden="true" />
                Retry
              </Button>
            }
            aria-label="Unable to load Action Center"
          >
            <span role="alert">{LOAD_ERROR_MESSAGE}</span>
          </Notice>
        ) : !hasItems ? (
          <Card className="p-0">
            <div role="status">
              <EmptyState
                icon={ListChecks}
                title="No approved actions yet"
                description="Source-linked actions appear here once you approve them in a meeting summary."
                action={
                  <Button variant="outline" onClick={() => router.push('/')}>
                    Open the review queue
                  </Button>
                }
              />
            </div>
          </Card>
        ) : (
          <>
            <ul className="space-y-3" aria-label="Approved actions">
              {visible.map((item) => (
                <li key={item.id}>
                  <ApprovedActionCard item={item} onOpenSource={openSource} />
                </li>
              ))}
            </ul>

            {visible.length === 0 && (
              <Card className="p-0">
                <EmptyState
                  variant="filtered"
                  icon={Search}
                  title="No loaded action matches that filter"
                  description="The filter narrows what is already loaded; it does not search your meetings."
                  action={
                    <Button variant="outline" size="sm" onClick={() => setFilter('')}>
                      Clear filter
                    </Button>
                  }
                />
              </Card>
            )}

            {error && (
              <Notice tone="destructive" as="div" aria-label="Could not load more actions">
                <span role="alert">{LOAD_ERROR_MESSAGE}</span>
              </Notice>
            )}

            {/* Load more stays reachable after a failed page. Hiding it behind the error
                left the only way forward inside the thing that had just failed. */}
            {nextOffset !== null && (
              <div className="flex justify-center pb-4">
                <Button
                  variant="outline"
                  disabled={isLoadingMore}
                  onClick={() => void loadPage(nextOffset, false)}
                >
                  {isLoadingMore ? 'Loading…' : error ? 'Try again' : 'Load more'}
                </Button>
              </div>
            )}
          </>
        )}
      </div>
    </div>
  );
}

export default ActionCenter;
