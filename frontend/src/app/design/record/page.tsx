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
import { SessionDock } from '@/components/shell/SessionDock';

function Frame({ caption, children }: { caption: string; children: React.ReactNode }) {
  return (
    <section className="overflow-hidden rounded-lg border border-border bg-background">
      <h2 className="border-b border-border px-4 py-2 text-caption text-muted-foreground">
        {caption}
      </h2>
      <div className="grid min-h-[120px] place-items-center bg-muted p-4 text-caption text-subtle-foreground">
        Listening for speech
      </div>
      {children}
    </section>
  );
}

export default function RecordFixture() {
  return (
    <div className="min-h-full bg-muted p-gutter" data-fixture="record">
      <h1 className="mb-4 text-title text-foreground">Session dock</h1>
      <div className="grid gap-4">
        <Frame caption="Recording — the dock on every route">
          <SessionDock
            isRecording
            isPaused={false}
            activeDuration={754}
            onStop={() => {}}
          />
        </Frame>
        <Frame caption="Paused — a square and the word, never a second dot">
          <SessionDock
            isRecording
            isPaused
            activeDuration={754}
            onStop={() => {}}
          />
        </Frame>
        <Frame caption="Stopping — Stop stays enabled, the label changes">
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
