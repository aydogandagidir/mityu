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

import React, { createContext, useContext, useMemo, useState } from 'react';
import { useRecordingStop } from '@/hooks/useRecordingStop';

export interface RecordingSessionContextType {
  /** The UI's own recording flag, written by the start/stop hooks. */
  isRecording: boolean;
  setIsRecording: (value: boolean) => void;
  /** Blocks a second start while a stop is still draining. */
  isRecordingDisabled: boolean;
  setIsRecordingDisabled: (value: boolean) => void;
  /** 🔒 Same function `window.handleRecordingStop` forwards to. */
  handleRecordingStop: (callApi: boolean) => Promise<void>;
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

  const value = useMemo(
    () => ({
      isRecording,
      setIsRecording,
      isRecordingDisabled,
      setIsRecordingDisabled,
      handleRecordingStop,
      setIsStopping,
    }),
    [isRecording, isRecordingDisabled, handleRecordingStop, setIsStopping]
  );

  return (
    <RecordingSessionContext.Provider value={value}>{children}</RecordingSessionContext.Provider>
  );
}
