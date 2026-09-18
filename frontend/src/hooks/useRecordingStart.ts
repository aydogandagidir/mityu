import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useTranscripts } from '@/contexts/TranscriptContext';
import { useSidebar } from '@/components/Sidebar/SidebarProvider';
import { useConfig } from '@/contexts/ConfigContext';
import { useRecordingState, RecordingStatus } from '@/contexts/RecordingStateContext';
import { useRecordingConsent } from '@/contexts/RecordingConsentContext';
import { useLicensing } from '@/contexts/LicensingContext';
import { isLicenseRequiredError } from '@/types/licensing';
import { recordingService } from '@/services/recordingService';
import Analytics from '@/lib/analytics';
import { showRecordingNotification } from '@/lib/recordingNotification';
import { toast } from 'sonner';
import type { TranscriptionReadiness } from '@/types';

/** How a refusal is presented. Separated from the hook so it can be tested. */
export interface ReadinessMessage {
  kind: 'downloading' | 'missing';
  title: string;
  description: string;
}

/**
 * Turn a backend readiness answer into what the user sees.
 *
 * Presentation only — whether the engine is ready is decided in Rust
 * (`api_transcription_readiness`) and is never re-decided here. `description`
 * is the backend's own sentence; this supplies one only when the backend sent
 * none, so the UI can never claim a cause the backend did not state.
 */
export function readinessMessage(readiness: TranscriptionReadiness): ReadinessMessage {
  const description =
    readiness.reason ?? 'The transcription engine is not ready to record yet.';
  return readiness.downloading
    ? { kind: 'downloading', title: 'Model download in progress', description }
    : { kind: 'missing', title: 'Transcription model not ready', description };
}

interface UseRecordingStartReturn {
  handleRecordingStart: () => Promise<void>;
  isAutoStarting: boolean;
}

/**
 * Custom hook for managing recording start lifecycle.
 * Handles both manual start (button click) and auto-start (from sidebar navigation).
 *
 * Features:
 * - Meeting title generation (format: Meeting DD_MM_YY_HH_MM_SS)
 * - Transcript clearing on start
 * - Analytics tracking
 * - Recording notification display
 * - Auto-start from sidebar via sessionStorage flag
 */
