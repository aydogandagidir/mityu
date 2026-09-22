// @vitest-environment jsdom

/**
 * A stop that ends natively -- the tray -- must finish through THE stop hook, the one
 * whose setters own `RecordingSessionContext.isRecording`.
 *
 * It used to finish through a second instance of the hook, mounted by a separate
 * provider with no-op setters. The meeting saved; the session's flag stayed true; every
 * start path that reads it ("Recording already in progress, ignoring") refused silently
 * until the window was reloaded. Pinned here: the session provider itself hears
 * `recording-stop-complete` and forwards the payload to its own `handleRecordingStop`.
 */

import React from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, waitFor } from '@testing-library/react';

type Handler = (event: { payload: boolean }) => void;
const handlers = new Map<string, Handler>();
const unlisten = vi.fn();
const handleRecordingStop = vi.fn(async () => {});

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async (name: string, handler: Handler) => {
    handlers.set(name, handler);
    return unlisten;
  }),
}));

vi.mock('@tauri-apps/api/path', () => ({
  appDataDir: vi.fn(async () => 'C:/appdata'),
}));

vi.mock('@/lib/isTauri', () => ({ isTauri: () => true }));

vi.mock('@/services/recordingService', () => ({
  recordingService: { stopRecording: vi.fn() },
}));

vi.mock('@/lib/analytics', () => ({
  default: { trackTranscriptionSuccess: vi.fn() },
}));

vi.mock('@/hooks/useRecordingStop', () => ({
  useRecordingStop: () => ({ handleRecordingStop, setIsStopping: vi.fn() }),
}));

import { RecordingSessionProvider } from './RecordingSessionContext';

beforeEach(() => {
  handlers.clear();
  unlisten.mockReset();
  handleRecordingStop.mockReset().mockResolvedValue(undefined);
});

afterEach(() => cleanup());

describe('the session provider owns the native stop-complete event', () => {
  it('forwards the payload to its own stop hook', async () => {
    render(
      <RecordingSessionProvider>
        <div />
      </RecordingSessionProvider>,
    );

    await waitFor(() => expect(handlers.has('recording-stop-complete')).toBe(true));
    handlers.get('recording-stop-complete')!({ payload: true });

    await waitFor(() => expect(handleRecordingStop).toHaveBeenCalledWith(true));
  });

  it('releases the listener when it unmounts', async () => {
    const view = render(
      <RecordingSessionProvider>
        <div />
      </RecordingSessionProvider>,
    );
    await waitFor(() => expect(handlers.has('recording-stop-complete')).toBe(true));
    view.unmount();
    expect(unlisten).toHaveBeenCalledTimes(1);
  });
});
