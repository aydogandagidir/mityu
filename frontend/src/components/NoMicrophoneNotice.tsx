'use client';

import { MicOff, RefreshCw } from 'lucide-react';

interface NoMicrophoneNoticeProps {
  /** The device query failed outright, rather than returning an empty list. */
  error: string | null;
  onRetry: () => void;
}

/**
 * Shown in the recording controls' place when no input device was found.
 *
 * The home page used to render the controls only when `hasMicrophone` was true,
 * so a user whose microphone was unplugged, disabled in Windows, or blocked by
 * the privacy setting got **an empty space** — no button, no disabled button, no
 * explanation anywhere in the app. "The record button is gone" and "the record
 * button does nothing" are indistinguishable to the person pressing it, and both
 * read as the app being broken.
 *
 * So the slot is never empty any more: either the control, or this, saying which
 * of the two situations it is and what to do about it.
 */
export function NoMicrophoneNotice({ error, onRetry }: NoMicrophoneNoticeProps) {
  return (
    <div
      className="bg-card rounded-2xl shadow-lg px-5 py-4 flex items-start gap-3 max-w-xl"
      role="status"
    >
      <MicOff className="w-5 h-5 mt-0.5 shrink-0 text-muted-foreground" aria-hidden="true" />
      <div className="min-w-0">
        <p className="text-title font-medium">No microphone found</p>
        <p className="text-meta text-muted-foreground mt-1">
          {error
            ? // The backend refused to enumerate devices at all. That is a
              // different fault from "you have no microphone", and saying the
              // wrong one sends the user to the wrong settings screen.
              'Mityu could not read the list of audio devices, so recording is unavailable.'
            : 'Recording needs an input device. Check that your microphone is plugged in and enabled, and that Windows lets desktop apps use it (Settings → Privacy & security → Microphone).'}
        </p>
        {error && (
          <p className="text-meta text-muted-foreground mt-1 break-words">{error}</p>
        )}
        <button
          type="button"
          onClick={onRetry}
          className="mt-3 inline-flex items-center gap-1.5 text-meta font-medium text-primary hover:underline"
        >
          <RefreshCw className="w-3.5 h-3.5" aria-hidden="true" />
          Check again
        </button>
      </div>
    </div>
  );
}
