'use client';

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
        className="mx-3 rounded-md border border-gray-100 px-3 py-4 text-center text-sm text-gray-500"
        role="status"
      >
        Type at least 2 letters or numbers to search.
      </div>
    );
  }

  if (isSearching) {
    return (
      <div
        className="mx-3 flex items-center justify-center gap-2 rounded-md border border-gray-100 px-3 py-6 text-sm text-gray-500"
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
        className="mx-3 flex gap-2 rounded-md border border-red-100 bg-red-50 px-3 py-3 text-sm text-red-700"
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
        className="mx-3 flex flex-col items-center gap-2 rounded-md border border-gray-100 px-3 py-6 text-center text-sm text-gray-500"
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
              className="group w-full rounded-md border border-gray-200 bg-white p-3 text-left transition-colors hover:border-blue-200 hover:bg-blue-50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-500"
              aria-label={`Open source in ${result.title}${displayTime ? ` at ${displayTime}` : ''}`}
            >
              <div className="flex items-start gap-2">
                <div className="mt-0.5 flex h-6 w-6 shrink-0 items-center justify-center rounded-full bg-blue-50">
                  <FileText className="h-3.5 w-3.5 text-blue-600" aria-hidden="true" />
                </div>

                <div className="min-w-0 flex-1">
                  <div className="flex items-center gap-2">
                    <span className="min-w-0 flex-1 truncate text-sm font-medium text-gray-800">
                      {result.title}
                    </span>
                    {displayTime && (
                      <span className="shrink-0 text-[11px] tabular-nums text-gray-400">
                        {displayTime}
                      </span>
                    )}
                  </div>

                  <div className="mt-1 line-clamp-3 text-xs leading-relaxed text-gray-600">
                    {result.matchContext}
                  </div>

                  <div className="mt-2 flex items-center justify-between">
                    <span className="rounded-full border border-blue-100 bg-blue-50 px-1.5 py-0.5 text-[10px] font-medium text-blue-700">
                      Transcript
                    </span>
                    <ChevronRight
                      className="h-3.5 w-3.5 text-gray-400 transition-transform group-hover:translate-x-0.5 group-hover:text-blue-600"
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
