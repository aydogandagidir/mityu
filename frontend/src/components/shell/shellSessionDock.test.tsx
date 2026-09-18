// @vitest-environment jsdom

/**
 * The dock's WIRING, which is the half `sessionDock.test.tsx` cannot see.
 *
 * That file renders the presentational `SessionDock` with a `vi.fn()` onStop, so it passes
 * whether or not the thing behind the button ends the capture. It did not: `onStop` was
 * bound to `handleRecordingStop`, which is post-stop processing and contains no `invoke`
 * (useRecordingStop.ts:170). On /settings, /actions and /meeting-details — every route
 * where this is the ONLY Stop — pressing it changed the label to "Stopping…", let the 5s
 * completion-token wait expire, put "Stop" back and left the recording running, with no
 * toast and no error. A manual smoke test on Windows caught it; nothing automated could.
 *
 * So what is pinned here is the contract the fixture shot cannot express: a click reaches
 * the native `stop_recording` with the frozen `{args:{save_path}}` shape, and
 * post-processing runs only when this caller owns the shutdown.
 */

import React from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';

const stopRecording = vi.fn();
const handleRecordingStop = vi.fn(async () => {});
const setIsStopping = vi.fn();
const trackTranscriptionSuccess = vi.fn();

vi.mock('@tauri-apps/api/path', () => ({
  appDataDir: vi.fn(async () => 'C:/appdata'),
}));

vi.mock('@/services/recordingService', () => ({
  recordingService: {
    stopRecording: (path: string) => stopRecording(path),
  },
}));

vi.mock('@/lib/analytics', () => ({
  default: { trackTranscriptionSuccess: () => trackTranscriptionSuccess() },
}));

vi.mock('@/hooks/useRecordingStop', () => ({
  useRecordingStop: () => ({ handleRecordingStop, setIsStopping }),
}));

vi.mock('@/contexts/RecordingStateContext', () => ({
  useRecordingState: () => ({
    isRecording: true,
    isPaused: false,
    activeDuration: 42,
    isStopping: false,
  }),
  RecordingStatus: { STOPPING: 'STOPPING' },
}));

import { RecordingSessionProvider } from '@/contexts/RecordingSessionContext';
import { ShellSessionDock } from './ShellSessionDock';

function renderDock() {
  return render(
    <RecordingSessionProvider>
      <ShellSessionDock />
    </RecordingSessionProvider>
  );
}

const clickStop = () => fireEvent.click(screen.getByRole('button', { name: 'Stop recording' }));

beforeEach(() => {
  stopRecording.mockReset().mockResolvedValue(true);
  handleRecordingStop.mockReset().mockResolvedValue(undefined);
  setIsStopping.mockReset();
  trackTranscriptionSuccess.mockReset();
});

afterEach(cleanup);

describe('ShellSessionDock wiring', () => {
  it('ends the capture natively — not just the post-processing half', async () => {
    renderDock();
    clickStop();

    await waitFor(() => expect(stopRecording).toHaveBeenCalledTimes(1));
    expect(stopRecording.mock.calls[0][0]).toMatch(/^C:\/appdata\/recording-.*\.wav$/);
  });

  it('post-processes only after the native stop, and only when it owns the shutdown', async () => {
    renderDock();
    clickStop();

    await waitFor(() => expect(handleRecordingStop).toHaveBeenCalledWith(true));
    expect(stopRecording).toHaveBeenCalledTimes(1);
  });

  it('does not post-process when another caller already owns the stop', async () => {
    // Rust's STOP_IN_PROGRESS guard answers `false` when the pill on `/` or the tray got
    // there first. Running post-processing anyway would claim the completion token twice.
    stopRecording.mockResolvedValue(false);
    renderDock();
    clickStop();

    await waitFor(() => expect(stopRecording).toHaveBeenCalledTimes(1));
    expect(handleRecordingStop).not.toHaveBeenCalled();
  });

  it('marks the session stopping on the click rather than on the round-trip', async () => {
    renderDock();
    clickStop();
    await waitFor(() => expect(setIsStopping).toHaveBeenCalledWith(true));
  });

  it('takes the tokenless branch when the native stop throws', async () => {
    stopRecording.mockRejectedValue(new Error('device disappeared'));
    renderDock();
    clickStop();

    await waitFor(() => expect(handleRecordingStop).toHaveBeenCalledWith(false));
  });

  it('treats "No recording in progress" as already stopped, not as a failure', async () => {
    stopRecording.mockRejectedValue(new Error('No recording in progress'));
    renderDock();
    clickStop();

    await waitFor(() => expect(stopRecording).toHaveBeenCalledTimes(1));
    expect(handleRecordingStop).not.toHaveBeenCalled();
  });

  it('a double click fires one native stop, not two', async () => {
    let release: (v: boolean) => void = () => {};
    stopRecording.mockImplementation(() => new Promise<boolean>((r) => { release = r; }));
    renderDock();

    clickStop();
    clickStop();
    await waitFor(() => expect(stopRecording).toHaveBeenCalledTimes(1));

    release(true);
    await waitFor(() => expect(handleRecordingStop).toHaveBeenCalledTimes(1));
  });
});
