'use client';

/**
 * The live copilot panel (BACKLOG I1, ADR-0038).
 *
 * **This is the shell, and it says so.** I1 gives the copilot a window, a
 * global shortcut and an honest screen-sharing posture; it has no AI in it at
 * all. The panel mirrors the live transcript of a recording the user already
 * started and nothing else — no suggestion, no answer, no model call. That is
 * why there is no "AI-generated" marking here yet: there is no AI output to
 * mark, and a transparency label on a feature that does not exist would be as
 * misleading as a missing one. It arrives with the insights, in I3.
 *
 * Two invariants are visible in this file:
 *
 * - **The copilot never captures.** There is no start-recording control. The
 *   panel subscribes to `transcript-update` — the stream of a session whose
 *   consent ticket was already consumed in Rust — and shows an explicit empty
 *   state when no recording is running.
 * - **Private, not covert.** The header states what the OS will really do about
 *   screen sharing, in the backend's own words (`policy.rs`), including "not
 *   hidden" on Linux and "best effort" on macOS 15+. It never claims the panel
 *   is invisible.
 *
 * Rendered by `app/copilot/page.tsx` in the panel window, and by
 * `app/design/copilot/page.tsx` with fixtures so it can be reviewed in a
 * browser.
 */

import { useMemo } from 'react';
import { Eye, EyeOff, Mic, Monitor, Pause, Play, ShieldCheck, ShieldAlert, X } from 'lucide-react';
import type { CopilotStatus } from '@/types/copilot';
import type { TranscriptUpdate } from '@/types';

/** One line in the panel's transcript tail. */
export interface PanelLine {
  id: number;
  /** Who was speaking, as far as the audio pipeline knows. */
  side: 'me' | 'them';
  text: string;
  timestamp: string;
}

/** How many final segments the tail keeps. Enough for context, short enough to read. */
export const TAIL_LENGTH = 6;

/**
 * Which channel a transcript segment came from.
 *
 * This is the one attribution the copilot can make honestly today: the audio
 * pipeline already separates the microphone from system audio
 * (`TranscriptUpdate.source`), so "me" and "them" are a fact about which device
 * captured the audio — not a guess about a voice. Nothing here infers identity
 * from how someone sounds; that would be biometric categorisation, which
 * ADR-0034 keeps out of the product. Diarization stays post-hoc and anonymous.
 */
export function sideForSource(source: string): 'me' | 'them' {
  return source.toLowerCase().includes('mic') ? 'me' : 'them';
}

/** Append a final segment to the tail, keeping it bounded. */
export function appendLine(lines: PanelLine[], update: TranscriptUpdate): PanelLine[] {
  // Partials are a preview of text that is about to change; letting them into
  // the tail makes lines rewrite themselves as the user reads.
  if (update.is_partial) return lines;
  const text = update.text?.trim();
  if (!text) return lines;
  const next: PanelLine = {
    id: update.sequence_id,
    side: sideForSource(update.source ?? ''),
    text,
    timestamp: update.timestamp,
  };
  return [...lines.filter((line) => line.id !== next.id), next].slice(-TAIL_LENGTH);
}

function ProtectionChip({ status }: { status: CopilotStatus }) {
  const { level, headline } = status.protection;
  const Icon = level === 'enforced' ? ShieldCheck : level === 'bestEffort' ? ShieldAlert : EyeOff;
  const tone =
    level === 'enforced'
      ? 'text-emerald-600 dark:text-emerald-400'
      : level === 'bestEffort'
        ? 'text-amber-600 dark:text-amber-400'
        : 'text-muted-foreground';

  return (
    <span
      className={`inline-flex items-center gap-1 text-[11px] ${tone}`}
      title={status.protection.detail}
    >
      <Icon className="h-3 w-3 shrink-0" />
      <span className="truncate">{headline}</span>
    </span>
  );
}

export interface CopilotPanelProps {
  status: CopilotStatus | null;
  lines: PanelLine[];
  /** Undefined in the design fixture, where there is no backend to call. */
  onClose?: () => void;
  onPause?: () => void;
  onResume?: () => void;
  onOpenMainWindow?: () => void;
  paused?: boolean;
  error?: string | null;
}

