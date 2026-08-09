import { describe, expect, it } from 'vitest';

import {
  formatTalkTime,
  hasCrosstalk,
  speakerCount,
  speakersForRow,
  talkTime,
  type SpeakerTurn,
} from './speakerTurns';

const turn = (start_ms: number, end_ms: number, speaker_label: string): SpeakerTurn => ({
  start_ms,
  end_ms,
  speaker_label,
  confidence: null,
});

describe('speakersForRow', () => {
  it('attributes a row inside one turn to that speaker', () => {
    const turns = [turn(0, 10_000, 'Speaker 1')];
    expect(speakersForRow({ start: 2, end: 5 }, turns)).toEqual([
      'Speaker 1',
    ]);
  });

  /**
   * The acceptance criterion for H6, and the reason this returns a list. Rows
   * and turns come from different passes and do not align.
   */
  it('shows BOTH speakers when a row spans a handover', () => {
    const turns = [turn(0, 5_000, 'Speaker 1'), turn(5_000, 12_000, 'Speaker 2')];
    expect(speakersForRow({ start: 3, end: 8 }, turns)).toEqual([
      'Speaker 1',
      'Speaker 2',
    ]);
  });

  it('orders by who spoke first within the row, not by turn order in the array', () => {
    const turns = [turn(6_000, 9_000, 'Speaker 2'), turn(0, 6_000, 'Speaker 1')];
    expect(speakersForRow({ start: 0, end: 9 }, turns)).toEqual([
      'Speaker 1',
      'Speaker 2',
    ]);
  });

  it('does not repeat a speaker who holds several turns inside one row', () => {
    const turns = [
      turn(0, 2_000, 'Speaker 1'),
      turn(3_000, 5_000, 'Speaker 2'),
      turn(6_000, 9_000, 'Speaker 1'),
    ];
    expect(speakersForRow({ start: 0, end: 9 }, turns)).toEqual([
      'Speaker 1',
      'Speaker 2',
    ]);
  });

  /**
   * Without the floor, touching edges would put two names on nearly every row,
   * which reads as constant crosstalk and hides the real overlaps.
   */
  it('ignores a few milliseconds of touching edges', () => {
    const turns = [turn(0, 5_050, 'Speaker 1'), turn(5_050, 20_000, 'Speaker 2')];
    // The row starts 50 ms before Speaker 2 hands over -- a boundary artefact.
    expect(speakersForRow({ start: 5, end: 15 }, turns)).toEqual([
      'Speaker 2',
    ]);
  });

  /** ...but on a short row that same 250 ms is most of it, so it counts. */
  it('keeps a short overlap when the row itself is short', () => {
    const turns = [turn(0, 5_200, 'Speaker 1'), turn(5_200, 20_000, 'Speaker 2')];
    expect(speakersForRow({ start: 5, end: 5.4 }, turns)).toEqual([
      'Speaker 1',
      'Speaker 2',
    ]);
  });

  it('reports simultaneous speech as both speakers', () => {
    const turns = [turn(0, 8_000, 'Speaker 1'), turn(2_000, 6_000, 'Speaker 2')];
    expect(speakersForRow({ start: 1, end: 7 }, turns)).toEqual([
      'Speaker 1',
      'Speaker 2',
    ]);
  });

  /**
   * "We cannot say" -- distinct from "nobody spoke". Recordings predating
   * recording-relative timestamps have no extent at all, and the UI must render
   * the difference rather than implying silence.
   */
  it('says nothing about a row with no timestamps', () => {
    const turns = [turn(0, 10_000, 'Speaker 1')];
    expect(speakersForRow({}, turns)).toEqual([]);
    expect(speakersForRow({ start: 1 }, turns)).toEqual([]);
    expect(speakersForRow({ end: 5 }, turns)).toEqual([]);
    expect(speakersForRow({ start: 5, end: 5 }, turns)).toEqual([]);
    expect(speakersForRow({ start: 8, end: 4 }, turns)).toEqual([]);
  });

  it('says nothing when there are no turns', () => {
    expect(speakersForRow({ start: 0, end: 9 }, [])).toEqual([]);
  });

  it('says nothing for a row entirely outside every turn', () => {
    const turns = [turn(0, 5_000, 'Speaker 1')];
    expect(speakersForRow({ start: 30, end: 40 }, turns)).toEqual([]);
  });
});

