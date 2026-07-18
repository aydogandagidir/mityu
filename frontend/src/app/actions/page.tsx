'use client';

import { useCallback, useEffect, useRef, useState } from 'react';
import { useRouter } from 'next/navigation';
import {
  AlertCircle,
  CalendarDays,
  ExternalLink,
  FileText,
  ListChecks,
  Loader2,
  RefreshCw,
  Shield,
  UserRound,
} from 'lucide-react';

import { useSidebar } from '@/components/Sidebar/SidebarProvider';
import {
  ACTION_CENTER_PAGE_SIZE,
  listApprovedActionItems,
  type ApprovedActionItem,
} from '@/services/actionCenterService';

const LOAD_ERROR_MESSAGE = 'Approved actions could not be loaded from local storage.';

function formatRecordingTime(seconds: number | null): string | null {
  if (seconds === null || !Number.isFinite(seconds) || seconds < 0) {
    return null;
  }

  const totalSeconds = Math.floor(seconds);
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const remainingSeconds = totalSeconds % 60;

  if (hours > 0) {
    return [hours, minutes, remainingSeconds]
      .map((part) => part.toString().padStart(2, '0'))
      .join(':');
  }

  return `${minutes.toString().padStart(2, '0')}:${remainingSeconds
    .toString()
    .padStart(2, '0')}`;
}

function formatMeetingDate(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return value;
  }

  return new Intl.DateTimeFormat(undefined, { dateStyle: 'medium' }).format(date);
}

function mergeUniqueItems(
  current: ApprovedActionItem[],
  incoming: ApprovedActionItem[],
): ApprovedActionItem[] {
  const seen = new Set(current.map((item) => item.id));
  return [...current, ...incoming.filter((item) => !seen.has(item.id))];
}

