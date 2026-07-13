'use client';

/**
 * /design/tour — a Tauri-free preview of the first-run product tour.
 *
 * It renders a self-contained MOCK of the sample-meeting view (transcript panel,
 * a source-linked summary block + Approve control, and a record button), each
 * carrying the real `data-tour` anchors, then drives the SAME presentational
 * components used in production (WelcomeOverlay + CoachMarkTour) so the welcome
 * card and every coach-mark step can be screenshotted in a plain browser.
 *
 * Not linked from product navigation and never mounted by the tour orchestrator
 * (that is isTauri()-gated). The coach-mark lookups are scoped to this page's own
 * container, so they target these mocks and never the real sidebar.
 *
 *   Preview URLs (dev server on :3118):
 *     /design/tour?tour=welcome   → the welcome overlay
 *     /design/tour?tour=1         → step 1 (transcript)
 *     /design/tour?tour=2         → step 2 (summary + approve)
 *     /design/tour?tour=3         → step 3 (record button)
 *     &hideTarget=1               → drop step 2's anchor to exercise the
 *                                   "target missing → centered, never skipped"
 *                                   fallback and verify Back/Next still work.
 */

import { Suspense, useEffect, useMemo, useRef, useState } from 'react';
import { useSearchParams } from 'next/navigation';
import {
  AlertTriangle,
  Check,
  Clock,
  FileText,
  Mic,
  Pencil,
  Sparkles,
  X,
} from 'lucide-react';
import { WelcomeOverlay } from '@/components/tour/WelcomeOverlay';
import { CoachMarkTour } from '@/components/tour/CoachMarkTour';
import { TOUR_ANCHORS, TOUR_STEPS } from '@/lib/tour';

type Phase = 'idle' | 'welcome' | 'tour';

const SAMPLE_TRANSCRIPT: { t: string; text: string }[] = [
  { t: '00:00', text: "Okay, let's lock the Q3 launch. I'd rather slip a week than ship the old onboarding." },
  { t: '00:20', text: "Design can't finish until the final copy lands — Friday is the real deadline for that." },
  { t: '00:55', text: 'Amira will send the revised budget to finance before Thursday’s review.' },
  { t: '01:40', text: 'Then we confirm the new date with vendors and move the weekly check-in to Thursday.' },
  { t: '02:25', text: 'Support is fine as long as the docs land two days before the public date.' },
];

