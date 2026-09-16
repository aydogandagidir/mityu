'use client';

/**
 * /design/report — read.ai-referenced Meeting Report prototype (Tauri-free).
 *
 * This is the visual target for Phase B: the meeting-details/summary view redesigned
 * as a read.ai-style Report — a header with on-device overview metrics, then sectioned
 * cards (Summary → Key takeaways → Action items → Topics/timeline → Transcript), every
 * AI block a source-linked HITL draft. Rendered with mock data so the look can be
 * reviewed + screenshot-inspected before wiring to the real (data-bound) view.
 *
 * All colors are semantic tokens (brand bluedev #1E56FF), so it adapts light/dark.
 */

import {
  Sparkles, ListChecks, MessageSquareQuote, Clock, FileText, Users,
  CheckCircle2, Circle, Download, Check, ChevronRight, Hash,
} from 'lucide-react';
import { AiLabel, SourceChip } from '@/components/report/primitives';
import { StatTile } from '@/components/ui/stat-tile';
import { Card } from '@/components/ui/card';
import { Button } from '@/components/ui/button';

/**
 * The section card is the only shape this fixture still declares, and it is a LAYOUT,
 * not a primitive: a titled `Card` with the Art. 50 marking in its header. The four
 * private copies of StatTile, AiLabel, SourceChip and the card chrome that used to live
 * here were deleted — a fixture that renders its own lookalikes proves the lookalikes.
 */
function SectionCard({
  icon: Icon,
  title,
  count,
  children,
  accent,
  marking = true,
}: {
  icon: any;
  title: string;
  count?: number;
  children: React.ReactNode;
  accent?: boolean;
  /**
   * The Art. 50 marking belongs on MODEL OUTPUT. Chapters are computed on-device from
   * pauses — deterministic, no model involved — and the transcript is the evidence a
   * claim is checked against, not a claim. Marking either of them "AI-generated" would
   * be inaccurate in the direction that costs the most: a reader who sees the label
   * everywhere stops reading it anywhere.
   */
  marking?: boolean;
}) {
  return (
    <Card className="overflow-hidden p-0">
      <section aria-label={title}>
        <header className="flex flex-wrap items-center gap-2 border-b border-border px-5 py-3">
          <span
            className={`grid size-7 place-items-center rounded-sm ${
              accent ? 'bg-primary text-primary-foreground' : 'bg-accent text-accent-foreground'
            }`}
          >
            <Icon className="size-4" aria-hidden="true" />
          </span>
          <h2 className="text-title text-foreground">{title}</h2>
          {count != null && (
            <span className="rounded-full bg-muted px-2 py-0.5 text-caption tabular-nums text-muted-foreground">
              {count}
            </span>
          )}
          {marking && (
            <div className="ml-auto">
              <AiLabel />
            </div>
          )}
        </header>
        <div className="p-5">{children}</div>
      </section>
    </Card>
  );
}

/* ---------- report ---------- */

const ACTIONS = [
  { text: 'Send the revised pricing deck to Acme by Friday', owner: 'AD', due: 'Fri', src: '12:04', done: false },
  { text: 'Book the security review with the platform team', owner: 'MY', due: 'Next week', src: '18:20', done: false },
  { text: 'Share the Phase-0 recordings for the transcription gate', owner: 'AD', due: '—', src: '31:47', done: true },
];

const TOPICS = [
  { t: 'Intro & context', start: 0, len: 14 },
  { t: 'Pricing & packaging', start: 14, len: 36 },
  { t: 'Security & compliance', start: 50, len: 22 },
  { t: 'Next steps', start: 72, len: 12 },
];

