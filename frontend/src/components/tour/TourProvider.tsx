'use client';

import React, {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useRef,
  useState,
} from 'react';
import { useRouter } from 'next/navigation';
import { invoke } from '@tauri-apps/api/core';
import { isTauri } from '@/lib/isTauri';
import {
  SAMPLE_MEETING_ID,
  sampleMeetingHref,
  TOUR_ANCHORS,
  TOUR_STEPS,
  clearTourCompleted,
  isTourCompleted,
  markTourCompleted,
  tourAnchorSelector,
} from '@/lib/tour';
import { WelcomeOverlay } from './WelcomeOverlay';
import { CoachMarkTour } from './CoachMarkTour';

interface TourContextValue {
  /** Settings → "Replay product tour": clears the flag, opens the sample, shows welcome. */
  replayTour: () => void;
  /**
   * `data-tour` anchor of the step currently on screen (null when the tour isn't
   * running). Pages read this to REVEAL a target before the coach-mark looks for
   * it — e.g. meeting-details expands / tabs to the summary for step 2 so the
   * spotlight lands on a real, visible block instead of the step being skipped.
   */
  activeAnchor: string | null;
}

const noop = () => {
  if (process.env.NODE_ENV !== 'production') {
    console.warn('[TourProvider] useTour() used outside a TourProvider — no-op.');
  }
};

const TourContext = createContext<TourContextValue>({
  replayTour: noop,
  activeAnchor: null,
});

export function useTour(): TourContextValue {
  return useContext(TourContext);
}

type Phase = 'idle' | 'welcome' | 'tour';

/**
 * Hands off to the user's first real recording without starting one (consent is
 * preserved — the tour never auto-records). Scrolls the sidebar record button
 * into view, focuses it, and pulses it briefly so the user knows where to click.
 */
function focusRecordButton() {
  if (typeof document === 'undefined') return;
  const el = document.querySelector(
    tourAnchorSelector(TOUR_ANCHORS.recordButton),
  ) as HTMLElement | null;
  if (!el) return;
  try {
    el.scrollIntoView({ block: 'nearest', inline: 'nearest' });
  } catch {
    /* ignore */
  }
  try {
    el.focus({ preventScroll: true });
  } catch {
    /* ignore */
  }
  el.classList.add('mityu-tour-pulse');
  window.setTimeout(() => el.classList.remove('mityu-tour-pulse'), 2400);
}

/**
 * Global orchestrator for the first-run product tour. Mounted once inside the
 * main-app shell (never during setup onboarding). Owns the state machine, the
 * completion flag, post-onboarding navigation to the sample meeting, and the
 * auto-show gate. Renders the welcome overlay + coach-mark tour on top of the app.
 *
 * Local-first: the only backend calls are local `invoke`s guarded by isTauri();
 * every failure path degrades to "do nothing" so a first run can never error.
 */
