/**
 * Transcript timestamp helpers (BACKLOG C4.1 — shared)
 *
 * The single source of truth for two things that both the COPY flow
 * (`useCopyOperations`) and the EXPORT flow (`useExportOperations`) must do the
 * SAME way:
 *
 *  1. `fetchAllTranscripts` — read EVERY transcript row for a meeting from the
 *     Rust core (`api_get_meeting_transcripts`), not the in-memory paginated
 *     slice the detail view happens to be holding. This matters for correctness:
 *     a summary block / action item can be grounded in a segment that is not on
 *     the currently loaded page, and we must still resolve its timestamp.
 *
 *  2. `formatTime` — render a segment's start as recording-relative `[MM:SS]`,
 *     with a wall-clock fallback for legacy rows that predate `audio_start_time`.
 *
 * These were originally inline in `useCopyOperations`; they are extracted here
 * verbatim (same behavior, same `invoke` args) so the export renderers link the
 * exact same timestamps the user already sees when they copy a transcript.
 *
 * NOTE: `source_chunk_id` on a draft block / action item is a transcripts-row
 * `id` (same id space), so a `Map<transcript.id, "[MM:SS]">` built from the
 * fetch-all result is exactly what the export model needs.
 */

import { invoke as invokeTauri } from '@tauri-apps/api/core';
import type { Transcript } from '@/types';

/** Wire shape of `api_get_meeting_transcripts` (mirrors `PaginatedTranscriptsResponse`). */
interface PaginatedTranscripts {
  transcripts: Transcript[];
  total_count: number;
  has_more: boolean;
}

/**
 * Fetch ALL transcripts for a meeting from the local core (offline; SQLite is
 * the source of truth in local-first). Two calls: one to learn `total_count`,
 * then one `limit = total_count` fetch — the exact pattern proven in
 * `useCopyOperations`. Returns `[]` on an empty meeting.
 *
 * Throws only on a real `invoke` failure; callers surface a user-friendly
 * message (never a raw Rust panic).
 */
export async function fetchAllTranscripts(meetingId: string): Promise<Transcript[]> {
  // First, get total count by fetching a single row.
  const firstPage = (await invokeTauri('api_get_meeting_transcripts', {
    meetingId,
    limit: 1,
    offset: 0,
  })) as PaginatedTranscripts;

  const totalCount = firstPage.total_count;
  if (totalCount === 0) {
    return [];
  }

  // Fetch all transcripts in one call.
  const allData = (await invokeTauri('api_get_meeting_transcripts', {
    meetingId,
    limit: totalCount,
    offset: 0,
  })) as PaginatedTranscripts;

  return allData.transcripts;
}

/**
 * Format a segment start as recording-relative `[MM:SS]`.
 *
 * @param seconds - `audio_start_time` (seconds from recording start). When
 *   `undefined` (legacy rows without recording-relative timing), the wall-clock
 *   `fallbackTimestamp` is returned unchanged instead.
 * @param fallbackTimestamp - the segment's wall-clock `timestamp` string.
 */
export function formatTime(
  seconds: number | undefined,
  fallbackTimestamp: string,
): string {
  if (seconds === undefined) {
    // For old transcripts without audio_start_time, use wall-clock time.
    return fallbackTimestamp;
  }
  const totalSecs = Math.floor(seconds);
  const mins = Math.floor(totalSecs / 60);
  const secs = totalSecs % 60;
  return `[${mins.toString().padStart(2, '0')}:${secs.toString().padStart(2, '0')}]`;
}

/**
 * Build the `Map<source_chunk_id, "[MM:SS]">` the export model consumes.
 * Keyed by transcript-row `id` (the same id space as a draft block /
 * action-item `source_chunk_id`), valued by the formatted start.
 */
export function buildTimestampMap(transcripts: Transcript[]): Map<string, string> {
  const map = new Map<string, string>();
  for (const t of transcripts) {
    map.set(t.id, formatTime(t.audio_start_time, t.timestamp));
  }
  return map;
}
