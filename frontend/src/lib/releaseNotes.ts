/**
 * What changed in each release, shipped INSIDE the app.
 *
 * Local-first is the product (CLAUDE.md §0.1), so this is a bundled constant,
 * not a fetch. A user with no network still learns what changed — and the notes
 * cannot be rewritten after the fact for a build already on someone's machine.
 *
 * ## The rule for writing an entry
 *
 * Say what a user can now DO, and say what is still not true. This app ships
 * capabilities that are deliberately off or deliberately unverified, and a
 * release note that lists only the good half is the same broken promise the
 * product spends its architecture avoiding. `caveats` is not optional
 * decoration: if a release has nothing to qualify, say so explicitly by
 * leaving it empty rather than by forgetting it.
 *
 * Newest first. `version` must match `package.json` exactly — the release
 * workflow already checks the four version sources agree, and
 * `whatsNewFor()` keys off that string.
 */

export interface ReleaseChange {
  /** One short line: what the user can now do, or what stopped being broken. */
  title: string;
  /** One or two sentences. No marketing, no feature names the UI does not use. */
  detail: string;
}

export interface ReleaseNote {
  /** Exactly the `package.json` version this shipped as. */
  version: string;
  /** ISO date, for the heading. */
  date: string;
  /** One sentence a user reads in two seconds. */
  headline: string;
  changes: ReleaseChange[];
  /**
   * What is still off, still unverified, or still not included. Shown in the
   * same dialog, not hidden behind a link.
   */
  caveats: string[];
}

export const RELEASE_NOTES: ReleaseNote[] = [
  {
    version: '1.2.1',
    date: '2026-09-14',
    headline: 'Recording starts with the engine you actually chose, and the live copilot is reachable.',
    changes: [
      {
        title: 'Recording no longer refuses to start for Local Whisper users',
        detail:
          'The app checked for a Parakeet model before every recording, whatever engine you had selected. If you use Local Whisper and have no Parakeet model, recording was refused with "Transcription model not ready" even though your model was fine. It now asks about the engine you configured.',
      },
      {
        title: 'A recording that does not start now says why',
        detail:
          'Several paths returned quietly to idle — the button appeared to do nothing. Every one of them now names the reason, and errors no longer tell you to check a console you cannot open.',
      },
      {
        title: 'Live copilot settings are reachable',
        detail:
          'The switch and the meeting-modes editor were built into a settings screen that was never rendered, so the whole feature was invisible. They are now in Settings → Beta.',
      },
      {
        title: 'Pin an answer to your notes',
        detail:
          'During a recording you can keep a copilot answer. It is written into the meeting summary as a draft when you save the meeting, and it survives regenerating the summary.',
      },
      {
        title: 'This dialog',
        detail: 'Mityu now tells you what changed after an update. Re-open it any time from Settings → General.',
      },
    ],
    caveats: [
      'The live copilot is OFF by default. Turn it on in Settings → Beta, then press Ctrl+Shift+M during a recording.',
      'A pinned note is kept in memory until you save the meeting. If you abandon a recording without saving, its pins are lost.',
      'Transcription accuracy has not been benchmarked against a target environment. Review anything important against the audio.',
      'The copilot has not been exercised against a real cloud model provider. What a model returns for these prompts is unverified.',
    ],
  },
  {
    version: '1.2.0',
    date: '2026-09-14',
    headline: 'The live copilot arrives: a panel that answers from the last few minutes of the conversation.',
    changes: [
      {
        title: 'Live copilot panel',
        detail:
          'An always-on-top panel that follows a recording you start and answers from what was just said. Every answer is tied to the transcript segment it came from.',
      },
      {
        title: 'Meeting modes',
        detail:
          'Eight built-in modes decide what kind of conversation the copilot thinks it is in, which actions it offers, and how strictly it must cite.',
      },
      {
        title: 'Three security fixes before release',
        detail:
          'A remote Ollama endpoint could bypass the "keep insights on this device" setting; shared mode files could carry unbounded text into the prompt; redaction failed open when its policy could not be read.',
      },
    ],
    caveats: [
      'In this build the copilot settings were unreachable. Fixed in 1.2.1.',
      'The copilot is off by default and makes no accuracy claim.',
    ],
  },
];

/** Compare two `major.minor.patch` strings. Unparseable parts sort as 0. */
export function compareVersions(a: string, b: string): number {
  const parse = (v: string) =>
    v
      .split('.')
      .slice(0, 3)
      .map((part) => Number.parseInt(part, 10) || 0);
  const [a1 = 0, a2 = 0, a3 = 0] = parse(a);
  const [b1 = 0, b2 = 0, b3 = 0] = parse(b);
  return a1 - b1 || a2 - b2 || a3 - b3;
}

/**
 * The notes to show when moving from `lastSeen` to `current`.
 *
 * - `lastSeen` null means a FRESH INSTALL: nothing is "new" to someone who has
 *   never run the app, and the product tour is what greets them. Returns empty.
 * - Skipping versions shows every note in between, newest first — a user who
 *   updates 1.2.0 → 1.4.0 should not have to guess what 1.3.0 did.
 * - A downgrade, or a version with no note, returns empty rather than
 *   inventing something.
 */
export function whatsNewFor(current: string, lastSeen: string | null): ReleaseNote[] {
  if (!lastSeen) return [];
  if (compareVersions(current, lastSeen) <= 0) return [];
  return RELEASE_NOTES.filter(
    (note) =>
      compareVersions(note.version, lastSeen) > 0 && compareVersions(note.version, current) <= 0
  ).sort((a, b) => compareVersions(b.version, a.version));
}

/** The note for one version, for re-opening the dialog on demand. */
export function releaseNoteFor(version: string): ReleaseNote | undefined {
  return RELEASE_NOTES.find((note) => note.version === version);
}
