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

import {
  InlinePiiBadge,
  RedactedText,
  SpeakerChips,
  swatchFor,
  TalkTimePanel,
} from '@/components/report/SpeakerTurns';
import {
  formatTalkTime,
  hasCrosstalk,
  speakersForRow,
  talkTime,
  type SpeakerTurn,
} from '@/lib/speakerTurns';

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
  turn(96_000, 108_000, 'Speaker 4'),
  turn(108_000, 118_000, 'Speaker 5'),
  turn(118_000, 127_000, 'Speaker 6'),
  turn(127_000, 135_000, 'Speaker 7'),
  turn(135_000, 142_000, 'Speaker 8'),
];

const TOTAL_MEETING_MS = 142_000;

const ROWS: Array<{ id: string; start?: number; end?: number; text: string; note: string }> = [
  {
    id: 'a',
    start: 1,
    end: 5.4,
    text: 'Right, let us pick up where we left off on the pricing tiers.',
    note: 'inside one turn (Speaker 1)',
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
    note: 'genuinely simultaneous speech (Nemotron 3.5 Transcribe Sortformer overlap resolution)',
  },
  {
    id: 'e',
    start: 97,
    end: 141,
    text: 'Engineering, Legal, Finance, Ops, and QA signed off — send the packet to [REDACTED_EMAIL] or call [REDACTED_PHONE].',
    note: 'spans Speakers 4–8 across the 8-colour palette with decode-time inline PII redaction ([REDACTED_EMAIL])',
  },
  {
    id: 'd',
    text: 'An older recording, saved before segment timings were stored.',
    note: 'no timestamps: nothing is claimed',
  },
];

function NemotronAttributionBar({ order }: { order: string[] }) {
  return (
    <section className="rounded-xl border border-border bg-card p-4">
      <div className="mb-3 flex flex-wrap items-center justify-between gap-2">
        <div>
          <div className="text-eyebrow uppercase text-subtle-foreground">
            NVIDIA Nemotron 3.5 Transcribe (30B-A4B MoE)
          </div>
          <h3 className="text-title-sm text-foreground">
            8-Speaker Live Diarization &amp; Sortformer Overlap Timeline
          </h3>
        </div>
        <div className="flex flex-wrap items-center gap-1.5">
          <span className="inline-flex items-center gap-1 rounded-full border border-warning-border bg-warning-surface px-2 py-0.5 text-micro font-medium text-warning-ink tabular-nums">
            Overlap · 01:02–01:04
          </span>
          <InlinePiiBadge token="[REDACTED_EMAIL]" />
        </div>
      </div>

      <div
        className="relative h-3 w-full overflow-hidden rounded-full border border-border/60 bg-muted/70"
        aria-label="8-speaker timeline attribution bar"
      >
        {TURNS.map((t, index) => {
          const swatch = swatchFor(t.speaker_label, order);
          const leftPct = (t.start_ms / TOTAL_MEETING_MS) * 100;
          const widthPct = Math.max(((t.end_ms - t.start_ms) / TOTAL_MEETING_MS) * 100, 1.5);
          return (
            <span
              key={`${t.speaker_label}-${t.start_ms}-${index}`}
              className={`absolute inset-y-0 ${swatch.bar}`}
              style={{ left: `${leftPct}%`, width: `${widthPct}%` }}
              title={`${t.speaker_label}: ${formatTalkTime(t.start_ms)}–${formatTalkTime(t.end_ms)}`}
            />
          );
        })}
      </div>

      <div className="mt-2.5 flex flex-wrap items-center justify-between gap-2">
        <div className="flex flex-wrap items-center gap-1.5">
          {order.map((label) => {
            const s = swatchFor(label, order);
            return (
              <span
                key={label}
                className="inline-flex items-center gap-1.5 rounded-full border border-border bg-surface-2 px-2 py-0.5 text-micro font-medium tabular-nums text-foreground"
              >
                <span className={`h-1.5 w-1.5 shrink-0 rounded-full ${s.dot}`} aria-hidden="true" />
                {label}
              </span>
            );
          })}
        </div>
        <span className="font-mono text-micro tabular-nums text-subtle-foreground">
          00:00 — 02:22 (8 voices)
        </span>
      </div>
    </section>
  );
}

function Row({ row, order }: { row: (typeof ROWS)[number]; order: string[] }) {
  const extent = { start: row.start, end: row.end };
  const speakers = speakersForRow(extent, TURNS);
  return (
    <div className="rounded-lg px-2 py-1.5 transition-colors duration-fast hover:bg-muted/40">
      <div className="flex items-start gap-2.5">
        <span className="mt-0.5 min-w-[46px] shrink-0 font-mono text-micro tabular-nums text-subtle-foreground">
          {row.start === undefined
            ? '[--:--]'
            : `[${String(Math.floor(row.start / 60)).padStart(2, '0')}:${String(
                Math.floor(row.start % 60)
              ).padStart(2, '0')}]`}
        </span>
        <div className="flex-1 space-y-1">
          <SpeakerChips speakers={speakers} order={order} crosstalk={hasCrosstalk(extent, TURNS)} />
          <p className="text-read leading-relaxed text-foreground">
            <RedactedText text={row.text} />
          </p>
          <p className="text-caption text-muted-foreground">{row.note}</p>
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
          <div className="text-eyebrow uppercase text-subtle-foreground">
            Design preview
          </div>
          <h1 className="text-display">Speakers and talk time</h1>
          <p className="mt-1 text-body text-muted-foreground">
            Fixture data. Every state the view can be in, so it can be checked without a recording.
          </p>
        </header>

        <NemotronAttributionBar order={order} />

        <TalkTimePanel
          state={{ kind: 'done', diarizedAt: '2026-08-09T10:12:00Z', turns: TURNS }}
        />

        <section className="rounded-xl border border-border bg-card p-4">
          <h3 className="mb-3 text-title-sm text-foreground">Transcript rows</h3>
          <div className="space-y-1">
            {ROWS.map((row) => (
              <Row key={row.id} row={row} order={order} />
            ))}
          </div>
        </section>

        <section className="space-y-3">
          <h2 className="text-title-sm text-muted-foreground">The remaining states</h2>
          <TalkTimePanel state={{ kind: 'ready' }} onRun={() => {}} />
          <TalkTimePanel state={{ kind: 'modelsMissing' }} onGetModels={() => {}} />
          <TalkTimePanel state={{ kind: 'noAudio' }} />
          <TalkTimePanel state={{ kind: 'done', diarizedAt: '2026-08-09T10:12:00Z', turns: [] }} />
        </section>
      </div>
    </div>
  );
}