describe('talkTime', () => {
  it('sums each speaker and shares out the speech', () => {
    const result = talkTime([
      turn(0, 30_000, 'Speaker 1'),
      turn(30_000, 40_000, 'Speaker 2'),
      turn(40_000, 50_000, 'Speaker 1'),
    ]);
    expect(result).toEqual([
      { label: 'Speaker 1', ms: 40_000, shareOfSpeech: 0.8 },
      { label: 'Speaker 2', ms: 10_000, shareOfSpeech: 0.2 },
    ]);
  });

  /**
   * ADR-0034: descriptive, never a ranking. If this sorted by duration the
   * screen would be a leaderboard of who talked most, which is the
   * employee-performance reading the project refuses to support.
   */
  it('orders by first appearance, NOT by who spoke longest', () => {
    const result = talkTime([
      turn(0, 1_000, 'Speaker 1'), // speaks first, speaks least
      turn(1_000, 60_000, 'Speaker 2'),
    ]);
    expect(result.map((r) => r.label)).toEqual(['Speaker 1', 'Speaker 2']);
  });

  /** A speaker whose own turns overlap must not be credited twice. */
  it('counts a speaker overlapping themselves once', () => {
    const result = talkTime([turn(0, 10_000, 'Speaker 1'), turn(5_000, 15_000, 'Speaker 1')]);
    expect(result).toEqual([{ label: 'Speaker 1', ms: 15_000, shareOfSpeech: 1 }]);
  });

  /**
   * The reason the denominator is speech and not meeting duration: here 30 s of
   * recording contains 40 s of speech. Over the meeting these would be 100% and
   * 33%, summing to 133%.
   */
  it('keeps shares summing to 1 even when people talk over each other', () => {
    const result = talkTime([turn(0, 30_000, 'Speaker 1'), turn(10_000, 20_000, 'Speaker 2')]);
    expect(result.map((r) => r.ms)).toEqual([30_000, 10_000]);
    const total = result.reduce((sum, r) => sum + r.shareOfSpeech, 0);
    expect(total).toBeCloseTo(1, 10);
  });

  it('handles a pass that separated nothing', () => {
    expect(talkTime([])).toEqual([]);
  });

  it('drops turns that end before they start rather than counting negative time', () => {
    const result = talkTime([turn(5_000, 1_000, 'Speaker 1'), turn(0, 4_000, 'Speaker 2')]);
    expect(result).toEqual([{ label: 'Speaker 2', ms: 4_000, shareOfSpeech: 1 }]);
  });
});

describe('formatTalkTime', () => {
  it('formats under an hour as m:ss', () => {
    expect(formatTalkTime(0)).toBe('0:00');
    expect(formatTalkTime(9_000)).toBe('0:09');
    expect(formatTalkTime(65_000)).toBe('1:05');
    expect(formatTalkTime(600_000)).toBe('10:00');
  });

  it('formats an hour and over as h:mm:ss', () => {
    expect(formatTalkTime(3_600_000)).toBe('1:00:00');
    expect(formatTalkTime(3_725_000)).toBe('1:02:05');
  });

  /** Rounding up would let 59.9 s read as a minute of talk time. */
  it('rounds down', () => {
    expect(formatTalkTime(59_900)).toBe('0:59');
  });

  it('does not render negative time', () => {
    expect(formatTalkTime(-5_000)).toBe('0:00');
  });
});

describe('speakerCount', () => {
  it('counts distinct speakers, not turns', () => {
    expect(speakerCount([])).toBe(0);
    expect(
      speakerCount([
        turn(0, 1_000, 'Speaker 1'),
        turn(1_000, 2_000, 'Speaker 2'),
        turn(2_000, 3_000, 'Speaker 1'),
      ])
    ).toBe(2);
  });
});

describe('hasCrosstalk', () => {
  /**
   * The distinction a review caught: a row can hold two speakers without them
   * ever speaking at the same time. Calling that "overlapping" described an
   * ordinary change of speaker as people talking over each other.
   */
  it('does NOT call a sequential handover crosstalk', () => {
    const turns = [turn(0, 5_000, 'Speaker 1'), turn(5_000, 12_000, 'Speaker 2')];
    const row = { start: 3, end: 8 };
    // ...even though the row genuinely has two speakers.
    expect(speakersForRow(row, turns)).toEqual(['Speaker 1', 'Speaker 2']);
    expect(hasCrosstalk(row, turns)).toBe(false);
  });

  it('does call genuinely simultaneous speech crosstalk', () => {
    const turns = [turn(0, 8_000, 'Speaker 1'), turn(2_000, 6_000, 'Speaker 2')];
    expect(hasCrosstalk({ start: 1, end: 7 }, turns)).toBe(true);
  });

  /** A speaker overlapping THEMSELVES is a segmentation artefact, not crosstalk. */
  it('ignores one speaker overlapping their own turns', () => {
    const turns = [turn(0, 8_000, 'Speaker 1'), turn(4_000, 12_000, 'Speaker 1')];
    expect(hasCrosstalk({ start: 0, end: 12 }, turns)).toBe(false);
  });

  /** Two speakers overlapping OUTSIDE this row is not this row's crosstalk. */
  it('only counts overlap that falls inside the row', () => {
    const turns = [turn(20_000, 30_000, 'Speaker 1'), turn(25_000, 35_000, 'Speaker 2')];
    expect(hasCrosstalk({ start: 0, end: 10 }, turns)).toBe(false);
    expect(hasCrosstalk({ start: 24, end: 31 }, turns)).toBe(true);
  });

  /** A few milliseconds of touching edges is rounding, not two people talking. */
  it('ignores a sliver of overlap at a boundary', () => {
    const turns = [turn(0, 5_030, 'Speaker 1'), turn(5_000, 12_000, 'Speaker 2')];
    expect(hasCrosstalk({ start: 0, end: 12 }, turns)).toBe(false);
  });

  it('says nothing about a row with no timings', () => {
    const turns = [turn(0, 8_000, 'Speaker 1'), turn(2_000, 6_000, 'Speaker 2')];
    expect(hasCrosstalk({}, turns)).toBe(false);
    expect(hasCrosstalk({ start: 5, end: 5 }, turns)).toBe(false);
  });
});
