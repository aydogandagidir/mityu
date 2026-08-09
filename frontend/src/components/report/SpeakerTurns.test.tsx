// @vitest-environment jsdom
import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';

import { SpeakerChips, TalkTimePanel } from './SpeakerTurns';
import type { SpeakerTurn } from '@/lib/speakerTurns';

afterEach(cleanup);

const turn = (start_ms: number, end_ms: number, speaker_label: string): SpeakerTurn => ({
  start_ms,
  end_ms,
  speaker_label,
  confidence: null,
});

/**
 * Speaker 1 speaks FIRST and LEAST -- the fixture that tells a description
 * apart from a leaderboard.
 */
const TURNS = [
  turn(0, 6_000, 'Speaker 1'),
  turn(6_000, 41_000, 'Speaker 2'),
  turn(41_000, 58_000, 'Speaker 3'),
  turn(58_000, 64_000, 'Speaker 1'),
];

describe('TalkTimePanel: the four states stay distinguishable', () => {
  /**
   * The whole reason the Rust side stamps `diarized_at` even for an empty
   * result. If these two ever render the same words, the app either keeps
   * offering a pass it already ran, or tells the user a single-voice recording
   * was never analysed.
   */
  it('says something different for "never ran" and "ran and found nothing"', () => {
    const { unmount } = render(<TalkTimePanel state={{ kind: 'ready' }} onRun={() => {}} />);
    const neverRan = document.body.textContent ?? '';
    expect(neverRan).toContain('have not been separated');
    expect(screen.getByRole('button', { name: /identify speakers/i })).toBeTruthy();
    unmount();

    render(
      <TalkTimePanel state={{ kind: 'done', diarizedAt: '2026-08-09T10:12:00Z', turns: [] }} />
    );
    const ranEmpty = document.body.textContent ?? '';
    expect(ranEmpty).toContain('no distinct speakers were separated');
    expect(ranEmpty).not.toEqual(neverRan);
    // Nothing to run again -- the pass already happened.
    expect(screen.queryByRole('button', { name: /identify speakers/i })).toBeNull();
  });

  it('explains that a transcripts-only meeting can never be diarized, and offers nothing', () => {
    render(<TalkTimePanel state={{ kind: 'noAudio' }} onRun={() => {}} />);
    expect(document.body.textContent).toContain('kept no audio');
    expect(screen.queryByRole('button')).toBeNull();
  });

  it('offers the download, not the pass, when the models are missing', () => {
    render(
      <TalkTimePanel state={{ kind: 'modelsMissing' }} onRun={() => {}} onGetModels={() => {}} />
    );
    expect(screen.getByRole('button', { name: /download models/i })).toBeTruthy();
    expect(screen.queryByRole('button', { name: /identify speakers/i })).toBeNull();
  });
});

describe('TalkTimePanel: what the numbers are allowed to claim', () => {
  it('lists speakers in the order they first spoke, not by who spoke longest', () => {
    render(<TalkTimePanel state={{ kind: 'done', diarizedAt: 'x', turns: TURNS }} />);
    const labels = [...document.querySelectorAll('li')].map((li) =>
      (li.textContent ?? '').trim()
    );
    // Speaker 1 talks least (0:12) and still comes first.
    expect(labels[0]).toContain('Speaker 1');
    expect(labels[0]).toContain('0:12');
    expect(labels[1]).toContain('Speaker 2');
  });

  /**
   * ADR-0034's ethical boundary, and the difference between a transcript tool
   * and an employee-monitoring one. Asserted on the rendered words so a future
   * redesign that quietly drops the caveat fails here.
   */
  it('says the percentage is of speech and that this is not a measure of contribution', () => {
    render(<TalkTimePanel state={{ kind: 'done', diarizedAt: 'x', turns: TURNS }} />);
    const text = document.body.textContent ?? '';
    expect(text).toContain('not a measure of contribution');
    expect(text).toMatch(/share of the\s*speech/);
    // ...and says WHY the meeting is the wrong denominator, rather than leaving
    // the reader to assume the percentages are of elapsed time.
    expect(text).toContain('more than 100%');
  });

  /** Naming a voice is a human act; nothing here may offer to do it. */
  it('keeps labels anonymous and offers no way to name a speaker', () => {
    render(<TalkTimePanel state={{ kind: 'done', diarizedAt: 'x', turns: TURNS }} />);
    const text = document.body.textContent ?? '';
    expect(text).toContain('Labels are anonymous');
    expect(screen.queryByRole('textbox')).toBeNull();
    expect(screen.queryByRole('button', { name: /name|rename|assign/i })).toBeNull();
  });
});

describe('SpeakerChips', () => {
  it('marks a row that spans more than one speaker', () => {
    render(<SpeakerChips speakers={['Speaker 1', 'Speaker 2']} order={['Speaker 1', 'Speaker 2']} />);
    const text = document.body.textContent ?? '';
    expect(text).toContain('Speaker 1');
    expect(text).toContain('Speaker 2');
    expect(text).toContain('overlapping');
  });

  it('does not call a single-speaker row overlapping', () => {
    render(<SpeakerChips speakers={['Speaker 1']} order={['Speaker 1']} />);
    expect(document.body.textContent).not.toContain('overlapping');
  });

  /** An empty list is "we cannot say" and must render as nothing at all. */
  it('renders nothing when there is nothing to say', () => {
    const { container } = render(<SpeakerChips speakers={[]} order={[]} />);
    expect(container.innerHTML).toBe('');
  });
});
