import { afterEach, describe, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { recordingService } from './recordingService';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(),
}));

const mockedInvoke = vi.mocked(invoke);

afterEach(() => {
  mockedInvoke.mockReset();
});

describe('recording consent authorization IPC contract', () => {
  it('requests an opaque one-time ticket from the native gate', async () => {
    mockedInvoke.mockResolvedValueOnce('ticket-123');

    await expect(recordingService.authorizeRecordingStart()).resolves.toBe('ticket-123');
    expect(mockedInvoke).toHaveBeenCalledExactlyOnceWith('authorize_recording_start');
  });

  it('passes the ticket to the basic recording-start command', async () => {
    mockedInvoke.mockResolvedValueOnce(undefined);

    await recordingService.startRecording('ticket-basic');

    expect(mockedInvoke).toHaveBeenCalledExactlyOnceWith('start_recording', {
      consent_ticket: 'ticket-basic',
    });
  });

  it('passes the same ticket to the device-aware recording-start command', async () => {
    mockedInvoke.mockResolvedValueOnce(undefined);

    await recordingService.startRecordingWithDevices(
      'microphone-id',
      'system-id',
      'Meeting title',
      'ticket-devices',
    );

    expect(mockedInvoke).toHaveBeenCalledExactlyOnceWith(
      'start_recording_with_devices_and_meeting',
      {
        mic_device_name: 'microphone-id',
        system_device_name: 'system-id',
        meeting_name: 'Meeting title',
        consent_ticket: 'ticket-devices',
      },
    );
  });
});

describe('recording stop ownership IPC contract', () => {
  it('returns whether this caller owned native shutdown', async () => {
    mockedInvoke.mockResolvedValueOnce(false);

    await expect(recordingService.stopRecording('recording.wav')).resolves.toBe(false);
    expect(mockedInvoke).toHaveBeenCalledExactlyOnceWith('stop_recording', {
      args: { save_path: 'recording.wav' },
    });
  });
});
