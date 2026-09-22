// @vitest-environment jsdom

/**
 * The provider polls the backend whenever the backend says a recording is live -- not
 * only when it hears `recording-started`.
 *
 * A reload during a recording (the tray's Settings entry does a full
 * `window.location.assign`) remounts the provider long after that event fired. The
 * mount sync then showed "Recording • 03:12" and the number never moved again, because
 * polling started in the event handler and nowhere else.
 */

import React from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, cleanup, render } from '@testing-library/react';

const backend = {
  is_recording: true,
  is_paused: false,
  is_active: true,
  recording_duration: 192,
  active_duration: 192,
};
const getRecordingState = vi.fn(async () => backend);
const noopUnlisten = () => {};

vi.mock('@/services/recordingService', () => ({
  recordingService: {
    getRecordingState: () => getRecordingState(),
    onRecordingStarted: vi.fn(async () => noopUnlisten),
    onRecordingStopped: vi.fn(async () => noopUnlisten),
    onRecordingPaused: vi.fn(async () => noopUnlisten),
    onRecordingResumed: vi.fn(async () => noopUnlisten),
  },
}));

import { RecordingStateProvider, RecordingStatus, useRecordingState } from './RecordingStateContext';

function Probe() {
  const { isRecording, status, recordingDuration } = useRecordingState();
  return (
    <output data-testid="probe">
      {String(isRecording)}|{status}|{String(recordingDuration)}
    </output>
  );
}

beforeEach(() => {
  vi.useFakeTimers();
  getRecordingState.mockClear();
  backend.is_recording = true;
  backend.recording_duration = 192;
});

afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

describe('mounting into a live recording', () => {
  it('keeps polling the backend and reports RECORDING', async () => {
    const view = render(
      <RecordingStateProvider>
        <Probe />
      </RecordingStateProvider>,
    );

    // The mount sync.
    await act(async () => {
      await Promise.resolve();
    });
    expect(getRecordingState).toHaveBeenCalledTimes(1);
    expect(view.getByTestId('probe').textContent).toBe(`true|${RecordingStatus.RECORDING}|192`);

    // Time passes; the duration the backend reports moves; so must the UI.
    backend.recording_duration = 193;
    await act(async () => {
      vi.advanceTimersByTime(500);
      await Promise.resolve();
    });
    expect(getRecordingState.mock.calls.length).toBeGreaterThanOrEqual(2);
    expect(view.getByTestId('probe').textContent).toBe(`true|${RecordingStatus.RECORDING}|193`);
  });

  it('stops polling once the backend says the recording ended', async () => {
    render(
      <RecordingStateProvider>
        <Probe />
      </RecordingStateProvider>,
    );
    await act(async () => {
      await Promise.resolve();
    });
    const afterMount = getRecordingState.mock.calls.length;

    backend.is_recording = false;
    await act(async () => {
      vi.advanceTimersByTime(500);
      await Promise.resolve();
    });
    const afterStop = getRecordingState.mock.calls.length;
    expect(afterStop).toBeGreaterThan(afterMount);

    await act(async () => {
      vi.advanceTimersByTime(2_000);
      await Promise.resolve();
    });
    expect(getRecordingState.mock.calls.length).toBe(afterStop);
  });
});
