'use client';

/**
 * Every state the speaker view can be in, on one page, with fixture data.
 *
 * This exists to be LOOKED AT. The real meeting screen needs a recording, a
 * finished diarization pass and the Tauri runtime, so it cannot be rendered in a
 * browser at all -- which historically meant this kind of screen shipped having
 * only ever been typechecked. `tools/ui/shoot.py` screenshots this route and
 * fails if it comes back blank.
 *
 * The fixtures are chosen to exercise the cases that are easy to get wrong:
 * a row spanning a handover, simultaneous speech, a row with no timestamps, a
 * speaker who talks least but speaks first, and a pass that separated nothing.
 */

import { SpeakerChips, TalkTimePanel } from '@/components/report/SpeakerTurns';
import { hasCrosstalk, speakersForRow, talkTime, type SpeakerTurn } from '@/lib/speakerTurns';

const turn = (start_ms: number, end_ms: number, speaker_label: string): SpeakerTurn => ({
  start_ms,
  end_ms,
  speaker_label,
  confidence: null,
});

/**
 * Speaker 1 speaks FIRST and LEAST. If the panel ever starts ranking by
 * duration this fixture puts Speaker 2 at the top and the screenshot shows it.
 * The 62-64 s stretch is deliberate crosstalk: 1 and 3 speak at once.
 */
const TURNS: SpeakerTurn[] = [
  turn(0, 6_000, 'Speaker 1'),
  turn(6_000, 41_000, 'Speaker 2'),
  turn(41_000, 58_000, 'Speaker 3'),
  turn(58_000, 64_000, 'Speaker 1'),
  turn(62_000, 79_000, 'Speaker 3'),
  turn(79_000, 96_000, 'Speaker 2'),
];

const ROWS: Array<{ id: string; start?: number; end?: number; text: string; note: string }> = [
  {
    id: 'a',
    start: 1,
    end: 5.4,
    text: 'Right, let us pick up where we left off on the pricing tiers.',
    note: 'inside one turn',
  },
  {
    id: 'b',
    start: 4.2,
    end: 9.8,
    text: 'Before that — did we ever hear back from their security team?',
    note: 'spans a handover: two speakers, but NOT at the same time',
  },
  {
    id: 'c',
    start: 60,
    end: 66,
    text: 'We can hold the managed tier — sorry, go ahead — no, you first.',
    note: 'genuinely simultaneous speech',
  },
  {
    id: 'd',
    text: 'An older recording, saved before segment timings were stored.',
    note: 'no timestamps: nothing is claimed',
  },
];

function Row({ row, order }: { row: (typeof ROWS)[number]; order: string[] }) {
  const extent = { start: row.start, end: row.end };
  const speakers = speakersForRow(extent, TURNS);
  return (
    <div className="rounded-lg px-2 py-1.5 hover:bg-muted/40">
      <div className="flex items-start gap-2">
        <span className="mt-0.5 min-w-[46px] shrink-0 text-[11px] tabular-nums text-muted-foreground/70">
          {row.start === undefined
            ? '[--:--]'
            : `[${String(Math.floor(row.start / 60)).padStart(2, '0')}:${String(
                Math.floor(row.start % 60)
              ).padStart(2, '0')}]`}
        </span>
        <div className="flex-1">
          <SpeakerChips speakers={speakers} order={order} crosstalk={hasCrosstalk(extent, TURNS)} />
          <p className="text-[15px] leading-relaxed text-foreground">{row.text}</p>
          <p className="text-xs text-muted-foreground/70">{row.note}</p>
        </div>
      </div>
    </div>
  );
}

export default function SpeakersPreview() {
  const order = talkTime(TURNS).map((r) => r.label);
  return (
    <div className="min-h-screen bg-background text-foreground">
      <div className="mx-auto max-w-3xl space-y-6 px-6 py-8">
        <header>
          <div className="text-xs uppercase tracking-widest text-muted-foreground">
            Design preview
          </div>
          <h1 className="text-2xl font-semibold tracking-tight">Speakers and talk time</h1>
          <p className="mt-1 text-sm text-muted-foreground">
            Fixture data. Every state the view can be in, so it can be checked without a recording.
          </p>
        </header>

        <TalkTimePanel
          state={{ kind: 'done', diarizedAt: '2026-08-09T10:12:00Z', turns: TURNS }}
        />

        <section className="rounded-xl border border-border bg-card p-4">
          <h3 className="mb-3 text-sm font-semibold">Transcript rows</h3>
          <div className="space-y-1">
            {ROWS.map((row) => (
              <Row key={row.id} row={row} order={order} />
            ))}
          </div>
        </section>

        <section className="space-y-3">
          <h2 className="text-sm font-semibold text-muted-foreground">The remaining states</h2>
          <TalkTimePanel state={{ kind: 'ready' }} onRun={() => {}} />
          <TalkTimePanel state={{ kind: 'modelsMissing' }} onGetModels={() => {}} />
          <TalkTimePanel state={{ kind: 'noAudio' }} />
          <TalkTimePanel state={{ kind: 'done', diarizedAt: '2026-08-09T10:12:00Z', turns: [] }} />
        </section>
      </div>
    </div>
  );
}
