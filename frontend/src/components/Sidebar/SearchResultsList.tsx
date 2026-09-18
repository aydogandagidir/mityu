'use client';

/**
 * Search results for the meetings pane.
 *
 * This file was in the guardrail-1 quarantine and still carried pre-redesign raw palette
 * (`bg-white`, `text-gray-*`, `border-blue-*`). The fixtures never caught it because the
 * result CARD only renders when a query actually returns a row — `/design/shell` shows the
 * idle "All meetings" state, so the shot set never painted one. In the running app, in dark
 * mode, that made a pure-white card inside the dark pane. Now token-only (§4.2), so it
 * follows the theme, and the file is out of both quarantine lists.
 */

import { AlertCircle, ChevronRight, FileText, Loader2, SearchX } from 'lucide-react';
import type { TranscriptSearchResult } from '@/services/search';

interface SearchResultsListProps {
  results: TranscriptSearchResult[];
  isSearching: boolean;
  isQueryTooShort: boolean;
  error: string | null;
  onSelect: (result: TranscriptSearchResult) => void;
}

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

export function SearchResultsList({
  results,
  isSearching,
  isQueryTooShort,
  error,
  onSelect,
}: SearchResultsListProps) {
  if (isQueryTooShort) {
    return (
      <div
        className="mx-3 rounded-md border border-border px-3 py-4 text-center text-body text-muted-foreground"
        role="status"
      >
        Type at least 2 letters or numbers to search.
      </div>
    );
  }

  if (isSearching) {
    return (
      <div
        className="mx-3 flex items-center justify-center gap-2 rounded-md border border-border px-3 py-6 text-body text-muted-foreground"
        role="status"
        aria-live="polite"
      >
        <Loader2 className="h-4 w-4 animate-spin" aria-hidden="true" />
        Searching local meeting evidence...
      </div>
    );
  }

  if (error) {
    return (
      <div
        className="mx-3 flex gap-2 rounded-md border border-destructive/30 bg-destructive/10 px-3 py-3 text-body text-destructive"
        role="alert"
      >
        <AlertCircle className="mt-0.5 h-4 w-4 shrink-0" aria-hidden="true" />
        <span>{error}</span>
      </div>
    );
  }

  if (results.length === 0) {
    return (
      <div
        className="mx-3 flex flex-col items-center gap-2 rounded-md border border-border px-3 py-6 text-center text-body text-muted-foreground"
        role="status"
      >
        <SearchX className="h-5 w-5" aria-hidden="true" />
        No matching meeting evidence found.
      </div>
    );
  }

  return (
    <div className="mx-3 space-y-2 pb-3" role="list" aria-label="Meeting search results">
      {results.map((result) => {
        const displayTime = formatRecordingTime(result.audioStartTime) || result.timestamp || null;

        return (
          <div key={`${result.id}-${result.sourceChunkId}`} role="listitem">
            <button
              type="button"
              onClick={() => onSelect(result)}
              className="group w-full rounded-md border border-border bg-card p-3 text-left transition-colors hover:border-primary-ink hover:bg-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-card"
              aria-label={`Open source in ${result.title}${displayTime ? ` at ${displayTime}` : ''}`}
            >
              <div className="flex items-start gap-2">
                <div className="mt-0.5 flex h-6 w-6 shrink-0 items-center justify-center rounded-full bg-accent">
                  <FileText className="h-3.5 w-3.5 text-primary" aria-hidden="true" />
                </div>

                <div className="min-w-0 flex-1">
                  <div className="flex items-center gap-2">
                    <span className="min-w-0 flex-1 truncate text-body font-medium text-foreground">
                      {result.title}
                    </span>
                    {displayTime && (
                      <span className="shrink-0 text-micro tabular-nums text-muted-foreground">
                        {displayTime}
                      </span>
                    )}
                  </div>

                  <div className="mt-1 line-clamp-3 text-caption leading-relaxed text-muted-foreground">
                    {result.matchContext}
                  </div>

                  <div className="mt-2 flex items-center justify-between">
                    <span className="rounded-full border border-input bg-card px-1.5 py-0.5 text-micro font-medium text-primary-ink">
                      Transcript
                    </span>
                    <ChevronRight
                      className="h-3.5 w-3.5 text-muted-foreground transition-transform group-hover:translate-x-0.5 group-hover:text-primary"
                      aria-hidden="true"
                    />
                  </div>
                </div>
              </div>
            </button>
          </div>
        );
      })}
    </div>
  );
}
