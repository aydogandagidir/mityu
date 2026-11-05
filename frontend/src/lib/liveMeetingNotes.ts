/**
 * Quick notes typed during a live recording — Granola-style hybrid notepad.
 *
 * The notes ride into the summary as its custom context (`customPrompt`), which the
 * summary pipeline already supports. They are held IN MEMORY ONLY, deliberately: no
 * localStorage, sessionStorage or IndexedDB copy, so meeting content never gains a
 * second at-rest home outside the encrypted database (the app purges its plaintext
 * recovery copies for the same reason). A reload drops unsaved notes; that is the
 * trade.
 *
 * Lifecycle: typed while recording (`setLiveNotes`) → bound to the saved meeting when the
 * stop flow learns its id (`attachLiveNotesToMeeting`) → read by that meeting's report
 * (`peekNotesForMeeting`), from whichever route the user opens it. Binding clears the
 * live buffer so the next recording starts empty.
 *
 * 🔒 Never log the text.
 */

let liveText = '';
let bound: { meetingId: string; text: string } | null = null;

export function getLiveNotes(): string {
  return liveText;
}

export function setLiveNotes(text: string): void {
  liveText = text;
}

/** Move the live notes onto a saved meeting. No-op (beyond clearing) when they are empty. */
export function attachLiveNotesToMeeting(meetingId: string): void {
  const text = liveText.trim();
  if (text && meetingId) {
    bound = { meetingId, text };
  }
  liveText = '';
}

/** The notes bound to `meetingId`, or `''`. Reading does not consume them. */
export function peekNotesForMeeting(meetingId: string | null | undefined): string {
  return meetingId && bound?.meetingId === meetingId ? bound.text : '';
}

/** Test seam: back to a fresh state. */
export function resetLiveNotesForTests(): void {
  liveText = '';
  bound = null;
}

