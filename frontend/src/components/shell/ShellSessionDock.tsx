'use client';

/**
 * The shell's wiring for `SessionDock` — DESIGN_SYSTEM.md §5.8.
 *
 * Split from the dock itself so the presentational component stays renderable in the
 * Tauri-free `/design/record` fixture: this half reads three contexts, that half reads
 * props. It also publishes `--bottom-chrome`, the single number every other piece of
 * fixed chrome (the sonner offset in `layout.tsx`, page bodies' bottom padding) uses to
 * stay clear of the dock — set as an inline style on `<html>` so a toast rendered in a
 * portal outside the shell can still read it.
 */

import React, { useEffect } from 'react';
import { useRecordingState } from '@/contexts/RecordingStateContext';
import { useRecordingSession } from '@/contexts/RecordingSessionContext';
import { SessionDock } from '@/components/shell/SessionDock';

export function ShellSessionDock() {
  const { isRecording, isPaused, activeDuration, isStopping } = useRecordingState();
  // `stopRecordingSession`, NOT `handleRecordingStop`: the latter is post-stop processing
  // only and never ends the capture, which is why this button used to do nothing on every
  // route but `/`.
  const { stopRecordingSession } = useRecordingSession();

  useEffect(() => {
    const root = document.documentElement;
    root.style.setProperty('--bottom-chrome', isRecording ? '40px' : '0px');
    return () => {
      root.style.setProperty('--bottom-chrome', '0px');
    };
  }, [isRecording]);

  return (
    <SessionDock
      isRecording={isRecording}
      isPaused={isPaused}
      activeDuration={activeDuration}
      isStopping={isStopping}
      onStop={() => void stopRecordingSession()}
    />
  );
}

export default ShellSessionDock;
