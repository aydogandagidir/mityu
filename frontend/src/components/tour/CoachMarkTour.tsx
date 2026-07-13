'use client';

import { useCallback, useLayoutEffect, useRef, useState } from 'react';
import { ChevronLeft, ChevronRight, Mic, X } from 'lucide-react';
import { Button } from '@/components/ui/button';
import {
  tourAnchorSelector,
  type TourPlacement,
  type TourStepContent,
} from '@/lib/tour';
import { useTourTarget, type TargetRect } from './useTourTarget';

interface CoachMarkTourProps {
  steps: TourStepContent[];
  stepIndex: number;
  onBack: () => void;
  onNext: () => void;
  /** "Skip tour" — abandons the tour and marks it complete. */
  onSkip: () => void;
  /** Last-step primary ("Start recording"). */
  onFinish: () => void;
  /** Scope the target lookup (dev-preview scopes to its own mock container). */
  getRoot?: () => ParentNode;
}

const POPOVER_WIDTH = 320; // matches w-80
const GAP = 14; // target ↔ popover
const MARGIN = 12; // popover ↔ viewport edge
const SPOTLIGHT_PAD = 8; // halo around the target
const DIM = 'rgba(2, 6, 23, 0.60)';

interface Position {
  top: number;
  left: number;
  side: TourPlacement;
}

/** Picks the side with room (preferred first) and clamps inside the viewport. */
function computePosition(
  target: TargetRect,
  popW: number,
  popH: number,
  vw: number,
  vh: number,
  preferred: TourPlacement,
): Position {
  const space: Record<TourPlacement, number> = {
    top: target.top,
    bottom: vh - (target.top + target.height),
    left: target.left,
    right: vw - (target.left + target.width),
  };
  const fits = (s: TourPlacement) =>
    s === 'top' || s === 'bottom'
      ? space[s] >= popH + GAP + MARGIN
      : space[s] >= popW + GAP + MARGIN;

  const order: TourPlacement[] = [preferred, 'bottom', 'top', 'right', 'left'];
  let side = order.find(fits);
  if (!side) {
    side = (['bottom', 'top', 'right', 'left'] as TourPlacement[]).sort(
      (a, b) => space[b] - space[a],
    )[0];
  }

  const cx = target.left + target.width / 2;
  const cy = target.top + target.height / 2;
  let top = 0;
  let left = 0;
  if (side === 'bottom') {
    top = target.top + target.height + GAP;
    left = cx - popW / 2;
  } else if (side === 'top') {
    top = target.top - GAP - popH;
    left = cx - popW / 2;
  } else if (side === 'right') {
    left = target.left + target.width + GAP;
    top = cy - popH / 2;
  } else {
    left = target.left - GAP - popW;
    top = cy - popH / 2;
  }

  left = Math.min(Math.max(left, MARGIN), Math.max(MARGIN, vw - popW - MARGIN));
  top = Math.min(Math.max(top, MARGIN), Math.max(MARGIN, vh - popH - MARGIN));
  return { top, left, side };
}

/**
 * A single coach-mark. When the current step's target is on screen it dims the
 * page, cuts a spotlight around the target, and floats the popover next to it.
 * When the target is hidden or not yet mounted (a collapsed summary, a tab that
 * isn't active, a draft still loading) the step is NOT skipped: the popover is
 * shown centered over a plain dim instead, so every step stays reachable and
 * Back/Next always work. `useTourTarget` keeps polling, so the spotlight snaps
 * in the moment the target becomes visible.
 */
