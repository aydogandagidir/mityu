'use client';

/**
 * The recording session, hoisted out of the home route — DESIGN_SYSTEM.md §6.2, ADR-F.
 *
 * `useRecordingStop` installs `window.handleRecordingStop` on mount and DELETES it on
 * unmount (useRecordingStop.ts), and until now it mounted in exactly one place:
 * `app/page.tsx`. Navigating to /settings therefore removed the function the Rust tray
 * menu calls through `window.eval` — a rename produces no compile error and neither did
 * the disappearance. It also meant a global Stop could not exist: there was nothing on
 * any other route to call.
 *
 * This provider owns the stop lifecycle for the whole shell. It is mounted INSIDE the
 * main-app branch only, never during onboarding: the hook reconciles an interrupted save
 * on mount, and a first-run user being asked to recover a meeting they never recorded is
 * not a thing worth shipping.
 *
 * What it deliberately does NOT own: starting. `useRecordingStart` keeps its home on `/`
 * because the rail's Record button navigates there first (`sessionStorage.autoStartRecording`),
 * which is the behaviour the consent gate and the device pickers are written against.
 */

import React, {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from 'react';
import { appDataDir } from '@tauri-apps/api/path';
import { listen } from '@tauri-apps/api/event';
import { isTauri } from '@/lib/isTauri';
import { useRecordingStop } from '@/hooks/useRecordingStop';
import { recordingService } from '@/services/recordingService';
import Analytics from '@/lib/analytics';

export interface RecordingSessionContextType {
  /** The UI's own recording flag, written by the start/stop hooks. */
  isRecording: boolean;
  setIsRecording: (value: boolean) => void;
  /** Blocks a second start while a stop is still draining. */
  isRecordingDisabled: boolean;
  setIsRecordingDisabled: (value: boolean) => void;
  /** 🔒 Same function `window.handleRecordingStop` forwards to. */
  handleRecordingStop: (callApi: boolean) => Promise<void>;
  /**
   * A COMPLETE stop: end the capture natively, then post-process.
   *
   * `handleRecordingStop` is only the second half — it waits for completion metadata the
   * native stop produces and contains no `invoke` at all (its own note at
   * useRecordingStop.ts:170). Until this existed, the shell dock's Stop called that half
   * on its own, so off `/` nothing ever ended the capture: the button returned to "Stop"
   * after the 5s wait and the recording ran on, silently. Every Stop control in the shell
   * calls THIS.
   */
  stopRecordingSession: () => Promise<void>;
  setIsStopping: (value: boolean) => void;
}

/** Exported for the Tauri-free `/design/*` fixtures ONLY (DESIGN_SYSTEM.md §11.1). */
export const RecordingSessionContext = createContext<RecordingSessionContextType | null>(null);

export const useRecordingSession = () => {
  const context = useContext(RecordingSessionContext);
  if (!context) {
    throw new Error('useRecordingSession must be used within a RecordingSessionProvider');
  }
  return context;
};

export function RecordingSessionProvider({ children }: { children: React.ReactNode }) {
  const [isRecording, setIsRecording] = useState(false);
  /**
   * Previously held by `useRecordingStateSync`, whose polling body is gated on
   * `window.__TAURI__` — undefined in Tauri 2 unless `withGlobalTauri` is set, which this
   * app does not set. The state was the only part of that hook that ever ran.
   */
  const [isRecordingDisabled, setIsRecordingDisabled] = useState(false);

  const { handleRecordingStop, setIsStopping } = useRecordingStop(
    setIsRecording,
    setIsRecordingDisabled
  );

  /** Keeps a double-click from firing two native stops before the first returns. */
  const stopRequestInFlight = useRef(false);

  const stopRecordingSession = useCallback(async () => {
    if (stopRequestInFlight.current) return;
    stopRequestInFlight.current = true;

    // Match `/`'s feedback: the label changes on the click, not on the round-trip.
    // `isStopping` is derived from the shared status machine
    // (RecordingStateContext.tsx:238), so whoever finishes the stop clears it — this
    // cannot strand the dock on "Stopping…".
    setIsStopping(true);

    try {
      const dataDir = await appDataDir();
      const timestamp = new Date().toISOString().replace(/[:.]/g, '-');
      const savePath = `${dataDir}/recording-${timestamp}.wav`;

      // The same `{args:{save_path}}` shape RecordingControls uses, through the wrapper
      // that already had it and had lost its last caller.
      const completedByThisCall = await recordingService.stopRecording(savePath);

      if (!completedByThisCall) {
        // Rust's STOP_IN_PROGRESS guard (recording_commands.rs:574) handed ownership to
        // another caller — the pill on `/`, or the tray, which post-processes through
        // `recording-stop-complete`. Running it here too would claim the completion
        // token twice.
        return;
      }

      Analytics.trackTranscriptionSuccess();
      await handleRecordingStop(true);
    } catch (error) {
      const message =
        error instanceof Error ? error.message : typeof error === 'string' ? error : String(error);
      // Rust answers this when the capture already ended; it is not a failure.
      if (message.includes('No recording in progress')) return;
      console.error('Failed to stop recording from the shell dock:', error);
      // `false` takes the tokenless branch, which reports the error and keeps recovery
      // data rather than waiting on transcripts that will never be claimed.
      await handleRecordingStop(false);
    } finally {
      stopRequestInFlight.current = false;
    }
  }, [handleRecordingStop, setIsStopping]);

  /**
   * Stops that end natively -- the tray menu, the tray toggle -- finish through this
   * event rather than through a button in the shell. Until v1.2.3 it was heard by a
   * separate provider that ran `useRecordingStop` a SECOND time, with no-op setters. A
   * tray stop therefore saved the meeting but never reset `isRecording` here, and the
   * rail's Record button, ⌘⇧R and the tray's own Record entry were then refused as
   * "already recording" -- silently -- until the window was reloaded. There is one stop
   * hook now, and it is this one, so the flag it clears is the flag the start path reads.
   */
  const handleRecordingStopRef = useRef(handleRecordingStop);
  handleRecordingStopRef.current = handleRecordingStop;
  useEffect(() => {
    // Outside Tauri (fixtures, tests, the dev preview) there is no event source.
    if (!isTauri()) return;
    let disposed = false;
    let unlistenFn: (() => void) | undefined;

    (async () => {
      try {
        const unlisten = await listen<boolean>('recording-stop-complete', (event) => {
          console.log('[RecordingSession] recording-stop-complete:', event.payload);
          // The payload is the callApi flag; the tray sends `true` for a clean stop.
          void handleRecordingStopRef.current(event.payload);
        });
        if (disposed) {
          unlisten();
          return;
        }
        unlistenFn = unlisten;
      } catch (error) {
        console.error('[RecordingSession] Failed to listen for recording-stop-complete:', error);
      }
    })();

    return () => {
      disposed = true;
      unlistenFn?.();
    };
  }, []);

  const value = useMemo(
    () => ({
      isRecording,
      setIsRecording,
      isRecordingDisabled,
      setIsRecordingDisabled,
      handleRecordingStop,
      stopRecordingSession,
      setIsStopping,
    }),
    [isRecording, isRecordingDisabled, handleRecordingStop, stopRecordingSession, setIsStopping]
  );

  return (
    <RecordingSessionContext.Provider value={value}>{children}</RecordingSessionContext.Provider>
  );
}
