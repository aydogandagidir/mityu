/**
 * Storage Service
 *
 * Handles all meeting storage and retrieval Tauri backend calls (SQLite persistence).
 * Pure 1-to-1 wrapper - no error handling changes, exact same behavior as direct invoke calls.
 */

import { invoke } from '@tauri-apps/api/core';
import { Transcript } from '@/types';

export interface SaveMeetingRequest {
  meetingTitle: string;
  transcripts: Transcript[];
  folderPath: string | null;
  completionToken: string | null;
}

export interface SaveMeetingResponse {
  meeting_id: string;
}

export interface PendingRecordingPostProcessing {
  completionToken: string;
  persisted: boolean;
  meetingId: string | null;
}

export interface Meeting {
  id: string;
  title: string;
  [key: string]: any; // Allow additional properties from backend
}

/**
 * Storage Service
 * Singleton service for managing meeting storage operations
 */
export class StorageService {
  /**
   * Save meeting transcript to SQLite database
   * @param meetingTitle - Title of the meeting
   * @param transcripts - Array of transcript segments
   * @param folderPath - Legacy recording-folder correlation value; never a native path authority
   * @param completionToken - Opaque, one-time token emitted by the native recording-stop flow
   * @returns Promise with { meeting_id: string }
   */
  async saveMeeting(
    meetingTitle: string,
    transcripts: Transcript[],
    folderPath: string | null,
    completionToken: string | null,
  ): Promise<SaveMeetingResponse> {
    return invoke<SaveMeetingResponse>('api_save_transcript', {
      meetingTitle,
      transcripts,
      folderPath,
      completionToken,
    });
  }

  /** Release the native new-recording gate after the matching save flow is fully complete. */
  async acknowledgeRecordingPostProcessing(completionToken: string): Promise<void> {
    return invoke('api_acknowledge_recording_post_processing', {
      completionToken,
    });
  }

  async getPendingRecordingPostProcessing(): Promise<PendingRecordingPostProcessing | null> {
    return invoke<PendingRecordingPostProcessing | null>(
      'api_get_pending_recording_post_processing',
    );
  }

  async abandonRecordingPostProcessing(completionToken: string): Promise<void> {
    return invoke('api_abandon_recording_post_processing', {
      completionToken,
    });
  }

  /**
   * Get meeting details by ID
   * @param meetingId - ID of the meeting to fetch
   * @returns Promise with meeting details
   */
  async getMeeting(meetingId: string): Promise<Meeting> {
    return invoke<Meeting>('api_get_meeting', { meetingId });
  }

  /**
   * Get list of all meetings
   * @returns Promise with array of meetings
   */
  async getMeetings(): Promise<Meeting[]> {
    return invoke<Meeting[]>('api_get_meetings');
  }
}

// Export singleton instance
export const storageService = new StorageService();
