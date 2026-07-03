/**
 * Search Service
 *
 * Handles meeting search Tauri backend calls.
 * Pure 1-to-1 wrapper over invoke() - no behavior changes vs. a direct invoke call.
 * This is the single typed definition of the search result shape (incl. matchedIn).
 */

import { invoke } from '@tauri-apps/api/core';

/**
 * A single meeting search hit returned by `api_search_transcripts`.
 *
 * `timestamp` is `""` (empty string) for summary-only hits, so callers must
 * guard against an empty value before formatting/rendering it as a time.
 */
export interface TranscriptSearchResult {
  /** Meeting id the hit belongs to. */
  id: string;
  /** Meeting title. */
  title: string;
  /** Snippet of the matched text, for preview. */
  matchContext: string;
  /** Segment timestamp; `""` when the hit came from a summary only. */
  timestamp: string;
  /** Where the hit was found. */
  matchedIn: 'transcript' | 'summary';
}

/**
 * Search Service
 * Singleton service for meeting content search.
 */
export class SearchService {
  /**
   * Search across meeting transcripts and summaries.
   * @param query - Free-text query.
   * @returns Promise with matching meetings (empty array for no matches).
   */
  async searchMeetings(query: string): Promise<TranscriptSearchResult[]> {
    return invoke<TranscriptSearchResult[]>('api_search_transcripts', { query });
  }
}

// Export singleton instance
export const searchService = new SearchService();

/**
 * Convenience wrapper matching the requested standalone signature.
 * @param query - Free-text query.
 */
export function searchMeetings(query: string): Promise<TranscriptSearchResult[]> {
  return searchService.searchMeetings(query);
}
