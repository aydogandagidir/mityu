'use client';

/**
 * Home, while idle — DESIGN_SYSTEM.md §6.1.
 *
 * It was a grid of meeting cards plus a list of open action items. The cards showed a
 * date parsed with a regular expression out of the auto-generated title, so a renamed
 * meeting lost its date and a meeting someone called "2026-01-02_notes" gained a wrong
 * one. The action items — model output no person had approved — shipped with no Art. 50
 * marking, no link to the segment they came from, and no way to act on them.
 *
 * It is now a review queue. What needs a human decision is first and reviewable in
 * place; the library is a digest below it, and the pane beside it holds the rest.
 *
 * DATA HONESTY. `api_get_meetings` carries `{id, title}` and nothing else, so no date is
 * printed and no metric tile is drawn for a number this app cannot actually produce
 * today. The regex is deleted rather than kept behind a guard: a date that is right most
 * of the time is worse than no date, because nothing tells the reader which kind they
 * are looking at.
 *
 * 🔒 `Analytics.trackPageView('home')` stays in `app/page.tsx`, and nothing on this
 * surface reports anything about the content it shows.
 */

import { useCallback, useEffect, useState } from 'react';
import { useRouter } from 'next/navigation';
import { ChevronRight, FileText, Mic } from 'lucide-react';
import { useSidebar } from '@/components/Sidebar/SidebarProvider';
import {
  summaryDraftService,
  type OpenActionItem,
} from '@/services/summaryDraftService';
import { isTauri } from '@/lib/isTauri';
import { PageHeader } from '@/components/shell/PageHeader';
import { ActionItemRow, type ActionItemRowItem } from '@/components/actions/ActionItemRow';
import { Card } from '@/components/ui/card';
import { Notice } from '@/components/ui/notice';
import { Button } from '@/components/ui/button';
import { EmptyState } from '@/components/ui/empty-state';
import { Skeleton } from '@/components/ui/skeleton';
import { focusRing } from '@/components/ui/focus-ring';
import { sampleMeetingHref } from '@/lib/tour';
import { cn } from '@/lib/utils';

/** How many drafts the queue asks for. The backend caps and orders; this only bounds. */
const QUEUE_LIMIT = 8;
/** How many meetings the digest shows before deferring to the pane. */
const DIGEST_LIMIT = 6;

export interface HomeDashboardProps {
  /**
   * Injected by `/design/home` so the fixture renders the real screen with fixture data
   * and adds no mount-time `invoke` — the `LearningSettings` pattern. Product code never
   * passes it.
   */
  service?: Pick<
    typeof summaryDraftService,
    'getOpenActionItems' | 'approveActionItem' | 'rejectActionItem' | 'editActionItem'
  >;
  /** Fixture-only: skip the Tauri guard so the queue renders in a browser. */
  forceLoad?: boolean;
}

type QueueState =
  | { phase: 'loading' }
  | { phase: 'ready'; items: OpenActionItem[] }
  | { phase: 'error' };

