'use client';

/**
 * What happens after Stop — DESIGN_SYSTEM.md §6.2.
 *
 * Stopping a recording takes up to sixty-five seconds: the transcription queue drains,
 * the buffer flushes, the meeting is written. Through all of it the user saw a small
 * overlay reading "Finalizing transcription…" and then, without asking, the app
 * navigated them to a report.
 *
 * Meanwhile `RecordingStateContext.statusMessage` was being written in four places
 * — "Waiting for transcription…", "Processing N remaining chunks…", "Flushing transcript
 * buffer…", "Saving meeting to database…" — and rendered NOWHERE. The pipeline below is
 * that message, finally shown, with the steps around it so a person can see which part
 * is slow rather than watching one spinner for a minute.
 *
 * The developer-facing wording is translated at the boundary: a queue depth is a real
 * thing to report, "chunks" is not a word this audience uses.
 */

import React from 'react';
import { Check, Loader2, Mic } from 'lucide-react';
import { RecordingStatus } from '@/contexts/RecordingStateContext';
import { Button } from '@/components/ui/button';
import { Card } from '@/components/ui/card';
import { cn } from '@/lib/utils';

export interface WrapUpPipelineProps {
  status: RecordingStatus;
  /** 🔒 Written by `useRecordingStop`; shown here for the first time. */
  statusMessage?: string;
  segmentCount: number;
  /** Present once the meeting is saved. */
  meetingId?: string | null;
  onOpenReport?: () => void;
  onRecordAnother?: () => void;
}

type StepState = 'pending' | 'active' | 'done';

function Step({
  state,
  label,
  detail,
}: {
  state: StepState;
  label: string;
  detail?: string;
}) {
  return (
    <li className="flex items-start gap-3">
      <span
        className={cn(
          'mt-0.5 grid size-5 shrink-0 place-items-center rounded-full border',
          state === 'done' && 'border-success bg-success text-success-foreground',
          state === 'active' && 'border-primary text-primary',
          state === 'pending' && 'border-border text-subtle-foreground'
        )}
        aria-hidden="true"
      >
        {state === 'done' ? (
          <Check className="size-3" />
        ) : state === 'active' ? (
          <Loader2 className="size-3 motion-safe:animate-spin" />
        ) : null}
      </span>
      <span className="min-w-0">
        <span
          className={cn(
            'block text-body',
            state === 'pending' ? 'text-subtle-foreground' : 'text-foreground'
          )}
        >
          {label}
        </span>
        {detail ? (
          <span className="block text-caption text-muted-foreground">{detail}</span>
        ) : null}
      </span>
    </li>
  );
}

/** Developer status → something a person can act on. */
function humanise(message?: string): string | undefined {
  if (!message) return undefined;
  const chunks = message.match(/Processing (\d+) remaining chunks/i);
  if (chunks) {
    const n = Number(chunks[1]);
    return `${n} ${n === 1 ? 'piece' : 'pieces'} of audio still being transcribed`;
  }
  if (/Flushing transcript buffer/i.test(message)) return 'Collecting the last of the text';
  if (/Waiting for transcription/i.test(message)) return 'Waiting for the transcriber to finish';
  if (/Saving meeting to database/i.test(message)) return 'Writing the meeting to this device';
  if (/Stopping recording/i.test(message)) return 'Closing the audio stream';
  return message;
}

export function WrapUpPipeline({
  status,
  statusMessage,
  segmentCount,
  meetingId,
  onOpenReport,
  onRecordAnother,
}: WrapUpPipelineProps) {
  const failed = status === RecordingStatus.ERROR;
  const done = status === RecordingStatus.COMPLETED;
  const saving = status === RecordingStatus.SAVING;
  const processing = status === RecordingStatus.PROCESSING_TRANSCRIPTS;
  const stopping = status === RecordingStatus.STOPPING;

  /**
   * On failure the component does NOT know which step broke: `useRecordingStop` sets
   * ERROR from four different places and all it hands over is the message. Guessing
   * would draw a red mark next to work that actually succeeded, so the failed case
   * drops the step list and shows the message instead (below).
   */
  const stepState = (mine: 'stop' | 'transcribe' | 'save'): StepState => {
    if (mine === 'stop') return stopping ? 'active' : 'done';
    if (mine === 'transcribe') {
      if (stopping) return 'pending';
      return processing ? 'active' : 'done';
    }
    if (done) return 'done';
    return saving ? 'active' : 'pending';
  };

  return (
    <div className="mx-auto w-full max-w-md p-gutter">
      <Card className="space-y-4 p-5" role="status" aria-live="polite">
        <div className="space-y-1">
          <h2 className="text-title text-foreground">
            {failed
              ? 'The recording was kept, but saving did not finish'
              : done
                ? 'Meeting saved'
                : 'Wrapping up'}
          </h2>
          <p className="text-body text-muted-foreground">
            {failed
              ? 'Your audio and the text captured so far are still on this device. You can recover this meeting from Home.'
              : done
                ? `${segmentCount} transcript ${segmentCount === 1 ? 'segment' : 'segments'} saved.`
                : 'Everything stays on this device. Leaving this screen will not stop it.'}
          </p>
        </div>

        {failed ? (
          <p className="rounded-md border border-destructive/40 bg-destructive/10 p-3 text-caption text-foreground">
            <span className="font-medium">What went wrong: </span>
            {humanise(statusMessage) || 'The app did not say what failed.'}
          </p>
        ) : (
          <ol className="space-y-2">
            <Step state={stepState('stop')} label="Closing the recording" />
            <Step
              state={stepState('transcribe')}
              label="Finalizing transcript"
              detail={processing ? humanise(statusMessage) : undefined}
            />
            <Step
              state={stepState('save')}
              label="Saving meeting"
              detail={saving ? humanise(statusMessage) : undefined}
            />
          </ol>
        )}

        {(done || failed) && (
          <div className="flex flex-wrap gap-2 pt-1">
            {done && meetingId && onOpenReport ? (
              <Button onClick={onOpenReport}>Open report</Button>
            ) : null}
            {onRecordAnother ? (
              <Button variant="outline" onClick={onRecordAnother}>
                <Mic className="size-4" aria-hidden="true" />
                Record another
              </Button>
            ) : null}
          </div>
        )}
      </Card>
    </div>
  );
}

export default WrapUpPipeline;
