'use client';

/**
 * /design/record — the recording session chrome, Tauri-free (DESIGN_SYSTEM.md §11.1).
 *
 * The real `SessionDock` with injected data, across the states that decide whether the
 * dock is trustworthy: recording, paused, the stop already draining, and the 32px compact
 * variant. The paused row is here specifically to be looked at — §4.9 removes the pulse
 * under reduced motion, so if "paused" and "recording" differed only by hue this frame
 * would be the evidence.
 */

import React from 'react';
import { CheckCircle2, Cpu, Mic, Pause, Square, Volume2 } from 'lucide-react';
import { SessionDock } from '@/components/shell/SessionDock';
import { cn } from '@/lib/utils';
import { focusRing } from '@/components/ui/focus-ring';

function DeviceStrip({ paused = false }: { paused?: boolean }) {
  const barHeights = [6, 12, 16, 10, 14, 8];
  return (
    <div className="flex flex-wrap items-center justify-between gap-3 border-b border-border bg-surface-2/60 px-4 py-2 text-caption tabular-nums text-muted-foreground">
      <div className="flex flex-wrap items-center gap-3">
        <span className="inline-flex items-center gap-1.5 rounded-full border border-border bg-card px-2.5 py-0.5 text-micro font-medium text-foreground">
          <Mic className="h-3 w-3 text-recording" aria-hidden />
          <span>Microphone</span>
          <span className="ml-1 flex items-center gap-0.5" aria-hidden>
            {barHeights.map((h, i) => (
              <span
                key={i}
                className={cn(
                  'w-0.5 rounded-full',
                  paused ? 'bg-warning' : 'bg-meter'
                )}
                style={{ height: `${paused ? 4 : h}px` }}
              />
            ))}
          </span>
        </span>
        <span className="inline-flex items-center gap-1.5 rounded-full border border-border bg-card px-2.5 py-0.5 text-micro font-medium text-foreground">
          <Volume2 className="h-3 w-3 text-primary-ink" aria-hidden />
          <span>System Audio</span>
          <span className="ml-1 flex items-center gap-0.5" aria-hidden>
            {barHeights.map((h, i) => (
              <span
                key={i}
                className={cn(
                  'w-0.5 rounded-full',
                  paused ? 'bg-warning' : 'bg-meter'
                )}
                style={{ height: `${paused ? 4 : Math.max(4, h - 3)}px` }}
              />
            ))}
          </span>
        </span>
      </div>
      <div className="flex items-center gap-2">
        <span className="inline-flex items-center gap-1 rounded-full border border-border bg-card px-2 py-0.5 font-mono text-micro text-foreground">
          <Cpu className="h-3 w-3 text-muted-foreground" aria-hidden />
          48 kHz · Parakeet · Nemotron 3.5
        </span>
        <span className="inline-flex items-center gap-1 rounded-full border border-success-border bg-success-surface px-2 py-0.5 text-micro font-medium text-success-ink">
          <CheckCircle2 className="h-3 w-3" aria-hidden />
          Participants informed · 14:32
        </span>
      </div>
    </div>
  );
}

function Frame({
  caption,
  paused = false,
  children,
}: {
  caption: string;
  paused?: boolean;
  children: React.ReactNode;
}) {
  return (
    <section className="overflow-hidden rounded-xl border border-border bg-card shadow-elev-1">
      <div className="flex items-center justify-between border-b border-border bg-surface-2/40 px-4 py-2">
        <h2 className="text-label font-semibold text-foreground">{caption}</h2>
        <span className="font-mono text-micro tabular-nums text-subtle-foreground">
          Local capture · WASAPI / CoreAudio
        </span>
      </div>
      <DeviceStrip paused={paused} />
      <div className="grid min-h-[120px] place-items-center bg-background p-4 text-caption text-subtle-foreground">
        <div className="flex flex-col items-center gap-2 text-center">
          <div className="flex items-center gap-2 rounded-full border border-border bg-surface-2 px-3 py-1 text-caption font-medium text-foreground">
            <span
              className={cn(
                'size-2 shrink-0',
                paused ? 'bg-warning' : 'rounded-full bg-recording'
              )}
              aria-hidden
            />
            <span>Listening for speech</span>
          </div>
          <p className="text-caption text-muted-foreground">
            Live segments stream into the 720px reading column as speech is detected.
          </p>
        </div>
      </div>
      {children}
    </section>
  );
}

export default function RecordFixture() {
  return (
    <div className="min-h-full space-y-6 bg-background p-gutter" data-fixture="record">
      <header className="space-y-1">
        <div className="text-eyebrow uppercase text-subtle-foreground">CAPTURE CHROME</div>
        <h1 className="text-display text-foreground">Session dock &amp; floating recording bar</h1>
        <p className="max-w-measure text-body text-muted-foreground">
          Always-visible, always-stoppable capture instrumentation with real per-device meters,
          engine telemetry (48 kHz), and tokenized floating pill controls.
        </p>
      </header>

      {/* Floating Recording Pill Bar Specimen */}
      <section className="flex flex-wrap items-center justify-between gap-4 rounded-xl border border-border bg-surface-2 p-4">
        <div className="space-y-0.5">
          <div className="text-label font-semibold text-foreground">
            Floating recording pill bar
          </div>
          <p className="text-caption text-muted-foreground">
            Tokenized `bg-recording text-recording-foreground` controls with `shadow-elev-2`,
            `backdrop-blur-md`, and `tabular-nums` telemetry.
          </p>
        </div>
        <div className="flex items-center gap-2 rounded-full border border-border bg-card/95 px-4 py-2 shadow-elev-2 backdrop-blur-md tabular-nums">
          <button
            type="button"
            aria-label="Pause recording"
            className={cn(
              'flex h-10 w-10 items-center justify-center rounded-full border border-border bg-card text-foreground transition-colors duration-fast ease-out hover:bg-muted',
              focusRing('card')
            )}
          >
            <Pause className="h-4 w-4" />
          </button>
          <button
            type="button"
            aria-label="Stop recording"
            className={cn(
              'flex h-10 w-10 items-center justify-center rounded-full bg-recording text-recording-foreground shadow-elev-1 transition-colors duration-fast ease-out hover:bg-recording-hover active:bg-recording-active',
              focusRing('card')
            )}
          >
            <Square className="h-4 w-4" />
          </button>
          <div className="mx-3 flex items-center gap-1" aria-hidden="true">
            <div className="h-3 w-1 rounded-full bg-recording" />
            <div className="h-5 w-1 rounded-full bg-recording" />
            <div className="h-3.5 w-1 rounded-full bg-recording" />
          </div>
          <span className="font-mono text-caption font-medium tabular-nums text-foreground">
            12:34
          </span>
        </div>
      </section>

      <div className="grid gap-4">
        <Frame caption="Recording — the dock on every route">
          <SessionDock
            isRecording
            isPaused={false}
            activeDuration={754}
            onStop={() => {}}
          />
        </Frame>
        <Frame caption="Paused — a square and the word, never a second dot" paused>
          <SessionDock
            isRecording
            isPaused
            activeDuration={754}
            onStop={() => {}}
          />
        </Frame>
        <Frame caption="Stopping — Stop stays enabled, the label changes · Finalizing transcript">
          <SessionDock
            isRecording
            isPaused={false}
            activeDuration={903}
            isStopping
            onStop={() => {}}
          />
        </Frame>
        <Frame caption="Compact (32px) — narrow windows">
          <SessionDock
            isRecording
            isPaused={false}
            activeDuration={62}
            compact
            onStop={() => {}}
          />
        </Frame>
      </div>
    </div>
  );
}