function PreviewChrome({ hideSummaryAnchor }: { hideSummaryAnchor: boolean }) {
  return (
    <>
      {/* Report header mock */}
      <div className="border-b border-border bg-card px-6 py-4">
        <h1 className="text-xl font-semibold text-foreground">Sample — Q3 launch planning</h1>
        <p className="mt-0.5 text-sm text-muted-foreground">14 segments · ~16 min · on-device</p>
      </div>

      <div className="flex flex-1 min-h-0">
        {/* Transcript panel mock (step 1 anchor) */}
        <div
          data-tour={TOUR_ANCHORS.transcriptPanel}
          className="flex w-1/2 min-w-0 flex-col border-r border-border bg-background"
        >
          <div className="flex items-center gap-2 border-b border-border px-4 py-2.5">
            <span className="grid h-6 w-6 place-items-center rounded-md bg-accent text-accent-foreground">
              <FileText className="h-3.5 w-3.5" />
            </span>
            <h2 className="text-sm font-semibold text-foreground">Transcript</h2>
            <span className="rounded-full bg-muted px-2 py-0.5 text-xs tabular-nums text-muted-foreground">
              14
            </span>
          </div>
          <div className="flex-1 space-y-3 overflow-y-auto p-4">
            {SAMPLE_TRANSCRIPT.map((seg, i) => (
              <div key={i} className="flex gap-3">
                <span className="mt-0.5 shrink-0 font-mono text-xs text-muted-foreground">{seg.t}</span>
                <p className="text-sm text-foreground">{seg.text}</p>
              </div>
            ))}
          </div>
        </div>

        {/* Summary panel mock */}
        <div className="flex w-1/2 min-w-0 flex-col bg-accent/25">
          <div className="flex items-center gap-2 border-b border-border bg-accent/50 px-4 py-2.5">
            <span className="grid h-6 w-6 place-items-center rounded-md bg-primary/10 text-primary">
              <Sparkles className="h-3.5 w-3.5" />
            </span>
            <h2 className="text-sm font-semibold text-foreground">Summary</h2>
          </div>

          <div className="flex-1 overflow-y-auto p-6">
            <div className="mx-auto max-w-xl">
              {/* Always-on AI transparency banner (mirrors production) */}
              <div className="mb-4 flex items-start gap-3 rounded-lg border border-amber-300 bg-amber-50 p-3 dark:border-amber-500/30 dark:bg-amber-500/10">
                <AlertTriangle className="mt-0.5 h-5 w-5 shrink-0 text-amber-600 dark:text-amber-400" />
                <div className="text-sm text-amber-900 dark:text-amber-200">
                  <p className="font-semibold">AI-generated · review required</p>
                </div>
              </div>

              <div className="rounded-xl border border-border bg-card p-4 shadow-sm">
                <h3 className="mb-2 text-sm font-semibold text-foreground">Overview</h3>

                {/* First summary block (step 2 anchor): text + source chip + approve.
                    hideSummaryAnchor drops the data-tour so the preview can verify
                    the "target missing → centered step, Back still works" path. */}
                <div
                  {...(hideSummaryAnchor
                    ? {}
                    : { 'data-tour': TOUR_ANCHORS.summaryApproveBlock })}
                  className="-mx-2 rounded-lg px-2 py-3"
                >
                  <div className="flex items-start justify-between gap-3">
                    <div className="min-w-0 flex-1">
                      <p className="text-sm text-foreground">
                        The team agreed to move the Q3 launch to March 21, pending vendor confirmation.
                      </p>
                      <div className="mt-2 flex flex-wrap items-center gap-2">
                        <span className="rounded-full border border-green-300 bg-green-100 px-2 py-0.5 text-xs font-medium text-green-800 dark:border-green-500/30 dark:bg-green-500/15 dark:text-green-300">
                          Approved
                        </span>
                        <span className="inline-flex items-center gap-1 rounded-full border border-border bg-muted/60 px-2 py-0.5 text-xs text-muted-foreground">
                          <Clock className="h-3 w-3" />
                          Source
                        </span>
                      </div>
                    </div>
                    <div className="flex shrink-0 items-center gap-1">
                      <span className="grid h-8 w-8 place-items-center rounded-md bg-green-600 text-white">
                        <Check className="h-4 w-4" />
                      </span>
                      <span className="grid h-8 w-8 place-items-center rounded-md border border-input bg-background text-foreground">
                        <Pencil className="h-4 w-4" />
                      </span>
                      <span className="grid h-8 w-8 place-items-center rounded-md border border-input bg-background text-red-600">
                        <X className="h-4 w-4" />
                      </span>
                    </div>
                  </div>
                </div>

                <div className="mt-1 border-t border-border px-2 py-3">
                  <p className="text-sm text-foreground">
                    Design is blocked on the final launch copy until Friday, the real deadline.
                  </p>
                </div>
              </div>
            </div>
          </div>

          {/* Mock sidebar-style record button (step 3 anchor) */}
          <div className="border-t border-border bg-card p-3">
            <button
              data-tour={TOUR_ANCHORS.recordButton}
              type="button"
              className="flex w-full items-center justify-center gap-2 rounded-xl bg-red-500 px-3 py-2.5 text-sm font-semibold text-white shadow-sm hover:bg-red-600"
            >
              <Mic className="h-4 w-4" />
              Start recording
            </button>
          </div>
        </div>
      </div>
    </>
  );
}

