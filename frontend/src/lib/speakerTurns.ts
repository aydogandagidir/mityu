/**
 * Turning stored speaker turns into things a person can read.
 *
 * Pure functions with no React and no `invoke`, because the two decisions in
 * here are the ones most likely to be wrong in a way nobody notices:
 *
 * 1. **A transcript row can overlap more than one speaker.** Rows and turns come
 *    from different passes over the same audio and do not line up — the turns
 *    migration says so in its own note (a VAD bridges a two-second pause that
 *    the transcriber split on). So a row is attributed to a LIST, never to one
 *    speaker. Picking "the dominant one" would print a confident single name
 *    over a stretch where two people spoke.
 *
 * 2. **Talk time cannot be a share of the meeting.** When two people talk at
 *    once, the speaking time adds up to more than the recording lasts, so shares
 *    over meeting duration exceed 100%. Normalising that away silently would
 *    make the numbers add up and be wrong. See {@link talkTime}.
 *
 * ADR-0034 bounds what this may say: talk time is descriptive, never a score or
 * a ranking, and labels stay anonymous `Speaker N` -- naming a real person is a
 * manual human act, kept out of automated output because a stored voice-to-name
 * mapping is biometric data under KVKK/GDPR.
 */

/** One stored turn, exactly as `api_get_speaker_turns` returns it. */
export interface SpeakerTurn {
  start_ms: number;
  end_ms: number;
  speaker_label: string;
  confidence: number | null;
}

/**
 * What `api_diarization_availability` returns.
 *
 * The field really is `diarized_at` next to a camelCase `status`: serde's
 * `rename_all` on an enum renames the variants and leaves struct-variant fields
 * alone. Asserted on the Rust side by
 * `diarization::service::tests::the_wire_shape_the_ui_is_written_against`,
 * because `diarizedAt` would read `undefined` and still typecheck.
 */
export type DiarizationAvailability =
  | { status: 'noAudio' }
  | { status: 'ready' }
  | { status: 'modelsMissing' }
  | { status: 'done'; diarized_at: string; turns: number };

/**
 * A transcript row's extent, in seconds from the recording start.
 *
 * Neutral field names on purpose: the app carries two row shapes -- `Transcript`
 * (`audio_start_time`) and `TranscriptSegmentData` (`timestamp`/`endTime`) --
 * and binding this to either would make the other call site read as a special
 * case. Both map into this at the boundary.
 */
export interface RowExtent {
  start?: number;
  end?: number;
}

/**
 * Overlap small enough to be a boundary artefact rather than someone speaking.
 *
 * Without a floor, every row picks up its neighbour's speaker from a few
 * milliseconds of touching edges, and almost every row would show two names --
 * which reads as constant crosstalk and makes the genuine overlaps invisible.
 */
const MIN_OVERLAP_MS = 250;

/**
 * ...unless the row is short, where 250 ms is most of it. A 400 ms row split
 * evenly between two speakers is a real overlap, so a proportional test runs
 * alongside the absolute one and either qualifies.
 */
const MIN_OVERLAP_FRACTION = 0.15;

const secondsToMs = (s: number) => Math.round(s * 1000);

/**
 * The speakers audible during one transcript row, in the order they start.
 *
 * Returns `[]` for a row with no timestamps. That is "we cannot say", and the
 * caller must render it as absence rather than as "nobody spoke" -- older
 * recordings predate recording-relative timestamps entirely.
 */
export function speakersForRow(row: RowExtent, turns: SpeakerTurn[]): string[] {
  if (row.start === undefined || row.end === undefined) {
    return [];
  }
  const start = secondsToMs(row.start);
  const end = secondsToMs(row.end);
  if (!Number.isFinite(start) || !Number.isFinite(end) || end <= start) return [];

  const rowMs = end - start;
  const floor = Math.min(MIN_OVERLAP_MS, rowMs * MIN_OVERLAP_FRACTION);

  const seen = new Map<string, number>();
  for (const t of turns) {
    const overlap = Math.min(end, t.end_ms) - Math.max(start, t.start_ms);
    if (overlap <= 0 || overlap < floor) continue;
    // A speaker may hold several turns inside one row; keep the earliest start
    // so the ordering below is "who spoke first", not "which turn came last".
    const prev = seen.get(t.speaker_label);
    if (prev === undefined || t.start_ms < prev) seen.set(t.speaker_label, t.start_ms);
  }
  return [...seen.entries()].sort((a, b) => a[1] - b[1]).map(([label]) => label);
}

/**
 * Overlap between two DIFFERENT speakers below which this is rounding, not
 * crosstalk. Smaller than {@link MIN_OVERLAP_MS} because genuine interruptions
 * are short -- "sorry, go ahead" is under a second -- but not zero, because
 * turns that merely touch at a handover must not be reported as people talking
 * over each other.
 */
const MIN_CROSSTALK_MS = 100;