export default function ReportPreview() {
  const total = 84;
  return (
    <div className="min-h-screen bg-background text-foreground">
      <div className="mx-auto max-w-4xl px-6 py-8 space-y-6">

        {/* Header */}
        <header className="space-y-4">
          <div className="flex items-start gap-4">
            <div className="flex-1">
              <div className="text-xs uppercase tracking-widest text-muted-foreground">Meeting report</div>
              <h1 className="text-2xl font-semibold tracking-tight">Q3 planning with Acme</h1>
              <div className="mt-1 flex items-center gap-3 text-sm text-muted-foreground">
                <span>Wed, 9 Jul 2026</span><span>·</span><span>01:24</span><span>·</span>
                <span className="inline-flex items-center gap-1"><Users className="h-3.5 w-3.5" /> 3 speakers</span>
              </div>
            </div>
            <div className="flex items-center gap-2">
              <button className="inline-flex items-center gap-1.5 rounded-lg border border-border bg-background px-3 py-2 text-sm font-medium hover:bg-muted transition-colors">
                <Download className="h-4 w-4" /> Export
              </button>
              <button className="inline-flex items-center gap-1.5 rounded-lg bg-primary px-3 py-2 text-sm font-medium text-primary-foreground shadow-sm hover:bg-primary/90 transition-colors">
                <Check className="h-4 w-4" /> Approve all
              </button>
            </div>
          </div>

          {/* On-device overview metrics */}
          <div className="flex flex-wrap gap-3">
            <StatTile icon={Clock} label="Duration" value="1h 24m" />
            <StatTile icon={FileText} label="Words" value="9,312" sub="~112 wpm" />
            <StatTile icon={Hash} label="Segments" value="146" />
            <StatTile icon={ListChecks} label="Action items" value="3" sub="1 done" />
          </div>
        </header>

        {/* Summary */}
        <SectionCard icon={Sparkles} title="Summary" accent>
          <p className="text-[15px] leading-relaxed text-foreground/90">
            The team aligned on Q3 pricing and packaging for Acme, agreeing to lead with the managed tier and
            treat security review as a gating step. Open questions on data residency were deferred to a follow-up
            with the platform team. Next steps center on the revised deck and scheduling the review.
          </p>
        </SectionCard>

        {/* Key takeaways */}
        <SectionCard icon={MessageSquareQuote} title="Key takeaways" count={3}>
          <ul className="space-y-3">
            {['Lead with the managed tier; usage-based add-ons stay optional.',
              'Security review is a hard gate before any signature.',
              'Data-residency answer owed before the next call.'].map((k, i) => (
              <li key={i} className="flex items-start gap-3">
                <span className="mt-1.5 h-1.5 w-1.5 shrink-0 rounded-full bg-primary" />
                <span className="flex-1 text-[15px] text-foreground/90">{k}</span>
                <SourceChip timestamp={['08:12', '50:03', '61:20'][i]} />
              </li>
            ))}
          </ul>
        </SectionCard>

        {/* Action items */}
        <SectionCard icon={ListChecks} title="Action items" count={ACTIONS.length}>
          <ul className="divide-y divide-border">
            {ACTIONS.map((a, i) => (
              <li key={i} className="flex items-center gap-3 py-3 first:pt-0 last:pb-0">
                {a.done
                  ? <CheckCircle2 className="h-5 w-5 shrink-0 text-primary" />
                  : <Circle className="h-5 w-5 shrink-0 text-muted-foreground" />}
                <div className="flex-1">
                  <div className={`text-[15px] ${a.done ? 'text-muted-foreground line-through' : 'text-foreground'}`}>{a.text}</div>
                  <div className="mt-1 flex items-center gap-2">
                    <span className="inline-flex items-center gap-1 rounded-full bg-muted px-2 py-0.5 text-xs text-muted-foreground">
                      <span className="grid h-4 w-4 place-items-center rounded-full bg-primary text-[9px] font-semibold text-primary-foreground">{a.owner}</span>
                      {a.due}
                    </span>
                    <SourceChip timestamp={a.src} />
                  </div>
                </div>
              </li>
            ))}
          </ul>
        </SectionCard>

        {/* Topics / chapters timeline */}
        <SectionCard icon={Hash} title="Topics & chapters" count={TOPICS.length} marking={false}>
          <div className="space-y-2.5">
            {TOPICS.map((topic, i) => (
              <button key={i} className="group flex w-full items-center gap-3 text-left">
                <span className="w-16 shrink-0 text-xs tabular-nums text-muted-foreground">
                  {String(Math.floor(topic.start * 0.6)).padStart(2, '0')}:{String((topic.start * 6) % 60).padStart(2, '0')}
                </span>
                <span className="w-40 shrink-0 truncate text-sm font-medium text-foreground group-hover:text-primary transition-colors">{topic.t}</span>
                <span className="relative h-2 flex-1 overflow-hidden rounded-full bg-muted">
                  <span className="absolute inset-y-0 rounded-full bg-primary/70"
                    style={{ left: `${(topic.start / total) * 100}%`, width: `${(topic.len / total) * 100}%` }} />
                </span>
                <ChevronRight className="h-4 w-4 shrink-0 text-muted-foreground group-hover:text-foreground transition-colors" />
              </button>
            ))}
          </div>
        </SectionCard>

        {/* Transcript (secondary, collapsible in the real view) */}
        <SectionCard icon={FileText} title="Transcript" marking={false}>
          <div className="space-y-3 text-sm">
            {[['05:10', 'A', 'So the main question is how we package the managed tier for Acme.'],
              ['05:24', 'B', 'Right — and whether security review blocks the timeline.'],
              ['05:31', 'A', 'It does. Let’s treat it as a gate before anything is signed.']].map((r, i) => (
              <div key={i} className="flex gap-3">
                <span className="w-12 shrink-0 tabular-nums text-muted-foreground">{r[0]}</span>
                <span className="grid h-5 w-5 shrink-0 place-items-center rounded-full bg-accent text-[10px] font-semibold text-accent-foreground">{r[1]}</span>
                <span className="text-foreground/90">{r[2]}</span>
              </div>
            ))}
          </div>
        </SectionCard>

      </div>
    </div>
  );
}
