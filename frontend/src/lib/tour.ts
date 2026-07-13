/**
 * First-run product-tour model (Tauri-free, no React).
 *
 * This is the single source of truth for:
 *  - the pre-seeded sample meeting the tour teaches on (`SAMPLE_MEETING_ID`),
 *  - the stable `data-tour` anchor names attached to REAL product elements,
 *  - the exact tour copy (welcome overlay + the three coach-mark steps),
 *  - the `localStorage` completion flag helpers.
 *
 * Local-first: nothing here performs any I/O or network call. The flag lives in
 * `localStorage`; the sample-meeting existence check is a plain local `invoke`
 * done by the provider (see TourProvider), never a remote request.
 */

/** Stable id of the seeded sample meeting (Rust `SAMPLE_MEETING_ID`). */
export const SAMPLE_MEETING_ID = 'meeting-sample-0001';

/** In-app route that opens the sample meeting (query-string form used elsewhere). */
export const sampleMeetingHref = `/meeting-details?id=${SAMPLE_MEETING_ID}`;

/**
 * `localStorage` key that records the tour as finished/skipped so it never nags.
 * Set on: welcome skip/dismiss, tour skip, or tour finish.
 */
export const TOUR_COMPLETED_STORAGE_KEY = 'mityu.onboardingTourCompleted';

/**
 * `data-tour` attribute values. These MUST match the attributes rendered on the
 * real product elements (transcript panel, first summary block, sidebar record
 * button). Both sides import from here so a rename can never silently break the
 * selector.
 */
export const TOUR_ANCHORS = {
  transcriptPanel: 'transcript-panel',
  summaryApproveBlock: 'summary-approve-block',
  recordButton: 'record-button',
} as const;

export type TourAnchor = (typeof TOUR_ANCHORS)[keyof typeof TOUR_ANCHORS];

/** Builds the attribute-selector for a `data-tour` anchor. */
export function tourAnchorSelector(anchor: TourAnchor | string): string {
  return `[data-tour="${anchor}"]`;
}

export type TourPlacement = 'top' | 'bottom' | 'left' | 'right';

export interface TourStepContent {
  /** Stable step id (analytics / keys). */
  id: string;
  /** Which real element this step points at. */
  anchor: TourAnchor;
  /** Short lead line, rendered as the popover heading. */
  title: string;
  /** Supporting sentence. */
  body: string;
  /** Preferred popover side; the renderer falls back to whatever fits. */
  preferredPlacement: TourPlacement;
}

/** Copy for the one-time welcome overlay (verbatim product copy). */
export const WELCOME_COPY = {
  title: 'This is what Mityu does.',
  body:
    "Here's a sample meeting we prepared. Mityu turned a recording into a searchable transcript and a source-linked, human-approved summary — all on your device. Take a quick tour, or jump straight into your first recording.",
  primary: 'Take a 30-second tour',
  secondary: 'Skip — start my own recording',
  footer: 'You can replay this anytime from Settings.',
} as const;

/**
 * The tour is EXACTLY three steps. Titles + bodies are the product copy split at
 * sentence boundaries so the wording stays verbatim while reading well in a small
 * popover. The tour only explains the human-approval model — it never approves
 * anything on the user's behalf (HITL, CLAUDE.md §0.5).
 */
export const TOUR_STEPS: TourStepContent[] = [
  {
    id: 'transcript',
    anchor: TOUR_ANCHORS.transcriptPanel,
    title: 'Everything starts with the transcript.',
    body: 'Every word is captured locally and timestamped.',
    preferredPlacement: 'right',
  },
  {
    id: 'summary',
    anchor: TOUR_ANCHORS.summaryApproveBlock,
    title: 'Mityu drafts a summary, but you stay in control.',
    body: 'Nothing is final until you approve it — and every point links back to the exact moment it came from.',
    preferredPlacement: 'left',
  },
  {
    id: 'record',
    anchor: TOUR_ANCHORS.recordButton,
    title: "When you're ready, start your own recording here.",
    body: "That's it.",
    preferredPlacement: 'right',
  },
];

/** True when the tour has been finished or skipped on this device. */
export function isTourCompleted(): boolean {
  if (typeof window === 'undefined') return false;
  try {
    return window.localStorage.getItem(TOUR_COMPLETED_STORAGE_KEY) === '1';
  } catch {
    return false;
  }
}

/** Marks the tour finished/skipped so the welcome overlay never auto-shows again. */
export function markTourCompleted(): void {
  if (typeof window === 'undefined') return;
  try {
    window.localStorage.setItem(TOUR_COMPLETED_STORAGE_KEY, '1');
  } catch {
    /* private-mode / disabled storage: fail quiet, tour simply may re-show */
  }
}

/** Clears the flag (used by Settings → "Replay product tour"). */
export function clearTourCompleted(): void {
  if (typeof window === 'undefined') return;
  try {
    window.localStorage.removeItem(TOUR_COMPLETED_STORAGE_KEY);
  } catch {
    /* ignore */
  }
}
