'use client';

/**
 * The session dock — DESIGN_SYSTEM.md §5.8, §6.2.
 *
 * A 40px bar pinned to the bottom of the shell for as long as a recording is live. It
 * exists because the Stop control used to live on one route: start a recording, open
 * Settings, and the only way to stop was to navigate back. The dock is the shell's
 * indicator (the dock is `fixed` to the shell's bottom edge and
 * the shell publishes `--bottom-chrome` so page bodies and toasts clear it).
 *
 * The paused state is a WARNING SQUARE plus the word, never a second dot: under
 * `prefers-reduced-motion` §4.9 removes the pulse from the recording dot, so shape and
 * hue would otherwise be all that separates "recording" from "paused" — and hue alone is
 * not a differentiator (§4.8).
 *
 * Stop is never disabled while recording. A stop can also arrive from the tray or a
 * keyboard shortcut with this button never having been clicked, so the dock reads the
 * backend-synced state rather than assuming it initiated anything.
 *
 * Since v1.2.6 the dock is the ONE live-session control (Granola/Cluely "single source of
 * control"): Pause/Resume and the real capture level moved here from the floating pill on
 * `/`, and the sticky `RecordingStatusBar` above the transcript was removed. Before that a
 * live recording showed two Stop buttons and three clocks at once.
 */

import React, { useEffect, useState } from 'react';
import { Pause, Play, Square } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { cn } from '@/lib/utils';

/** Bar scale for the three-bar meter: centre bar tallest. */
const LEVEL_BAR_SCALES = [0.75, 1, 0.75] as const;
/** Smallest bar height in px, so the meter never disappears. */
const LEVEL_BAR_MIN_PX = 4;
/** Extra height a full-scale reading adds, in px. */
const LEVEL_BAR_RANGE_PX = 14;

export interface SessionDockProps {
  isRecording: boolean;
  isPaused: boolean;
  /** Backend-synced active seconds (pauses excluded). */
  activeDuration: number | null;
  /** True once Stop has been pressed and the wrap-up is draining. */
  isStopping?: boolean;
  onStop: () => void;
  /** Pause when recording, resume when paused. Omitted → no pause control is drawn. */
  onTogglePause?: () => void;
  /** True while a pause/resume round-trip is in flight. */
  isPauseBusy?: boolean;
  /**
   * Loudest RMS across capture devices, 0..1, or `null` when there is no reading yet.
   * Omitted (`undefined`) → no meter is drawn.
   */
  level?: number | null;
  /** 32px instead of 40px, for the narrow copilot-style windows (§5.8). */
  compact?: boolean;
}

function formatDuration(seconds: number): string {
  const mins = Math.floor(seconds / 60);
  const secs = seconds % 60;
  return `${mins.toString().padStart(2, '0')}:${secs.toString().padStart(2, '0')}`;
}

function levelBarHeight(level: number | null, isPaused: boolean, scale: number): number {
  if (isPaused || level === null || level <= 0) return LEVEL_BAR_MIN_PX;
  // A little log shaping so quiet speech still moves the bars.
  const shaped = Math.log10(Math.min(level, 1) * 9 + 1);
  return Math.max(LEVEL_BAR_MIN_PX, Math.round(shaped * LEVEL_BAR_RANGE_PX * scale) + LEVEL_BAR_MIN_PX);
}

export function SessionDock({
  isRecording,
  isPaused,
  activeDuration,
  isStopping = false,
  onStop,
  onTogglePause,
  isPauseBusy = false,
  level,
  compact = false,
}: SessionDockProps) {
  const [displaySeconds, setDisplaySeconds] = useState(0);

  useEffect(() => {
    if (activeDuration !== null) {
      setDisplaySeconds(Math.floor(activeDuration));
    }
  }, [activeDuration]);

  if (!isRecording) return null;

  return (
    <div
      className={cn(
        'flex w-full shrink-0 items-center gap-3 border-t border-border bg-card px-gutter',
        compact ? 'h-8' : 'h-10'
      )}
      role="status"
      aria-live="off"
      aria-label={isPaused ? 'Recording paused' : 'Recording in progress'}
    >
      {isPaused ? (
        <span
          aria-hidden="true"
          className="size-2.5 shrink-0 bg-warning"
          title="Paused"
        />
      ) : (
        <span
          aria-hidden="true"
          className="size-2.5 shrink-0 rounded-full bg-recording motion-safe:animate-pulse"
        />
      )}

      <span
        className={cn(
          'text-label tabular-nums',
          isPaused ? 'text-warning-ink' : 'text-recording-ink'
        )}
      >
        {isPaused ? 'Paused' : 'Recording'} • {formatDuration(displaySeconds)}
      </span>

      {/* The REAL capture level (useAudioLevels), not decoration. `aria-hidden`: the
          sr-only line beside it carries the same fact in words. */}
      {level !== undefined && (
        <>
          <span className="flex h-5 items-center gap-0.5" aria-hidden="true" data-testid="dock-level">
            {LEVEL_BAR_SCALES.map((scale, index) => (
              <span
                key={index}
                className={cn(
                  'w-1 rounded-full transition-[height] duration-fast ease-out motion-reduce:transition-none',
                  isPaused ? 'bg-warning opacity-60' : 'bg-recording'
                )}
                style={{ height: `${levelBarHeight(level, isPaused, scale)}px` }}
              />
            ))}
          </span>
          <span className="sr-only">
            {isPaused
              ? 'Recording paused'
              : level === null
                ? 'No audio level reading yet'
                : level > 0
                  ? 'Audio is being captured'
                  : 'No sound is being picked up'}
          </span>
        </>
      )}

      <span className="ml-auto flex items-center gap-2">
        <span className="hidden text-caption text-muted-foreground sm:inline">
          Everything stays on this device
        </span>
        {onTogglePause && (
          <Button
            variant="outline"
            size="sm"
            onClick={onTogglePause}
            disabled={isPauseBusy || isStopping}
            aria-label={isPaused ? 'Resume recording' : 'Pause recording'}
          >
            {isPaused ? (
              <Play className="size-3 fill-current" aria-hidden="true" />
            ) : (
              <Pause className="size-3 fill-current" aria-hidden="true" />
            )}
            {isPaused ? 'Resume' : 'Pause'}
          </Button>
        )}
        <Button
          variant="record"
          size="sm"
          onClick={onStop}
          aria-label="Stop recording"
        >
          <Square className="size-3 fill-current" aria-hidden="true" />
          {isStopping ? 'Stopping…' : 'Stop'}
        </Button>
      </span>
    </div>
  );
}

export default SessionDock;