function TourPreview() {
  const search = useSearchParams();
  const tourParam = search.get('tour');
  const hideTargetParam = search.get('hideTarget') === '1';
  const containerRef = useRef<HTMLDivElement>(null);
  const [phase, setPhase] = useState<Phase>('idle');
  const [stepIndex, setStepIndex] = useState(0);
  const [hideSummaryAnchor, setHideSummaryAnchor] = useState(false);

  // Derive the initial state from the URL so a direct link screenshots cleanly.
  useEffect(() => {
    if (tourParam === 'welcome') {
      setPhase('welcome');
      setStepIndex(0);
    } else if (tourParam === '1' || tourParam === '2' || tourParam === '3') {
      setPhase('tour');
      setStepIndex(Number(tourParam) - 1);
    } else {
      setPhase('idle');
    }
  }, [tourParam]);

  useEffect(() => {
    setHideSummaryAnchor(hideTargetParam);
  }, [hideTargetParam]);

  // Scope coach-mark lookups to this page's mocks (never the real sidebar).
  const getRoot = useMemo(() => () => containerRef.current ?? document, []);

  return (
    <div ref={containerRef} className="flex h-screen flex-col bg-muted">
      {/* Dev toolbar — flip states without editing the URL. */}
      <div className="flex flex-wrap items-center gap-2 border-b border-border bg-card px-4 py-2 text-sm">
        <span className="font-semibold text-foreground">Tour preview</span>
        <span className="text-muted-foreground">·</span>
        <PreviewButton label="Welcome" active={phase === 'welcome'} onClick={() => { setPhase('welcome'); setStepIndex(0); }} />
        <PreviewButton label="Step 1" active={phase === 'tour' && stepIndex === 0} onClick={() => { setPhase('tour'); setStepIndex(0); }} />
        <PreviewButton label="Step 2" active={phase === 'tour' && stepIndex === 1} onClick={() => { setPhase('tour'); setStepIndex(1); }} />
        <PreviewButton label="Step 3" active={phase === 'tour' && stepIndex === 2} onClick={() => { setPhase('tour'); setStepIndex(2); }} />
        <PreviewButton label="Reset" active={phase === 'idle'} onClick={() => setPhase('idle')} />
        <PreviewButton
          label={hideSummaryAnchor ? 'Step-2 target: hidden' : 'Step-2 target: shown'}
          active={hideSummaryAnchor}
          onClick={() => setHideSummaryAnchor((v) => !v)}
        />
        <span className="ml-auto text-xs text-muted-foreground">
          ?tour=welcome · 1 · 2 · 3 · &amp;hideTarget=1
        </span>
      </div>

      <PreviewChrome hideSummaryAnchor={hideSummaryAnchor} />

      <WelcomeOverlay
        open={phase === 'welcome'}
        onTakeTour={() => {
          setPhase('tour');
          setStepIndex(0);
        }}
        onSkip={() => setPhase('idle')}
        onDismiss={() => setPhase('idle')}
      />

      {phase === 'tour' && (
        <CoachMarkTour
          steps={TOUR_STEPS}
          stepIndex={stepIndex}
          onBack={() => setStepIndex((i) => Math.max(0, i - 1))}
          onNext={() => setStepIndex((i) => Math.min(TOUR_STEPS.length - 1, i + 1))}
          onSkip={() => setPhase('idle')}
          onFinish={() => setPhase('idle')}
          getRoot={getRoot}
        />
      )}
    </div>
  );
}

function PreviewButton({
  label,
  active,
  onClick,
}: {
  label: string;
  active: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={`rounded-md px-2.5 py-1 text-xs font-medium transition-colors ${
        active
          ? 'bg-primary text-primary-foreground'
          : 'border border-input bg-background text-foreground hover:bg-accent'
      }`}
    >
      {label}
    </button>
  );
}

export default function TourPreviewPage() {
  return (
    <Suspense fallback={<div className="p-8 text-muted-foreground">Loading tour preview…</div>}>
      <TourPreview />
    </Suspense>
  );
}
