'use client';

/**
 * The session dock — DESIGN_SYSTEM.md §5.8, §6.2.
 *
 * A 40px bar pinned to the bottom of the shell for as long as a recording is live. It
 * exists because the Stop control used to live on one route: start a recording, open
 * Settings, and the only way to stop was to navigate back. The dock is the shell's
 * indicator; `RecordingStatusBar` stays the in-column marker on the transcript panel and
 * the two never occupy the same 100px (the dock is `fixed` to the shell's bottom edge and
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
 */

import React, { useEffect, useState } from 'react';
import { Square } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { cn } from '@/lib/utils';

export interface SessionDockProps {
  isRecording: boolean;
  isPaused: boolean;
  /** Backend-synced active seconds (pauses excluded). */
  activeDuration: number | null;
  /** True once Stop has been pressed and the wrap-up is draining. */
  isStopping?: boolean;
  onStop: () => void;
  /** 32px instead of 40px, for the narrow copilot-style windows (§5.8). */
  compact?: boolean;
}

function formatDuration(seconds: number): string {
  const mins = Math.floor(seconds / 60);
  const secs = seconds % 60;
  return `${mins.toString().padStart(2, '0')}:${secs.toString().padStart(2, '0')}`;
}

export function SessionDock({
  isRecording,
  isPaused,
  activeDuration,
  isStopping = false,
  onStop,
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

      <span className="ml-auto flex items-center gap-2">
        <span className="hidden text-caption text-muted-foreground sm:inline">
          Everything stays on this device
        </span>
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
