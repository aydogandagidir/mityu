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

/* ---------- primitives ---------- */

function StatTile({ icon: Icon, label, value, sub }: { icon: any; label: string; value: string; sub?: string }) {
  return (
    <div className="flex-1 min-w-[130px] rounded-xl border border-border bg-card p-4">
      <div className="flex items-center gap-2 text-muted-foreground">
        <Icon className="h-4 w-4" />
        <span className="text-caption uppercase tracking-wide">{label}</span>
      </div>
      <div className="mt-2 text-2xl font-semibold tracking-tight text-foreground">{value}</div>
      {sub && <div className="text-caption text-muted-foreground mt-0.5">{sub}</div>}
    </div>
  );
}

function AiLabel() {
  return (
    <span className="inline-flex items-center gap-1 rounded-full bg-accent px-2 py-0.5 text-[10px] font-medium text-accent-foreground">
      <Sparkles className="h-3 w-3" /> AI-generated · review required
    </span>
  );
}

function SourceChip({ t }: { t: string }) {
  return (
    <button className="inline-flex items-center gap-1 rounded-full border border-border bg-muted/60 px-2 py-0.5 text-caption text-muted-foreground hover:text-foreground hover:border-primary/40 transition-colors">
      <Clock className="h-3 w-3" /> {t}
    </button>
  );
}

function SectionCard({
  icon: Icon, title, count, children, accent,
}: { icon: any; title: string; count?: number; children: React.ReactNode; accent?: boolean }) {
  return (
    <section className="rounded-2xl border border-border bg-card shadow-sm overflow-hidden">
      <header className="flex items-center gap-2 border-b border-border px-5 py-3">
        <span className={`grid h-7 w-7 place-items-center rounded-lg ${accent ? 'bg-primary text-primary-foreground' : 'bg-accent text-accent-foreground'}`}>
          <Icon className="h-4 w-4" />
        </span>
        <h2 className="text-h2 font-semibold text-foreground">{title}</h2>
        {count != null && (
          <span className="ml-1 rounded-full bg-muted px-2 py-0.5 text-caption text-muted-foreground">{count}</span>
        )}
        <div className="ml-auto"><AiLabel /></div>
      </header>
      <div className="p-5">{children}</div>
    </section>
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
              <div className="text-caption uppercase tracking-widest text-muted-foreground">Meeting report</div>
              <h1 className="text-display tracking-tight">Q3 planning with Acme</h1>
              <div className="mt-1 flex items-center gap-3 text-small text-muted-foreground">
                <span>Wed, 9 Jul 2026</span><span>·</span><span>01:24</span><span>·</span>
                <span className="inline-flex items-center gap-1"><Users className="h-3.5 w-3.5" /> 3 speakers</span>
              </div>
            </div>
            <div className="flex items-center gap-2">
              <button className="inline-flex items-center gap-1.5 rounded-lg border border-border bg-background px-3 py-2 text-small font-medium hover:bg-muted transition-colors">
                <Download className="h-4 w-4" /> Export
              </button>
              <button className="inline-flex items-center gap-1.5 rounded-lg bg-primary px-3 py-2 text-small font-medium text-primary-foreground shadow-sm hover:bg-primary/90 transition-colors">
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
          <p className="text-body leading-relaxed text-foreground/90">
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
                <span className="flex-1 text-body text-foreground/90">{k}</span>
                <SourceChip t={['08:12', '50:03', '61:20'][i]} />
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
                  <div className={`text-body ${a.done ? 'text-muted-foreground line-through' : 'text-foreground'}`}>{a.text}</div>
                  <div className="mt-1 flex items-center gap-2">
                    <span className="inline-flex items-center gap-1 rounded-full bg-muted px-2 py-0.5 text-caption text-muted-foreground">
                      <span className="grid h-4 w-4 place-items-center rounded-full bg-primary text-[9px] font-semibold text-primary-foreground">{a.owner}</span>
                      {a.due}
                    </span>
                    <SourceChip t={a.src} />
                  </div>
                </div>
              </li>
            ))}
          </ul>
        </SectionCard>

        {/* Topics / chapters timeline */}
        <SectionCard icon={Hash} title="Topics & chapters" count={TOPICS.length}>
          <div className="space-y-2.5">
            {TOPICS.map((topic, i) => (
              <button key={i} className="group flex w-full items-center gap-3 text-left">
                <span className="w-16 shrink-0 text-caption tabular-nums text-muted-foreground">
                  {String(Math.floor(topic.start * 0.6)).padStart(2, '0')}:{String((topic.start * 6) % 60).padStart(2, '0')}
                </span>
                <span className="w-40 shrink-0 truncate text-small font-medium text-foreground group-hover:text-primary transition-colors">{topic.t}</span>
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
        <SectionCard icon={FileText} title="Transcript">
          <div className="space-y-3 text-small">
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