export function CopilotPanel({
  status,
  lines,
  onClose,
  onPause,
  onResume,
  onOpenMainWindow,
  paused = false,
  error = null,
}: CopilotPanelProps) {
  const recording = status?.recording ?? false;
  const toggleShortcut = useMemo(
    () => status?.shortcuts.find((s) => s.action === 'togglePanel'),
    [status]
  );

  return (
    <div className="flex h-full w-full flex-col overflow-hidden bg-card text-card-foreground">
      {/* The whole header is the drag handle: the window is frameless, so
          without this the panel could not be moved. */}
      <header
        data-tauri-drag-region
        className="flex shrink-0 items-center gap-2 border-b border-border px-3 py-2"
      >
        <div data-tauri-drag-region className="min-w-0 flex-1">
          <div data-tauri-drag-region className="text-[13px] font-semibold leading-tight">
            Mityu copilot
          </div>
          {status && <ProtectionChip status={status} />}
        </div>
        {onClose && (
          <button
            type="button"
            onClick={onClose}
            aria-label="Close the copilot panel"
            className="rounded-md p-1 text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
          >
            <X className="h-4 w-4" />
          </button>
        )}
      </header>

      <div className="flex min-h-0 flex-1 flex-col">
        {error && (
          <p role="alert" className="border-b border-border px-3 py-2 text-[12px] text-destructive">
            {error}
          </p>
        )}

        {recording ? (
          <>
            <div className="flex shrink-0 flex-wrap items-center gap-x-2 gap-y-1 px-3 py-2">
              <span className="inline-flex items-center gap-1.5 text-[11px] font-medium text-foreground">
                <span
                  className={`h-2 w-2 rounded-full ${paused ? 'bg-amber-500' : 'bg-red-500'}`}
                  aria-hidden
                />
                {paused ? 'Recording paused' : 'Recording'}
              </span>
              <div className="ml-auto flex flex-wrap items-center gap-1">
                {paused
                  ? onResume && (
                      <button
                        type="button"
                        onClick={onResume}
                        className="inline-flex items-center gap-1 rounded-md border border-border px-2 py-1 text-[11px] transition-colors hover:bg-muted"
                      >
                        <Play className="h-3 w-3" /> Resume
                      </button>
                    )
                  : onPause && (
                      <button
                        type="button"
                        onClick={onPause}
                        className="inline-flex items-center gap-1 rounded-md border border-border px-2 py-1 text-[11px] transition-colors hover:bg-muted"
                      >
                        <Pause className="h-3 w-3" /> Pause
                      </button>
                    )}
                {/* Stopping is deliberately not duplicated here — see the note
                    in `app/copilot/page.tsx`. */}
                {onOpenMainWindow && (
                  <button
                    type="button"
                    onClick={onOpenMainWindow}
                    className="inline-flex items-center gap-1 rounded-md border border-border px-2 py-1 text-[11px] transition-colors hover:bg-muted"
                  >
                    <Monitor className="h-3 w-3" /> Open Mityu
                  </button>
                )}
              </div>
            </div>

            <ol className="min-h-0 flex-1 space-y-2 overflow-y-auto px-3 pb-3">
              {lines.length === 0 ? (
                <li className="pt-6 text-center text-[12px] text-muted-foreground">
                  Listening. Transcribed speech appears here within a few seconds.
                </li>
              ) : (
                lines.map((line) => (
                  <li key={line.id} className="text-[12px] leading-snug">
                    <span
                      className={`mr-1.5 font-medium ${
                        line.side === 'me' ? 'text-primary' : 'text-muted-foreground'
                      }`}
                    >
                      {line.side === 'me' ? 'You' : 'Them'}
                    </span>
                    <span className="text-foreground">{line.text}</span>
                  </li>
                ))
              )}
            </ol>
          </>
        ) : (
          <div className="flex flex-1 flex-col items-center justify-center gap-2 px-6 text-center">
            <Mic className="h-5 w-5 text-muted-foreground" aria-hidden />
            <p className="text-[12px] text-muted-foreground">
              The copilot follows a recording you start in Mityu. It never records on its own.
            </p>
            {onOpenMainWindow && (
              <button
                type="button"
                onClick={onOpenMainWindow}
                className="mt-1 inline-flex items-center gap-1 rounded-md border border-border px-2 py-1 text-[11px] transition-colors hover:bg-muted"
              >
                <Monitor className="h-3 w-3" /> Open Mityu
              </button>
            )}
          </div>
        )}
      </div>

      <footer className="shrink-0 border-t border-border px-3 py-2">
        <p className="text-[10px] leading-snug text-muted-foreground">
          <Eye className="mr-1 inline h-3 w-3 align-[-2px]" />
          This panel mirrors the live transcript. Suggestions, follow-up questions and recaps
          arrive in a later version.
          {toggleShortcut?.registered && <> Press {toggleShortcut.keybind} to hide it.</>}
        </p>
      </footer>
    </div>
  );
}
