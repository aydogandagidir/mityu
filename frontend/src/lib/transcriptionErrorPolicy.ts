/**
 * What a `transcription-error` event means for the recording UI.
 *
 * The Rust worker emits this event in two situations that look alike on the wire and
 * are nothing alike for the user:
 *
 * - `actionable: true` -- the speech engine could not be initialised when the
 *   recording started (no model, wrong provider). Nothing will be transcribed for this
 *   recording, and the user has to change something in Settings.
 * - `actionable: false` -- ONE chunk failed to decode, mid-recording. Rust logs it and
 *   carries on with the next chunk; capture never stopped.
 *
 * Until v1.2.3 `RecordingControls` treated both the same way and ended the recording
 * in the UI: `isRecording` false, status ERROR, "Recording stop did not complete" --
 * while the dock still said "Recording" and the microphone was still open. Twenty
 * minutes into a meeting, one failed chunk made the app contradict itself.
 */

export interface TranscriptionErrorPayload {
  error?: string;
  userMessage?: string;
  actionable?: boolean;
}

export interface ParsedTranscriptionError {
  /** The sentence to show, if anything is shown. */
  message: string;
  /** Whether the user must act (engine could not start) or this was one bad chunk. */
  actionable: boolean;
}

export function parseTranscriptionError(payload: unknown): ParsedTranscriptionError {
  if (typeof payload === 'object' && payload !== null) {
    const p = payload as TranscriptionErrorPayload;
    return {
      message: p.userMessage || p.error || 'Transcription failed.',
      actionable: p.actionable === true,
    };
  }
  return { message: String(payload), actionable: false };
}

/**
 * Only an engine that could not start ends the recording in the UI. A single failed
 * chunk is counted and surfaced by the global toast, and the recording carries on --
 * because natively, it does.
 */
export function endsRecordingUi(payload: unknown): boolean {
  return parseTranscriptionError(payload).actionable;
}
