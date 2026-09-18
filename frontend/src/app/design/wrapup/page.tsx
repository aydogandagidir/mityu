'use client';

/**
 * /design/wrapup — what happens after Stop, Tauri-free (DESIGN_SYSTEM.md §11.1).
 *
 * The four states side by side, because the interesting thing about this screen is the
 * transition between them: which step is active, what the status message says while it
 * is, and what is offered once there is nothing left to wait for.
 */

import React from 'react';
import { WrapUpPipeline } from '@/app/_components/WrapUpPipeline';
import { RecordingStatus } from '@/contexts/RecordingStateContext';

const CASES = [
  {
    caption: 'Transcribing — the queue is still draining',
    status: RecordingStatus.PROCESSING_TRANSCRIPTS,
    statusMessage: 'Processing 7 remaining chunks...',
    segmentCount: 41,
  },
  {
    caption: 'Saving — the meeting is being written',
    status: RecordingStatus.SAVING,
    statusMessage: 'Saving meeting to database...',
    segmentCount: 48,
  },
  {
    caption: 'Saved',
    status: RecordingStatus.COMPLETED,
    statusMessage: undefined,
    segmentCount: 48,
    meetingId: 'm-1',
  },
  {
    caption: 'The save failed — the audio is still here',
    status: RecordingStatus.ERROR,
    statusMessage: 'database is locked',
    segmentCount: 48,
  },
] as const;

export default function WrapUpFixture() {
  return (
    <div className="min-h-full bg-muted p-gutter" data-fixture="wrapup">
      <h1 className="mb-4 text-title text-foreground">Wrap-up pipeline</h1>
      <div className="grid gap-4 md:grid-cols-2">
        {CASES.map((c) => (
          <section key={c.caption} className="rounded-lg border border-border bg-background">
            <h2 className="border-b border-border px-4 py-2 text-caption text-muted-foreground">
              {c.caption}
            </h2>
            <WrapUpPipeline
              status={c.status}
              statusMessage={c.statusMessage}
              segmentCount={c.segmentCount}
              meetingId={'meetingId' in c ? c.meetingId : null}
              onOpenReport={() => {}}
              onRecordAnother={() => {}}
            />
          </section>
        ))}
      </div>
    </div>
  );
}
