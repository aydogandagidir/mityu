'use client';

/**
 * Showing who spoke, without saying more than the data supports.
 *
 * The arithmetic lives in `@/lib/speakerTurns` and is tested there. What this
 * file decides is what the screen is allowed to CLAIM, which ADR-0034 bounds:
 *
 * - **Labels stay anonymous.** `Speaker 1`, never a name. Naming a voice is a
 *   manual human act; a stored voice-to-name mapping is biometric data under
 *   KVKK/GDPR, so nothing here offers to do it automatically.
 * - **Talk time is descriptive, never a ranking.** Speakers appear in the order
 *   they first spoke. No bar is "the winner", nothing is sorted by duration, and
 *   the panel says outright that this is not a measure of contribution.
 * - **The share is of SPEECH, not of the meeting** — people talk over each
 *   other, so shares over meeting duration would exceed 100%. The label on
 *   screen says which denominator it is.
 * - **A transcript row can belong to more than one speaker**, and when it does
 *   both are shown rather than the louder one winning.
 */

import { Users } from 'lucide-react';

/**
 * Megabytes actually fetched over the network, not what lands on disk: the
 * segmentation model arrives as a `.tar.bz2` and expands. Pinned on the Rust
 * side by `models::tests::the_download_size_the_ui_promises`, so bumping a
 * model pin fails the build rather than quietly making this button lie.
 */
const MODEL_DOWNLOAD_MB = 35;

import {
  formatTalkTime,
  type SpeakerTalkTime,
  type SpeakerTurn,
  speakerCount,
  talkTime,
} from '@/lib/speakerTurns';

/**
 * Stable, deliberately flat colours: they identify a speaker across the page,
 * they do not rank. No red/green, nothing that reads as good or bad.
 */
const SWATCHES = [
  { dot: 'bg-sky-500', bar: 'bg-sky-500/70', text: 'text-sky-700 dark:text-sky-300' },
  { dot: 'bg-violet-500', bar: 'bg-violet-500/70', text: 'text-violet-700 dark:text-violet-300' },
  { dot: 'bg-amber-500', bar: 'bg-amber-500/70', text: 'text-amber-700 dark:text-amber-300' },
  { dot: 'bg-teal-500', bar: 'bg-teal-500/70', text: 'text-teal-700 dark:text-teal-300' },
  { dot: 'bg-pink-500', bar: 'bg-pink-500/70', text: 'text-pink-700 dark:text-pink-300' },
  { dot: 'bg-lime-600', bar: 'bg-lime-600/70', text: 'text-lime-700 dark:text-lime-300' },
];

/**
 * Colour by position in the speaker list, so a speaker keeps one colour
 * everywhere on the page. Falls back to a neutral swatch past the palette
 * rather than repeating one and implying two speakers are the same.
 */
export function swatchFor(label: string, order: string[]) {
  const i = order.indexOf(label);
  if (i < 0 || i >= SWATCHES.length) {
    return { dot: 'bg-muted-foreground', bar: 'bg-muted-foreground/60', text: 'text-muted-foreground' };
  }
  return SWATCHES[i];
}

/**
 * The speakers audible during one transcript row.
 *
 * More than one is normal, not an error: rows and turns come from separate
 * passes over the same audio and do not align. Renders nothing at all when the
 * row has no timestamps to match against -- an empty space is honest, a
 * "Unknown" chip would assert something.
 */
export function SpeakerChips({
  speakers,
  order,
  crosstalk,
}: {
  speakers: string[];
  order: string[];
  /**
   * Two people genuinely talking at once -- NOT the same as a row having two
   * speakers, which is usually just a row spanning a handover. Only real
   * simultaneous speech is marked: two chips already say "two speakers", and a
   * word there would describe an ordinary change of speaker as crosstalk.
   */
  crosstalk?: boolean;
}) {
  if (speakers.length === 0) return null;
  return (
    <span className="inline-flex flex-wrap items-center gap-1 align-middle">
      {speakers.map((label) => {
        const s = swatchFor(label, order);
        return (
          <span
            key={label}
            className={`inline-flex items-center gap-1 rounded-full border border-border bg-muted/50 px-1.5 py-0.5 text-[10px] font-medium ${s.text}`}
          >
            <span className={`h-1.5 w-1.5 rounded-full ${s.dot}`} aria-hidden />
            {label}
          </span>
        );
      })}
      {crosstalk && speakers.length > 1 && (
        <span
          className="text-[10px] text-muted-foreground"
          title="Two speakers were talking at the same time here"
        >
          at the same time
        </span>
      )}
    </span>
  );
}