export function CoachMarkTour({
  steps,
  stepIndex,
  onBack,
  onNext,
  onSkip,
  onFinish,
  getRoot,
}: CoachMarkTourProps) {
  const step = steps[stepIndex];
  const active = !!step;

  // Keep the root resolver stable so useTourTarget's effect doesn't restart.
  const getRootRef = useRef<() => ParentNode>(getRoot ?? (() => document));
  getRootRef.current = getRoot ?? (() => document);
  const rootFn = useCallback(() => getRootRef.current(), []);

  const rect = useTourTarget(
    step ? tourAnchorSelector(step.anchor) : '',
    active,
    rootFn,
  );

  const popRef = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState<Position | null>(null);

  useLayoutEffect(() => {
    if (!rect || !popRef.current) {
      setPos(null);
      return;
    }
    const pop = popRef.current.getBoundingClientRect();
    const next = computePosition(
      rect,
      pop.width || POPOVER_WIDTH,
      pop.height || 168,
      window.innerWidth,
      window.innerHeight,
      step.preferredPlacement,
    );
    setPos((prev) =>
      prev &&
      prev.top === next.top &&
      prev.left === next.left &&
      prev.side === next.side
        ? prev
        : next,
    );
  }, [rect, step]);

  if (!active) return null;

  const isLast = stepIndex === steps.length - 1;
  const total = steps.length;

  // Positioned next to the target when we have its rect + a computed position;
  // hidden for the one frame we're measuring; centered when there's no target.
  const popStyle: React.CSSProperties = rect
    ? pos
      ? { top: pos.top, left: pos.left, opacity: 1 }
      : { top: -9999, left: -9999, opacity: 0 }
    : { top: '50%', left: '50%', transform: 'translate(-50%, -50%)', opacity: 1 };

  return (
    <div
      className="fixed inset-0 z-[95]"
      role="dialog"
      aria-modal="true"
      aria-label={`Product tour, step ${stepIndex + 1} of ${total}`}
    >
      {/* Transparent click-swallower: keeps the page non-interactive during the
          tour (the popover has the only controls). */}
      <div className="absolute inset-0" aria-hidden="true" />

      {rect ? (
        /* Spotlight: transparent box whose huge box-shadow dims everything else. */
        <div
          className="absolute rounded-xl ring-2 ring-primary pointer-events-none"
          style={{
            top: rect.top - SPOTLIGHT_PAD,
            left: rect.left - SPOTLIGHT_PAD,
            width: rect.width + SPOTLIGHT_PAD * 2,
            height: rect.height + SPOTLIGHT_PAD * 2,
            boxShadow: `0 0 0 9999px ${DIM}`,
          }}
          aria-hidden="true"
        />
      ) : (
        /* No target to spotlight — dim the whole screen so the centered step reads. */
        <div
          className="absolute inset-0"
          style={{ background: DIM }}
          aria-hidden="true"
        />
      )}

      {/* Popover */}
      <div
        ref={popRef}
        className="absolute w-80 max-w-[calc(100vw-24px)] rounded-xl border border-border bg-popover text-popover-foreground shadow-2xl p-4 pointer-events-auto animate-in fade-in-0 zoom-in-95 duration-150"
        style={popStyle}
      >
        <div className="flex items-start justify-between gap-2">
          <span className="text-xs font-medium tabular-nums text-muted-foreground">
            Step {stepIndex + 1} of {total}
          </span>
          <button
            type="button"
            onClick={onSkip}
            className="-mr-1 -mt-1 rounded-md p-1 text-muted-foreground transition-colors hover:text-foreground"
            aria-label="Skip tour"
          >
            <X className="h-4 w-4" />
          </button>
        </div>

        <h3 className="mt-1.5 text-sm font-semibold text-foreground">
          {step.title}
        </h3>
        <p className="mt-1 text-sm leading-relaxed text-muted-foreground">
          {step.body}
        </p>

        <div className="mt-4 flex items-center justify-between gap-2">
          <button
            type="button"
            onClick={onSkip}
            className="text-xs text-muted-foreground transition-colors hover:text-foreground"
          >
            Skip tour
          </button>
          <div className="flex items-center gap-2">
            {stepIndex > 0 && (
              <Button variant="outline" size="sm" onClick={onBack}>
                <ChevronLeft className="h-4 w-4" />
                Back
              </Button>
            )}
            {isLast ? (
              <Button size="sm" onClick={onFinish}>
                <Mic className="h-4 w-4" />
                Start recording
              </Button>
            ) : (
              <Button size="sm" onClick={onNext}>
                Next
                <ChevronRight className="h-4 w-4" />
              </Button>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}

export default CoachMarkTour;
