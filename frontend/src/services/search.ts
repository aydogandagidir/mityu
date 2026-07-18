/**
 * Search Service
 *
 * Handles trusted local meeting-evidence search Tauri calls.
 * Pure 1-to-1 wrapper over invoke() - no behavior changes vs. a direct invoke call.
 * This is the single typed definition of the ranked local search result shape.
 */

import { invoke } from '@tauri-apps/api/core';

/**
 * A single transcript-evidence hit returned by `api_search_evidence`.
 *
 * Results arrive in relevance order from the Rust core. Callers must preserve
 * that order rather than re-sorting them in the UI.
 */
export interface TranscriptSearchResult {
  /** Meeting id the hit belongs to. */
  id: string;
  /** Meeting title. */
  title: string;
  /** Snippet of the matched text, for preview. */
  matchContext: string;
  /** Original segment display timestamp. */
  timestamp: string;
  /** Where the hit was found. */
  matchedIn: 'transcript';
  /** Transcript row id used by the meeting view's jump-to-source path. */
  sourceChunkId: string;
  /** Recording-relative start time in seconds, when available. */
  audioStartTime: number | null;
}

const SEARCHABLE_TOKEN = /[\p{L}\p{N}]{2}/u;

/**
 * Mirror the backend's minimum searchable token length for immediate UI
 * feedback. The Rust query builder remains the security/performance authority.
 */
export function isEvidenceQuerySearchable(query: string): boolean {
  return SEARCHABLE_TOKEN.test(Array.from(query).slice(0, 256).join(''));
}

/**
 * Search Service
 * Singleton service for meeting content search.
 */
export class SearchService {
  /**
   * Search ranked, source-linked transcript evidence.
   * @param query - Free-text query.
   * @returns Promise with matching meetings (empty array for no matches).
   */
  async searchEvidence(query: string): Promise<TranscriptSearchResult[]> {
    return invoke<TranscriptSearchResult[]>('api_search_evidence', { query });
  }

  /** Backward-compatible frontend alias; uses the evidence command. */
  async searchMeetings(query: string): Promise<TranscriptSearchResult[]> {
    return this.searchEvidence(query);
  }
}

// Export singleton instance
export const searchService = new SearchService();

/** Search trusted, ranked transcript evidence. */
export function searchEvidence(query: string): Promise<TranscriptSearchResult[]> {
  return searchService.searchEvidence(query);
}

/**
 * Convenience wrapper matching the requested standalone signature.
 * @param query - Free-text query.
 */
export function searchMeetings(query: string): Promise<TranscriptSearchResult[]> {
  return searchService.searchEvidence(query);
}