export function useRecordingStart(
  isRecording: boolean,
  setIsRecording: (value: boolean) => void,
  showModal?: (name: 'modelSelector', message?: string) => void
): UseRecordingStartReturn {
  const [isAutoStarting, setIsAutoStarting] = useState(false);

  const { clearTranscripts, setMeetingTitle } = useTranscripts();
  const { setIsMeetingActive } = useSidebar();
  const { selectedDevices } = useConfig();
  const { setStatus } = useRecordingState();
  // C5: pre-recording multi-party consent gate. Awaited at the top of every
  // start path; resolves false when the user cancels/dismisses, which aborts
  // the start before any capture is triggered.
  const { ensureRecordingConsent } = useRecordingConsent();
  // ADR-0023: license gate. Gated backend commands reject with a
  // LICENSE_REQUIRED error once the trial expires / license is revoked; the
  // catch blocks below route that to the paywall dialog instead of the
  // generic error surfaces.
  const { openActivateDialog } = useLicensing();

  // Generate meeting title with timestamp
  const generateMeetingTitle = useCallback(() => {
    const now = new Date();
    const day = String(now.getDate()).padStart(2, '0');
    const month = String(now.getMonth() + 1).padStart(2, '0');
    const year = String(now.getFullYear()).slice(-2);
    const hours = String(now.getHours()).padStart(2, '0');
    const minutes = String(now.getMinutes()).padStart(2, '0');
    const seconds = String(now.getSeconds()).padStart(2, '0');
    return `Meeting ${day}_${month}_${year}_${hours}_${minutes}_${seconds}`;
  }, []);

  /**
   * Can the engine the user actually configured record right now?
   *
   * This used to be two calls, and both asked Parakeet — on every start path,
   * whatever the user had chosen. A Local Whisper user with a perfectly good
   * Whisper model was told "Transcription model not ready" and could never
   * record, while the Rust check three lines later would have accepted it.
   *
   * The engine decision now lives in Rust and nowhere else, and this renders
   * its sentence rather than composing one (ADR-0042: no validation mirrored
   * in TypeScript — a second copy drifts from the rule that governs).
   *
   * A failed call is reported, never treated as "not ready": refusing to start
   * because a pre-flight could not run is the silent failure this whole change
   * exists to remove. The authoritative validation still runs in the backend
   * at the moment recording starts, so proceeding is safe.
   */
  const checkTranscriptionReady = useCallback(async (): Promise<TranscriptionReadiness> => {
    try {
      return await invoke<TranscriptionReadiness>('api_transcription_readiness');
    } catch (error) {
      console.error('Failed to read transcription readiness:', error);
      return { ready: true, provider: 'unknown', downloading: false, reason: null };
    }
  }, []);

  /**
   * Tell the user why the recording did not start, in the backend's own words.
   * Every caller of this used to have its own copy of these two toasts.
   */
  const reportNotReady = useCallback(
    (readiness: TranscriptionReadiness, source: string) => {
      const message = readinessMessage(readiness);
      if (message.kind === 'downloading') {
        toast.info(message.title, { description: message.description, duration: 6000 });
        Analytics.trackButtonClick('start_recording_blocked_downloading', source);
        return;
      }
      toast.error(message.title, { description: message.description, duration: 6000 });
      // The model picker is the action the user needs; opening it with the
      // backend's sentence keeps the reason attached to the remedy.
      showModal?.('modelSelector', message.description);
      Analytics.trackButtonClick('start_recording_blocked_missing', source);
    },
    [showModal]
  );

  // Handle manual recording start (from button click)
  const handleRecordingStart = useCallback(async () => {
    try {
      console.log('handleRecordingStart called - checking recording consent');

      // C5: gate on multi-party consent BEFORE any capture is triggered.
      // Shows the reminder only if the local gate requires it; a cancel aborts
      // the start cleanly (return to IDLE, no backend call).
      const consented = await ensureRecordingConsent();
      if (!consented) {
        // A cancel is a decision, not a fault — but the button having no
        // visible effect is indistinguishable from a broken app, which is
        // exactly how an unreportable "it just doesn't record" starts.
        console.log('Recording start cancelled at consent gate');
        toast.info('Recording not started', {
          description: 'Recording consent was not confirmed.',
          duration: 4000,
        });
        setStatus(RecordingStatus.IDLE);
        return;
      }

      console.log('Consent confirmed - checking transcription readiness');

      // Check if Parakeet transcription model is ready before starting
      const readiness = await checkTranscriptionReady();
      if (!readiness.ready) {
        reportNotReady(readiness, 'home_page');
        setStatus(RecordingStatus.IDLE);
        return;
      }

      console.log('Transcription engine ready - setting up meeting title and state');

      const randomTitle = generateMeetingTitle();
      setMeetingTitle(randomTitle);

      // Set STARTING status before initiating backend recording
      setStatus(RecordingStatus.STARTING, 'Initializing recording...');

      // Start the actual backend recording
      console.log('Starting backend recording');
      const consentTicket = await recordingService.authorizeRecordingStart();
      await recordingService.startRecordingWithDevices(
        selectedDevices?.micDevice || null,
        selectedDevices?.systemDevice || null,
        randomTitle,
        consentTicket
      );
      console.log('Backend recording started successfully');

      // Update state after successful backend start
      // Note: RECORDING status will be set by RecordingStateContext event listener
      console.log('Setting isRecordingState to true');
      setIsRecording(true); // This will also update the sidebar via the useEffect
      clearTranscripts(); // Clear previous transcripts when starting new recording
      setIsMeetingActive(true);
      Analytics.trackButtonClick('start_recording', 'home_page');

      // Show recording notification if enabled
      await showRecordingNotification();
    } catch (error) {
      // ADR-0023: blocked by the license gate — show the paywall dialog and
      // swallow (no rethrow) so RecordingControls doesn't raise a device-error
      // dialog for a licensing condition.
      if (isLicenseRequiredError(error)) {
        console.log('Recording start blocked by license gate - showing paywall');
        openActivateDialog({ paywall: true });
        setStatus(RecordingStatus.IDLE);
        setIsRecording(false);
        Analytics.trackButtonClick('start_recording_blocked_license', 'home_page');
        return;
      }
      console.error('Failed to start recording:', error);
      setStatus(RecordingStatus.ERROR, error instanceof Error ? error.message : 'Failed to start recording');
      setIsRecording(false); // Reset state on error
      Analytics.trackButtonClick('start_recording_error', 'home_page');
      // Re-throw so RecordingControls can handle device-specific errors
      throw error;
    }
  }, [generateMeetingTitle, setMeetingTitle, setIsRecording, clearTranscripts, setIsMeetingActive, checkTranscriptionReady, reportNotReady, selectedDevices, showModal, setStatus, ensureRecordingConsent, openActivateDialog]);

  // Check for autoStartRecording flag and start recording automatically
  useEffect(() => {
    const checkAutoStartRecording = async () => {
      if (typeof window !== 'undefined') {
        const shouldAutoStart = sessionStorage.getItem('autoStartRecording');
        if (shouldAutoStart === 'true' && !isRecording && !isAutoStarting) {
          console.log('Auto-starting recording from navigation...');
          setIsAutoStarting(true);
          sessionStorage.removeItem('autoStartRecording'); // Clear the flag

          // C5: gate on multi-party consent BEFORE any capture is triggered.
          const consented = await ensureRecordingConsent();
          if (!consented) {
            console.log('Auto-start cancelled at consent gate');
            toast.info('Recording not started', {
              description: 'Recording consent was not confirmed.',
              duration: 4000,
            });
            setStatus(RecordingStatus.IDLE);
            setIsAutoStarting(false);
            return;
          }

          // Check if Parakeet transcription model is ready before starting
          const readiness = await checkTranscriptionReady();
          if (!readiness.ready) {
            reportNotReady(readiness, 'sidebar_auto');
            setStatus(RecordingStatus.IDLE);
            setIsAutoStarting(false);
            return;
          }

          // Start the actual backend recording
          try {
            // Generate meeting title
            const generatedMeetingTitle = generateMeetingTitle();

            // Set STARTING status before initiating backend recording
            setStatus(RecordingStatus.STARTING, 'Initializing recording...');

            console.log('Auto-starting backend recording');
            const consentTicket = await recordingService.authorizeRecordingStart();
            const result = await recordingService.startRecordingWithDevices(
              selectedDevices?.micDevice || null,
              selectedDevices?.systemDevice || null,
              generatedMeetingTitle,
              consentTicket
            );
            console.log('Auto-start backend recording result:', result);

            // Update UI state after successful backend start
            // Note: RECORDING status will be set by RecordingStateContext event listener
            setMeetingTitle(generatedMeetingTitle);
            setIsRecording(true);
            clearTranscripts();
            setIsMeetingActive(true);
            Analytics.trackButtonClick('start_recording', 'sidebar_auto');

            // Show recording notification if enabled
            await showRecordingNotification();
          } catch (error) {
            // ADR-0023: blocked by the license gate — paywall instead of alert.
            if (isLicenseRequiredError(error)) {
              console.log('Auto-start blocked by license gate - showing paywall');
              openActivateDialog({ paywall: true });
              setStatus(RecordingStatus.IDLE);
              Analytics.trackButtonClick('start_recording_blocked_license', 'sidebar_auto');
            } else {
              console.error('Failed to auto-start recording:', error);
              const message = error instanceof Error ? error.message : String(error);
              setStatus(RecordingStatus.ERROR, message || 'Failed to auto-start recording');
              // The backend's sentence, not "check the console": the user
              // cannot open one, and the reason is already in `error`.
              toast.error('Recording could not start', { description: message, duration: 8000 });
              Analytics.trackButtonClick('start_recording_error', 'sidebar_auto');
            }
          } finally {
            setIsAutoStarting(false);
          }
        }
      }
    };

    checkAutoStartRecording();
  }, [
    isRecording,
    isAutoStarting,
    selectedDevices,
    generateMeetingTitle,
    setMeetingTitle,
    setIsRecording,
    clearTranscripts,
    setIsMeetingActive,
    checkTranscriptionReady,
    reportNotReady,
    showModal,
    setStatus,
    ensureRecordingConsent,
    openActivateDialog,
  ]);

  // Listen for direct recording trigger from sidebar when already on home page
  useEffect(() => {
    const handleDirectStart = async () => {
      if (isRecording || isAutoStarting) {
        console.log('Recording already in progress, ignoring direct start event');
        return;
      }

      console.log('Direct start from sidebar - checking recording consent');
      setIsAutoStarting(true);

      // C5: gate on multi-party consent BEFORE any capture is triggered.
      const consented = await ensureRecordingConsent();
      if (!consented) {
        console.log('Direct start cancelled at consent gate');
        toast.info('Recording not started', {
          description: 'Recording consent was not confirmed.',
          duration: 4000,
        });
        setStatus(RecordingStatus.IDLE);
        setIsAutoStarting(false);
        return;
      }

      console.log('Consent confirmed - checking transcription readiness');

      // Check if Parakeet transcription model is ready before starting
      const readiness = await checkTranscriptionReady();
      if (!readiness.ready) {
        reportNotReady(readiness, 'sidebar_direct');
        setStatus(RecordingStatus.IDLE);
        setIsAutoStarting(false);
        return;
      }

      try {
        // Generate meeting title
        const generatedMeetingTitle = generateMeetingTitle();

        // Set STARTING status before initiating backend recording
        setStatus(RecordingStatus.STARTING, 'Initializing recording...');

        console.log('Starting backend recording');
        const consentTicket = await recordingService.authorizeRecordingStart();
        const result = await recordingService.startRecordingWithDevices(
          selectedDevices?.micDevice || null,
          selectedDevices?.systemDevice || null,
          generatedMeetingTitle,
          consentTicket
        );
        console.log('Backend recording result:', result);

        // Update UI state after successful backend start
        // Note: RECORDING status will be set by RecordingStateContext event listener
        setMeetingTitle(generatedMeetingTitle);
        setIsRecording(true);
        clearTranscripts();
        setIsMeetingActive(true);
        Analytics.trackButtonClick('start_recording', 'sidebar_direct');

        // Show recording notification if enabled
        await showRecordingNotification();
      } catch (error) {
        // ADR-0023: blocked by the license gate — paywall instead of alert.
        if (isLicenseRequiredError(error)) {
          console.log('Direct start blocked by license gate - showing paywall');
          openActivateDialog({ paywall: true });
          setStatus(RecordingStatus.IDLE);
          Analytics.trackButtonClick('start_recording_blocked_license', 'sidebar_direct');
        } else {
          console.error('Failed to start recording from sidebar:', error);
          const message = error instanceof Error ? error.message : String(error);
          setStatus(RecordingStatus.ERROR, message || 'Failed to start recording from sidebar');
          toast.error('Recording could not start', { description: message, duration: 8000 });
          Analytics.trackButtonClick('start_recording_error', 'sidebar_direct');
        }
      } finally {
        setIsAutoStarting(false);
      }
    };

    window.addEventListener('start-recording-from-sidebar', handleDirectStart);

    return () => {
      window.removeEventListener('start-recording-from-sidebar', handleDirectStart);
    };
  }, [
    isRecording,
    isAutoStarting,
    selectedDevices,
    generateMeetingTitle,
    setMeetingTitle,
    setIsRecording,
    clearTranscripts,
    setIsMeetingActive,
    checkTranscriptionReady,
    reportNotReady,
    showModal,
    setStatus,
    ensureRecordingConsent,
    openActivateDialog,
  ]);

  return {
    handleRecordingStart,
    isAutoStarting,
  };
}
