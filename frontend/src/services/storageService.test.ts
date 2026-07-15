import { afterEach, describe, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { storageService } from './storageService';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

const mockedInvoke = vi.mocked(invoke);

afterEach(() => {
  mockedInvoke.mockReset();
});

describe('recording completion save IPC contract', () => {
  const transcripts = [
    {
      id: 'segment-1',
      text: 'Approved transcript text',
      timestamp: '00:00:01',
    },
  ];

  it('passes the native one-time completion token for a recording-stop save', async () => {
    mockedInvoke.mockResolvedValueOnce({ meeting_id: 'meeting-1' });

    await storageService.saveMeeting(
      'Recorded meeting',
      transcripts,
      'C:\\Mityu\\recording-1',
      'completion-token-1',
    );

    expect(mockedInvoke).toHaveBeenCalledExactlyOnceWith('api_save_transcript', {
      meetingTitle: 'Recorded meeting',
      transcripts,
      folderPath: 'C:\\Mityu\\recording-1',
      completionToken: 'completion-token-1',
    });
  });

  it('makes recovery/manual saves explicitly tokenless and folderless', async () => {
    mockedInvoke.mockResolvedValueOnce({ meeting_id: 'meeting-recovered' });

    await storageService.saveMeeting('Recovered meeting', transcripts, null, null);

    expect(mockedInvoke).toHaveBeenCalledExactlyOnceWith('api_save_transcript', {
      meetingTitle: 'Recovered meeting',
      transcripts,
      folderPath: null,
      completionToken: null,
    });
  });

  it('acknowledges post-processing with the same native completion token', async () => {
    mockedInvoke.mockResolvedValueOnce(undefined);

    await storageService.acknowledgeRecordingPostProcessing('completion-token-1');

    expect(mockedInvoke).toHaveBeenCalledExactlyOnceWith(
      'api_acknowledge_recording_post_processing',
      { completionToken: 'completion-token-1' },
    );
  });

  it('queries and explicitly abandons interrupted native post-processing', async () => {
    mockedInvoke
      .mockResolvedValueOnce({
        completionToken: 'completion-token-1',
        persisted: false,
        meetingId: null,
      })
      .mockResolvedValueOnce(undefined);

    await expect(storageService.getPendingRecordingPostProcessing()).resolves.toMatchObject({
      completionToken: 'completion-token-1',
      persisted: false,
    });
    await storageService.abandonRecordingPostProcessing('completion-token-1');

    expect(mockedInvoke).toHaveBeenNthCalledWith(
      1,
      'api_get_pending_recording_post_processing',
    );
    expect(mockedInvoke).toHaveBeenNthCalledWith(
      2,
      'api_abandon_recording_post_processing',
      { completionToken: 'completion-token-1' },
    );
  });
});