export function TourProvider({ children }: { children: React.ReactNode }) {
  const router = useRouter();
  const [phase, setPhase] = useState<Phase>('idle');
  const [stepIndex, setStepIndex] = useState(0);

  const autoRanRef = useRef(false);

  const navigateToSample = useCallback(() => {
    if (typeof window === 'undefined') return;
    const params = new URLSearchParams(window.location.search);
    const onSample =
      window.location.pathname.replace(/\/$/, '').endsWith('/meeting-details') &&
      params.get('id') === SAMPLE_MEETING_ID;
    if (!onSample) router.push(sampleMeetingHref);
  }, [router]);

  const openWelcome = useCallback(() => {
    setStepIndex(0);
    setPhase('welcome');
  }, []);

  const replayTour = useCallback(() => {
    // Re-arm the flag and relaunch the welcome overlay immediately. Then move to a
    // meeting the coach-marks can anchor on: the seeded sample if present (the
    // designed target), otherwise stay on the meeting already open, else the most
    // recent meeting — so upgraded installs that never got the sample still get a
    // real tour instead of an empty one.
    clearTourCompleted();
    openWelcome();
    (async () => {
      try {
        if (typeof window === 'undefined') return;
        const meetings = await invoke<Array<{ id: string }>>('api_get_meetings');
        const list = Array.isArray(meetings) ? meetings : [];
        const hasSample = list.some((m) => m?.id === SAMPLE_MEETING_ID);
        const params = new URLSearchParams(window.location.search);
        const onMeeting =
          window.location.pathname.replace(/\/$/, '').endsWith('/meeting-details') &&
          !!params.get('id');
        if (hasSample) {
          navigateToSample();
        } else if (!onMeeting && list.length > 0) {
          router.push(`/meeting-details?id=${list[0].id}`);
        }
        // else: no sample but already viewing a meeting → run the tour right here.
      } catch {
        // Local read failed — leave the user where they are; coach-marks skip
        // any absent targets gracefully.
      }
    })();
  }, [navigateToSample, openWelcome, router]);

  // Auto-show once per app launch, only when every gate passes.
  useEffect(() => {
    if (autoRanRef.current) return;
    autoRanRef.current = true;

    if (!isTauri()) return; // browser dev-preview never auto-shows
    if (isTourCompleted()) return; // already finished / skipped

    let cancelled = false;
    (async () => {
      try {
        // Belt-and-braces: this provider only mounts post-onboarding, but confirm.
        const status = await invoke<{ completed: boolean } | null>(
          'get_onboarding_status',
        );
        if (cancelled || !status?.completed) return;

        const meetings = await invoke<Array<{ id: string }>>('api_get_meetings');
        if (cancelled) return;
        const exists =
          Array.isArray(meetings) &&
          meetings.some((m) => m?.id === SAMPLE_MEETING_ID);
        if (!exists) return; // no sample (older installs) → fall back to normal home

        navigateToSample();
        openWelcome();
      } catch {
        // Any local read failing must never surface — just skip the tour.
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [navigateToSample, openWelcome]);

  // --- Welcome handlers ----------------------------------------------------
  const handleTakeTour = useCallback(() => {
    setStepIndex(0);
    setPhase('tour');
  }, []);

  const handleSkipWelcome = useCallback(() => {
    markTourCompleted();
    setPhase('idle');
    focusRecordButton();
  }, []);

  const handleDismissWelcome = useCallback(() => {
    // Close button / Esc / outside click: dismiss quietly so it never nags.
    markTourCompleted();
    setPhase('idle');
  }, []);

  // --- Tour handlers -------------------------------------------------------
  const handleBack = useCallback(() => {
    setStepIndex((i) => Math.max(0, i - 1));
  }, []);

  const handleNext = useCallback(() => {
    setStepIndex((i) => Math.min(TOUR_STEPS.length - 1, i + 1));
  }, []);

  const handleSkipTour = useCallback(() => {
    markTourCompleted();
    setPhase('idle');
  }, []);

  const handleFinishTour = useCallback(() => {
    markTourCompleted();
    setPhase('idle');
    focusRecordButton();
  }, []);

  return (
    <TourContext.Provider
      value={{
        replayTour,
        activeAnchor:
          phase === 'tour' ? TOUR_STEPS[stepIndex]?.anchor ?? null : null,
      }}
    >
      {children}

      <WelcomeOverlay
        open={phase === 'welcome'}
        onTakeTour={handleTakeTour}
        onSkip={handleSkipWelcome}
        onDismiss={handleDismissWelcome}
      />

      {phase === 'tour' && (
        <CoachMarkTour
          steps={TOUR_STEPS}
          stepIndex={stepIndex}
          onBack={handleBack}
          onNext={handleNext}
          onSkip={handleSkipTour}
          onFinish={handleFinishTour}
        />
      )}
    </TourContext.Provider>
  );
}

export default TourProvider;