export default function ActionsPage() {
  const router = useRouter();
  const { setCurrentMeeting } = useSidebar();
  const requestNonceRef = useRef(0);
  const sourceJumpNonceRef = useRef(0);
  const [items, setItems] = useState<ApprovedActionItem[]>([]);
  const [nextOffset, setNextOffset] = useState<number | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isLoadingMore, setIsLoadingMore] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const loadPage = useCallback(async (offset: number, replace: boolean) => {
    const requestNonce = ++requestNonceRef.current;
    setError(null);
    if (replace) {
      setIsLoading(true);
    } else {
      setIsLoadingMore(true);
    }

    try {
      const page = await listApprovedActionItems(offset, ACTION_CENTER_PAGE_SIZE);
      if (requestNonce !== requestNonceRef.current) return;

      setItems((current) => (replace ? page.items : mergeUniqueItems(current, page.items)));
      setNextOffset(page.hasMore ? page.nextOffset : null);
    } catch {
      if (requestNonce !== requestNonceRef.current) return;
      setError(LOAD_ERROR_MESSAGE);
    } finally {
      if (requestNonce === requestNonceRef.current) {
        setIsLoading(false);
        setIsLoadingMore(false);
      }
    }
  }, []);

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

  return (
    <div className="flex h-screen flex-col bg-gray-50">
      <header className="sticky top-0 z-10 border-b border-gray-200 bg-gray-50/95 backdrop-blur">
        <div className="mx-auto w-full max-w-6xl px-6 py-6 pr-8">
          <div className="flex items-start gap-3">
            <div className="mt-1 flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-blue-100">
              <ListChecks className="h-5 w-5 text-blue-700" aria-hidden="true" />
            </div>
            <div>
              <h1 className="text-3xl font-bold tracking-tight text-gray-950">Action Center</h1>
              <p className="mt-1 text-sm text-gray-600">
                Human-approved, source-linked actions across your meetings.
              </p>
            </div>
          </div>
        </div>
      </header>

      <div className="flex-1 overflow-y-auto">
        <div className="mx-auto w-full max-w-6xl space-y-5 px-6 py-6 pr-8">
          <section
            className="flex gap-3 rounded-xl border border-blue-200 bg-blue-50 px-4 py-3 text-sm text-blue-950"
            aria-label="Action provenance"
          >
            <Shield className="mt-0.5 h-4 w-4 shrink-0 text-blue-700" aria-hidden="true" />
            <div>
              <p className="font-semibold">AI-extracted · human approved</p>
              <p className="mt-0.5 text-blue-800">
                This view is read-only. Each action links to its transcript source so you can
                verify it; approval is separate from future work-progress tracking.
              </p>
            </div>
          </section>

          {isLoading ? (
            <div
              className="flex min-h-64 items-center justify-center gap-3 rounded-xl border border-gray-200 bg-white text-sm text-gray-600"
              role="status"
              aria-live="polite"
            >
              <Loader2 className="h-5 w-5 animate-spin text-blue-600" aria-hidden="true" />
              Loading approved actions…
            </div>
          ) : error && items.length === 0 ? (
            <div
              className="flex min-h-64 flex-col items-center justify-center gap-4 rounded-xl border border-red-200 bg-white px-6 text-center"
              role="alert"
            >
              <AlertCircle className="h-7 w-7 text-red-600" aria-hidden="true" />
              <div>
                <h2 className="font-semibold text-gray-900">Unable to load Action Center</h2>
                <p className="mt-1 text-sm text-gray-600">{error}</p>
              </div>
              <button
                type="button"
                onClick={() => void loadPage(0, true)}
                className="inline-flex items-center gap-2 rounded-lg bg-gray-900 px-4 py-2 text-sm font-medium text-white transition-colors hover:bg-gray-700 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-500 focus-visible:ring-offset-2"
              >
                <RefreshCw className="h-4 w-4" aria-hidden="true" />
                Retry
              </button>
            </div>
          ) : items.length === 0 ? (
            <div
              className="flex min-h-64 flex-col items-center justify-center rounded-xl border border-dashed border-gray-300 bg-white px-6 text-center"
              role="status"
            >
              <div className="flex h-12 w-12 items-center justify-center rounded-full bg-gray-100">
                <ListChecks className="h-6 w-6 text-gray-500" aria-hidden="true" />
              </div>
              <h2 className="mt-4 text-lg font-semibold text-gray-900">No approved actions yet</h2>
              <p className="mt-1 max-w-lg text-sm leading-6 text-gray-600">
                Source-linked actions will appear here after you explicitly approve them in a
                meeting summary.
              </p>
            </div>
          ) : (
            <>
              <ul className="space-y-3" aria-label="Approved actions">
                {items.map((item) => {
                  const recordingTime = formatRecordingTime(item.audioStartTime);
                  const sourceLabel = recordingTime || item.sourceTimestamp;

                  return (
                    <li key={item.id}>
                      <article className="rounded-xl border border-gray-200 bg-white p-5 shadow-sm">
                        <div className="flex flex-col gap-4 sm:flex-row sm:items-start sm:justify-between">
                          <div className="min-w-0 flex-1">
                            <div className="flex flex-wrap items-center gap-2 text-xs text-gray-500">
                              <span className="min-w-0 max-w-full break-words font-semibold text-gray-800">
                                {item.meetingTitle}
                              </span>
                              <span aria-hidden="true">·</span>
                              <span>{formatMeetingDate(item.meetingCreatedAt)}</span>
                              <span className="rounded-full border border-blue-200 bg-blue-50 px-2 py-0.5 font-medium text-blue-700">
                                Approved
                              </span>
                            </div>

                            <h2 className="mt-3 whitespace-pre-wrap break-words text-base font-semibold leading-6 text-gray-950">
                              {item.text}
                            </h2>

                            {(item.assignee || item.due) && (
                              <dl className="mt-4 flex flex-wrap gap-2 text-xs">
                                {item.assignee && (
                                  <div className="inline-flex min-w-0 max-w-full items-center gap-1.5 rounded-lg border border-gray-200 bg-gray-50 px-2.5 py-1.5">
                                    <UserRound className="h-3.5 w-3.5 text-gray-500" aria-hidden="true" />
                                    <dt className="shrink-0 font-medium text-gray-500">Assignee:</dt>
                                    <dd className="min-w-0 break-words text-gray-800">
                                      {item.assignee}
                                    </dd>
                                  </div>
                                )}
                                {item.due && (
                                  <div className="inline-flex min-w-0 max-w-full items-center gap-1.5 rounded-lg border border-gray-200 bg-gray-50 px-2.5 py-1.5">
                                    <CalendarDays className="h-3.5 w-3.5 text-gray-500" aria-hidden="true" />
                                    <dt className="shrink-0 font-medium text-gray-500">Due:</dt>
                                    <dd className="min-w-0 break-words text-gray-800">{item.due}</dd>
                                  </div>
                                )}
                              </dl>
                            )}
                          </div>

                          <button
                            type="button"
                            onClick={() => openSource(item)}
                            className="inline-flex shrink-0 items-center justify-center gap-2 rounded-lg border border-gray-300 bg-white px-3 py-2 text-sm font-medium text-gray-800 transition-colors hover:border-blue-300 hover:bg-blue-50 hover:text-blue-800 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-500 focus-visible:ring-offset-2"
                            aria-label={`Open source in ${item.meetingTitle} at ${sourceLabel}`}
                          >
                            <FileText className="h-4 w-4" aria-hidden="true" />
                            Source
                            <ExternalLink className="h-3.5 w-3.5" aria-hidden="true" />
                          </button>
                        </div>

                        <div className="mt-4 border-t border-gray-100 pt-3 text-xs text-gray-500">
                          Transcript source · {sourceLabel}
                        </div>
                      </article>
                    </li>
                  );
                })}
              </ul>

              {error && (
                <div
                  className="flex flex-col gap-3 rounded-lg border border-red-200 bg-red-50 px-4 py-3 text-sm text-red-800 sm:flex-row sm:items-center sm:justify-between"
                  role="alert"
                >
                  <span>{error}</span>
                  <button
                    type="button"
                    onClick={() => nextOffset !== null && void loadPage(nextOffset, false)}
                    className="inline-flex items-center gap-2 self-start rounded-md border border-red-300 bg-white px-3 py-1.5 font-medium text-red-800 hover:bg-red-100 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-red-500"
                  >
                    <RefreshCw className="h-3.5 w-3.5" aria-hidden="true" />
                    Retry
                  </button>
                </div>
              )}

              {nextOffset !== null && !error && (
                <div className="flex justify-center pb-6">
                  <button
                    type="button"
                    disabled={isLoadingMore}
                    onClick={() => void loadPage(nextOffset, false)}
                    className="inline-flex items-center gap-2 rounded-lg border border-gray-300 bg-white px-4 py-2 text-sm font-medium text-gray-800 transition-colors hover:border-blue-300 hover:bg-blue-50 disabled:cursor-not-allowed disabled:opacity-60 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-500 focus-visible:ring-offset-2"
                  >
                    {isLoadingMore && (
                      <Loader2 className="h-4 w-4 animate-spin" aria-hidden="true" />
                    )}
                    {isLoadingMore ? 'Loading…' : 'Load more'}
                  </button>
                </div>
              )}
            </>
          )}
        </div>
      </div>
    </div>
  );
}