/**
 * Did two different people actually speak AT THE SAME TIME during this row?
 *
 * Distinct from "this row has more than one speaker", which is the far more
 * common case of a row spanning a handover: turns `[0, 5s]` and `[5s, 12s]` are
 * sequential and do not overlap each other at all. Calling that crosstalk would
 * describe an ordinary change of speaker as people talking over one another.
 */
export function hasCrosstalk(row: RowExtent, turns: SpeakerTurn[]): boolean {
  if (row.start === undefined || row.end === undefined) return false;
  const start = secondsToMs(row.start);
  const end = secondsToMs(row.end);
  if (!Number.isFinite(start) || !Number.isFinite(end) || end <= start) return false;

  // Clip each turn to the row, then look for two clipped turns from different
  // speakers that intersect. Clipping matters: two speakers may overlap outside
  // the row while this row itself contains only one of them.
  const clipped = turns
    .map((t) => ({
      label: t.speaker_label,
      s: Math.max(start, t.start_ms),
      e: Math.min(end, t.end_ms),
    }))
    .filter((t) => t.e > t.s);

  for (let i = 0; i < clipped.length; i++) {
    for (let j = i + 1; j < clipped.length; j++) {
      if (clipped[i].label === clipped[j].label) continue;
      const overlap = Math.min(clipped[i].e, clipped[j].e) - Math.max(clipped[i].s, clipped[j].s);
      if (overlap >= MIN_CROSSTALK_MS) return true;
    }
  }
  return false;
}

export interface SpeakerTalkTime {
  label: string;
  /** Union of this speaker's turns, so self-overlap is not counted twice. */
  ms: number;
  /**
   * Fraction of ATTRIBUTED SPEECH, not of the meeting. Sums to 1 across
   * speakers. See {@link talkTime} for why the meeting is the wrong denominator.
   */
  shareOfSpeech: number;
}

/** Total length of a set of intervals, counting overlapping stretches once. */
function unionMs(intervals: Array<[number, number]>): number {
  if (intervals.length === 0) return 0;
  const sorted = [...intervals].sort((a, b) => a[0] - b[0]);
  let total = 0;
  let [openStart, openEnd] = sorted[0];
  for (let i = 1; i < sorted.length; i++) {
    const [s, e] = sorted[i];
    if (s > openEnd) {
      total += openEnd - openStart;
      openStart = s;
      openEnd = e;
    } else if (e > openEnd) {
      openEnd = e;
    }
  }
  return total + (openEnd - openStart);
}

/**
 * How long each speaker spoke, and their share of the speech.
 *
 * **Order is by first appearance, never by duration.** Sorting by talk time
 * would turn a description into a leaderboard, which ADR-0034 rules out: a
 * transcript is not a measure of contribution, and presenting it as a ranking
 * invites exactly the employee-performance reading the project refuses to make.
 *
 * **The denominator is the sum of speech, not the meeting.** Two people talking
 * at once produce more speaking time than the recording lasts, so shares over
 * meeting duration can exceed 100%. Callers must label these as a share of
 * speech; a percentage of "the meeting" would be a different and wrong claim.
 *
 * Each speaker's own time is a UNION of their turns, so a speaker whose turns
 * overlap themselves is not credited twice.
 */
export function talkTime(turns: SpeakerTurn[]): SpeakerTalkTime[] {
  const byLabel = new Map<string, { first: number; spans: Array<[number, number]> }>();
  for (const t of turns) {
    if (t.end_ms <= t.start_ms) continue;
    const entry = byLabel.get(t.speaker_label);
    if (entry) {
      entry.spans.push([t.start_ms, t.end_ms]);
      if (t.start_ms < entry.first) entry.first = t.start_ms;
    } else {
      byLabel.set(t.speaker_label, { first: t.start_ms, spans: [[t.start_ms, t.end_ms]] });
    }
  }

  const rows = [...byLabel.entries()]
    .map(([label, { first, spans }]) => ({ label, first, ms: unionMs(spans) }))
    .sort((a, b) => a.first - b.first || a.label.localeCompare(b.label));

  const total = rows.reduce((sum, r) => sum + r.ms, 0);
  return rows.map(({ label, ms }) => ({
    label,
    ms,
    shareOfSpeech: total > 0 ? ms / total : 0,
  }));
}

/** `m:ss`, or `h:mm:ss` past an hour. Rounded down: 59.9 s is not a minute. */
export function formatTalkTime(ms: number): string {
  const totalSeconds = Math.floor(Math.max(0, ms) / 1000);
  const seconds = totalSeconds % 60;
  const minutes = Math.floor(totalSeconds / 60) % 60;
  const hours = Math.floor(totalSeconds / 3600);
  const mm = String(minutes).padStart(2, '0');
  const ss = String(seconds).padStart(2, '0');
  return hours > 0 ? `${hours}:${mm}:${ss}` : `${minutes}:${ss}`;
}

/** Distinct speakers found, for a headline count. */
export function speakerCount(turns: SpeakerTurn[]): number {
  return new Set(turns.map((t) => t.speaker_label)).size;
}
