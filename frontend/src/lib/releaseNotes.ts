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
    version: '1.2.3',
    date: '2026-09-18',
    headline: 'Recording no longer closes the app on processors without AVX-512.',
    changes: [
      {
        title: 'The app no longer disappears when you press Record',
        detail:
          'On many processors — including most consumer laptops sold in the last few years — pressing Record closed Mityu instantly, with no message and nothing written to the log. The build had been compiled to use an instruction set (AVX-512) that those processors do not have, so the processor refused the instruction and Windows ended the program. Mityu is now built for a processor baseline that every machine from roughly 2013 onwards meets, and that baseline is written down in the project instead of being inherited from whichever machine compiled the release.',
      },
      {
        title: 'If your processor is too old, you are told so instead of losing the app',
        detail:
          'Mityu now checks the processor before it starts recording. On a machine below the supported baseline the Record button is refused with a sentence explaining why, rather than the program closing without warning.',
      },
      {
        title: 'The log now says what your processor supports',
        detail:
          'One line at startup records the processor features Mityu found. If anything like this ever happens again, that line turns a forensic investigation into a single look at the log.',
      },
      {
        title: 'A voice-detection failure can no longer take the app down with it',
        detail:
          'If the voice-activity detector failed to start, the app aborted the thread that was starting your recording — while it was holding audio that cannot be recreated. It now reports the failure and leaves the app running.',
      },
    ],
    caveats: [
      'This was not new in 1.2.2. The same instruction was in 1.2.1 as well; it never fired because 1.2.1 could not start a recording at all. Do not downgrade — 1.2.1 is worse, not safer.',
      'Verified on Windows on an AMD Ryzen 7 7435HS, which is one of the affected processors. macOS remains unverified, as does the Vulkan GPU path.',
      'No automated test runs the built application yet. This release was checked by hand. Adding that gate is the next piece of work, and it is the gate whose absence let this reach users.',
      'One open question remains: instructions above the new baseline were reported inside the speech-recognition runtime we do not compile ourselves. Nothing confirmed or ruled it out, which is why this release was tested by hand on an affected machine rather than declared safe from the build alone.',
    ],
  },
  {
    version: '1.2.2',
    date: '2026-09-18',
    headline: 'The Stop button outside the home screen now actually stops the recording.',
    changes: [
      {
        title: 'Stop works on every screen, not just Home',
        detail:
          'The Stop in the bar at the bottom of the window ended the on-screen session but never told the recorder to stop. On Settings, Actions or a meeting report it looked like it worked — the label changed to "Stopping…" — and then it went back to "Stop" with the recording still running and nothing saved. It now ends the recording and saves the meeting from wherever you are.',
      },
      {
        title: 'Search results are readable in dark mode',
        detail:
          'A result in the meetings search painted itself as a white card inside the dark window. It follows the theme now.',
      },
      {
        title: 'A summary opened in dark mode is no longer a white sheet',
        detail:
          'The summary editor was pinned to the light theme, so opening one in dark mode lit up the whole pane.',
      },
      {
        title: 'Nothing is cut off at the bottom while you record',
        detail:
          'With a recording running, the last 40 pixels of every scrollable screen were hidden behind the session bar.',
      },
      {
        title: 'The recording controls have names a screen reader can read',
        detail:
          'Record, Pause and Stop announced themselves only as "button". They now say which is which.',
      },
    ],
    caveats: [
      'This release was verified on Windows. The macOS path was not exercised for 1.2.2.',
      'Transcription accuracy is still not benchmarked, and Turkish in particular has a known open problem. Review anything important against the audio.',
      'The fixes above were found by running the app by hand. The screenshot checks that run automatically never rendered the affected states, which is why they passed while the bugs shipped.',
    ],
  },
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