export function HomeDashboard({ service, forceLoad = false }: HomeDashboardProps) {
  const router = useRouter();
  const { meetings, setCurrentMeeting } = useSidebar();
  const api = service ?? summaryDraftService;

  const [queue, setQueue] = useState<QueueState>({ phase: 'loading' });
  const [reloadToken, setReloadToken] = useState(0);

  useEffect(() => {
    if (!isTauri() && !forceLoad) {
      setQueue({ phase: 'ready', items: [] });
      return;
    }
    let cancelled = false;
    setQueue({ phase: 'loading' });
    api
      .getOpenActionItems(QUEUE_LIMIT)
      .then((items) => {
        if (!cancelled) setQueue({ phase: 'ready', items });
      })
      .catch(() => {
        // Never log the item text. A failure here is visible, not silent: this is the
        // queue of things a person still has to decide on.
        if (!cancelled) setQueue({ phase: 'error' });
      });
    return () => {
      cancelled = true;
    };
  }, [api, forceLoad, meetings.length, reloadToken]);

  const openMeeting = useCallback(
    (id: string, title: string) => {
      setCurrentMeeting({ id, title });
      router.push(`/meeting-details?id=${id}`);
    },
    [router, setCurrentMeeting]
  );

  const jumpToSource = useCallback(
    (item: ActionItemRowItem) => {
      setCurrentMeeting({ id: item.meeting_id, title: item.meeting_title });
      const params = new URLSearchParams({
        id: item.meeting_id,
        segment: item.source_chunk_id,
        source: 'home-queue',
        jump: `${Date.now()}`,
      });
      router.push(`/meeting-details?${params.toString()}`);
    },
    [router, setCurrentMeeting]
  );

  const items = queue.phase === 'ready' ? queue.items : [];
  const digest = meetings.slice(0, DIGEST_LIMIT);
  const nothingRecordedYet = meetings.length === 0;

  return (
    <div className="flex min-h-0 flex-1 flex-col overflow-y-auto">
      <PageHeader
        eyebrow="Home"
        title="Needs review"
        meta={
          queue.phase === 'ready' && items.length > 0
            ? `${items.length} action ${items.length === 1 ? 'item' : 'items'} awaiting a decision`
            : undefined
        }
      />

      <div className="space-y-6 p-gutter">
        <section aria-labelledby="home-queue-heading" className="space-y-2">
          <h2 id="home-queue-heading" className="sr-only">
            Action items awaiting review
          </h2>

          {queue.phase === 'loading' && (
            <Card className="p-4">
              <Skeleton className="h-4 w-2/3" />
              <Skeleton className="mt-3 h-4 w-1/2" />
              <span className="sr-only" role="status">
                Loading action items awaiting review
              </span>
            </Card>
          )}

          {queue.phase === 'error' && (
            <Notice
              tone="destructive"
              title="Action items could not be loaded"
              action={
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => setReloadToken((n) => n + 1)}
                >
                  Retry
                </Button>
              }
            >
              Your meetings are unaffected. Nothing was approved or rejected.
            </Notice>
          )}

          {queue.phase === 'ready' && items.length > 0 && (
            <Card className="overflow-hidden p-0">
              <ul className="divide-y divide-border">
                {items.map((item) => (
                  <ActionItemRow
                    key={item.id}
                    item={item}
                    onJumpToSource={jumpToSource}
                    onApprove={(id) => api.approveActionItem(id)}
                    onReject={(id, reason) => api.rejectActionItem(id, reason)}
                    onEdit={(id, text) => api.editActionItem(id, { text })}
                  />
                ))}
              </ul>
            </Card>
          )}

          {queue.phase === 'ready' && items.length === 0 && !nothingRecordedYet && (
            <Card className="p-0">
              <EmptyState
                variant="filtered"
                icon={FileText}
                title="Nothing waiting on you"
                description="Action items appear here as soon as a summary produces them."
              />
            </Card>
          )}
        </section>

        {nothingRecordedYet ? (
          <section aria-labelledby="home-firstrun-heading" className="space-y-3">
            <h2 id="home-firstrun-heading" className="sr-only">
              Getting started
            </h2>
            <Card className="p-0">
              <EmptyState
                icon={Mic}
                title="Nothing recorded yet"
                description="Everything stays on this device — capture, transcription and the summary."
                action={
                  <Button
                    variant="record"
                    onClick={() =>
                      window.dispatchEvent(new CustomEvent('start-recording-from-sidebar'))
                    }
                  >
                    <Mic className="size-4" aria-hidden="true" />
                    Record your first meeting
                  </Button>
                }
              />
            </Card>
            {/* The tour used to navigate here automatically, so the first thing a new
                user saw was a report of a meeting they had never recorded. It is an
                offer now. */}
            <Card className="flex items-center gap-3 p-4">
              <span className="grid size-9 shrink-0 place-items-center rounded-md bg-accent text-accent-foreground">
                <FileText className="size-4" aria-hidden="true" />
              </span>
              <div className="min-w-0 flex-1">
                <p className="text-title-sm text-foreground">Try the sample report</p>
                <p className="text-caption text-subtle-foreground">
                  A short pre-recorded meeting that shows how review and source links work.
                </p>
              </div>
              <Button variant="outline" size="sm" onClick={() => router.push(sampleMeetingHref)}>
                Open sample
              </Button>
            </Card>
          </section>
        ) : (
          <section aria-labelledby="home-digest-heading" className="space-y-2">
            <div className="flex items-baseline justify-between gap-2">
              <h2 id="home-digest-heading" className="text-title text-foreground">
                Recent meetings
              </h2>
              <span className="text-caption tabular-nums text-subtle-foreground">
                {meetings.length} total
              </span>
            </div>
            <ul className="grid grid-cols-1 gap-2 sm:grid-cols-2">
              {digest.map((m) => (
                <li key={m.id}>
                  <button
                    type="button"
                    onClick={() => openMeeting(m.id, m.title)}
                    className={cn(
                      'group flex w-full items-center gap-3 rounded-md border border-border bg-card p-3 text-left',
                      'transition-colors duration-fast ease-out hover:bg-muted',
                      focusRing('background')
                    )}
                  >
                    <span className="grid size-8 shrink-0 place-items-center rounded-sm bg-accent text-accent-foreground">
                      <FileText className="size-4" aria-hidden="true" />
                    </span>
                    <span className="min-w-0 flex-1 truncate text-body text-foreground">
                      {m.title}
                    </span>
                    <ChevronRight
                      className="size-4 shrink-0 text-subtle-foreground"
                      aria-hidden="true"
                    />
                  </button>
                </li>
              ))}
            </ul>
          </section>
        )}
      </div>
    </div>
  );
}
