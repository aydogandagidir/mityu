'use client';

import React, {
  createContext,
  useCallback,
  useContext,
  useRef,
  useState,
} from 'react';
import { RecordingConsentDialog } from '@/components/consent/RecordingConsentDialog';
import {
  setRecordingConsentAcknowledged,
  shouldPromptBeforeRecording,
} from '@/lib/recordingConsent';

/**
 * RecordingConsentContext (BACKLOG C5).
 *
 * Owns the single, app-level pre-recording consent dialog and exposes one
 * promise-based gate — {@link RecordingConsentContextType.ensureRecordingConsent} —
 * that every recording-start path awaits. This keeps the gate in ONE place
 * regardless of which trigger started the recording (record button, sidebar
 * auto-start, tray/keyboard "start-recording-from-sidebar").
 *
 * ensureRecordingConsent():
 *  - reads the local consent flags (recording-consent.json);
 *  - if no prompt is required (already acknowledged and not "ask every time"),
 *    resolves `true` immediately — recording proceeds with no interruption;
 *  - otherwise shows the dialog and resolves `true` on confirm (persisting the
 *    acknowledgment if the user ticked "don't show again") or `false` on cancel.
 *
 * Local-first: all persistence is a local plugin-store; no network.
 */
interface RecordingConsentContextType {
  /**
   * Resolve `true` to proceed with the recording start, `false` to abort.
   * Shows the consent dialog only when the local gate requires it.
   */
  ensureRecordingConsent: () => Promise<boolean>;
}

const RecordingConsentContext = createContext<RecordingConsentContextType | null>(null);

export function useRecordingConsent(): RecordingConsentContextType {
  const ctx = useContext(RecordingConsentContext);
  if (!ctx) {
    throw new Error('useRecordingConsent must be used within a RecordingConsentProvider');
  }
  return ctx;
}

export function RecordingConsentProvider({ children }: { children: React.ReactNode }) {
  const [isOpen, setIsOpen] = useState(false);
  // The pending gate's resolver — settled exactly once per open dialog.
  const resolverRef = useRef<((proceed: boolean) => void) | null>(null);

  const settle = useCallback((proceed: boolean) => {
    const resolve = resolverRef.current;
    resolverRef.current = null;
    setIsOpen(false);
    resolve?.(proceed);
  }, []);

  const ensureRecordingConsent = useCallback(async (): Promise<boolean> => {
    const mustPrompt = await shouldPromptBeforeRecording();
    if (!mustPrompt) {
      return true;
    }

    // If a dialog is somehow already pending (double-trigger), reuse it by
    // rejecting the new caller's need to open a second one: resolve the previous
    // as cancelled so we never leak a hanging promise, then open fresh.
    if (resolverRef.current) {
      settle(false);
    }

    return new Promise<boolean>((resolve) => {
      resolverRef.current = resolve;
      setIsOpen(true);
    });
  }, [settle]);

  const handleConfirm = useCallback(
    (dontShowAgain: boolean) => {
      // Persist the one-time acknowledgment if requested. Best-effort: even if the
      // store write fails, we still proceed with THIS recording (the user already
      // confirmed); the gate will simply re-prompt next time.
      if (dontShowAgain) {
        setRecordingConsentAcknowledged(true).catch((error) => {
          console.error('[RecordingConsent] Failed to persist acknowledgment:', error);
        });
      }
      settle(true);
    },
    [settle],
  );

  const handleCancel = useCallback(() => {
    settle(false);
  }, [settle]);

  return (
    <RecordingConsentContext.Provider value={{ ensureRecordingConsent }}>
      {children}
      <RecordingConsentDialog open={isOpen} onConfirm={handleConfirm} onCancel={handleCancel} />
    </RecordingConsentContext.Provider>
  );
}
