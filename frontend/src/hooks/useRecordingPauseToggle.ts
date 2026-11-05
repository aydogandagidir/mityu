'use client';

/**
 * Pause ⇄ resume for the live recording, shared by every surface that offers it (the
 * shell's SessionDock and the ⌘K palette), so they cannot drift apart.
 *
 * The paused flag itself is owned by RecordingStateContext, which hears the backend's
 * pause/resume events — a tray pause and these controls stay in agreement. This hook
 * only sends the command, guards against a double-click sending pause and resume back
 * to back, and reports a failure in words.
 */

import { useCallback, useRef, useState } from 'react';
import { toast } from 'sonner';
import { useRecordingState } from '@/contexts/RecordingStateContext';
import { recordingService } from '@/services/recordingService';

export function useRecordingPauseToggle() {
  const { isRecording, isPaused } = useRecordingState();
  const [isPauseBusy, setIsPauseBusy] = useState(false);
  const inFlight = useRef(false);

  const togglePause = useCallback(async () => {
    if (!isRecording || inFlight.current) return;
    inFlight.current = true;
    setIsPauseBusy(true);
    try {
      if (isPaused) {
        await recordingService.resumeRecording();
      } else {
        await recordingService.pauseRecording();
      }
    } catch (error) {
      console.error('Failed to toggle recording pause:', error);
      toast.error(isPaused ? 'Could not resume the recording' : 'Could not pause the recording', {
        description: isPaused
          ? 'It is still paused. Try again, or stop it to keep what you have.'
          : 'The recording is still running. Try again, or stop it to keep what you have.',
      });
    } finally {
      inFlight.current = false;
      setIsPauseBusy(false);
    }
  }, [isRecording, isPaused]);

  return { togglePause, isPauseBusy, isPaused };
}