function TalkTimeRow({ row, order }: { row: SpeakerTalkTime; order: string[] }) {
  const s = swatchFor(row.label, order);
  const percent = Math.round(row.shareOfSpeech * 100);
  return (
    <li className="flex items-center gap-3">
      <span className={`h-2 w-2 shrink-0 rounded-full ${s.dot}`} aria-hidden />
      <span className="w-24 shrink-0 truncate text-sm font-medium">{row.label}</span>
      <span className="h-1.5 flex-1 overflow-hidden rounded-full bg-muted">
        <span
          className={`block h-full rounded-full ${s.bar}`}
          style={{ width: `${Math.max(row.shareOfSpeech * 100, 1)}%` }}
        />
      </span>
      <span className="w-14 shrink-0 text-right text-sm tabular-nums">
        {formatTalkTime(row.ms)}
      </span>
      <span className="w-12 shrink-0 text-right text-xs tabular-nums text-muted-foreground">
        {percent}%
      </span>
    </li>
  );
}

/** Everything the diarization pass can be in the middle of. */
export type TalkTimeState =
  /** No saved audio, so a pass can never run. */
  | { kind: 'noAudio' }
  /** Audio and models present; a pass has not been run. */
  | { kind: 'ready' }
  /** Models are not on disk yet. */
  | { kind: 'modelsMissing' }
  /** A pass finished. `turns` may be empty -- that is an answer, not a failure. */
  | { kind: 'done'; diarizedAt: string; turns: SpeakerTurn[] };

function Frame({ children, actions }: { children: React.ReactNode; actions?: React.ReactNode }) {
  return (
    <section className="rounded-xl border border-border bg-card p-4">
      <div className="mb-3 flex items-center justify-between gap-3">
        <h3 className="inline-flex items-center gap-2 text-sm font-semibold">
          <Users className="h-4 w-4 text-muted-foreground" aria-hidden />
          Speakers
        </h3>
        {actions}
      </div>
      {children}
    </section>
  );
}

/**
 * The talk-time panel, in whichever of the four states applies.
 *
 * The states are kept apart on purpose. "Ran and separated nothing" and "never
 * ran" look identical if you only check whether the turn list is empty, and
 * collapsing them would leave the app forever offering a pass it has already
 * done -- or, worse, implying a one-voice recording had not been analysed.
 */
export function TalkTimePanel({
  state,
  onRun,
  onGetModels,
  busy,
}: {
  state: TalkTimeState;
  onRun?: () => void;
  onGetModels?: () => void;
  busy?: boolean;
}) {
  if (state.kind === 'noAudio') {
    return (
      <Frame>
        <p className="text-sm text-muted-foreground">
          This meeting kept no audio, so speakers cannot be separated. Recordings saved as
          transcripts only have nothing left to analyse.
        </p>
      </Frame>
    );
  }

  if (state.kind === 'modelsMissing') {
    return (
      <Frame
        actions={
          onGetModels && (
            <button
              type="button"
              onClick={onGetModels}
              disabled={busy}
              className="rounded-lg border border-border px-2.5 py-1 text-xs font-medium transition-colors hover:bg-muted disabled:opacity-60"
            >
              {busy ? 'Downloading…' : `Download models (${MODEL_DOWNLOAD_MB} MB)`}
            </button>
          )
        }
      >
        <p className="text-sm text-muted-foreground">
          Separating speakers needs two small models that are not on this machine yet. They are
          downloaded once and then run entirely on-device.
        </p>
      </Frame>
    );
  }

  if (state.kind === 'ready') {
    return (
      <Frame
        actions={
          onRun && (
            <button
              type="button"
              onClick={onRun}
              disabled={busy}
              className="rounded-lg border border-border px-2.5 py-1 text-xs font-medium transition-colors hover:bg-muted disabled:opacity-60"
            >
              {busy ? 'Analysing…' : 'Identify speakers'}
            </button>
          )
        }
      >
        <p className="text-sm text-muted-foreground">
          Speakers have not been separated for this meeting yet. The pass runs on-device over the
          saved recording and does not change the transcript.
        </p>
      </Frame>
    );
  }

  const rows = talkTime(state.turns);
  const count = speakerCount(state.turns);

  if (rows.length === 0) {
    return (
      <Frame>
        <p className="text-sm text-muted-foreground">
          Analysed on {state.diarizedAt.slice(0, 10)} and no distinct speakers were separated. That
          is the expected result for a single-voice recording, and can also happen when voices are
          too similar or the audio is too noisy to tell apart.
        </p>
      </Frame>
    );
  }

  const order = rows.map((r) => r.label);
  return (
    <Frame>
      <ul className="space-y-2">
        {rows.map((row) => (
          <TalkTimeRow key={row.label} row={row} order={order} />
        ))}
      </ul>
      <p className="mt-3 border-t border-border pt-3 text-xs leading-relaxed text-muted-foreground">
        {count} {count === 1 ? 'voice' : 'voices'} separated on-device, listed in the order they
        first spoke. This is a <strong className="font-medium">best-effort estimate</strong>: how
        often it gets the speakers right has not been measured on recordings like yours, so check it
        before relying on it. Percentages are each speaker&apos;s share of the <em>speech</em>, not of the
        meeting — people talk over each other, so shares of the meeting would add up to more than
        100%. Speaking time describes the recording; it is not a measure of contribution. Labels stay
        anonymous and there is no way to rename them here: storing which voice belongs to
        whom would be biometric data. Write names in your own notes if you need them.
      </p>
    </Frame>
  );
}
