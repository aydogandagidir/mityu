'use client';

/**
 * ReportHeader — the read.ai-style header for a meeting (see docs/DESIGN_READAI.md,
 * prototype at /design/report). Renders the meeting title, date + duration meta, and
 * an on-device overview-metrics strip (Duration / Words / Segments / Action items).
 *
 * All metrics are computed locally from the already-loaded transcript — nothing
 * leaves the device, no charisma/bias inference (per the four guardrails). Styled
 * entirely with semantic tokens, so it adapts light/dark.
 */

import { Clock, FileText, Hash, ListChecks } from 'lucide-react';
import type { Transcript } from '@/types';

function formatDuration(sec: number): string {
  if (!sec || sec < 1) return '—';
  const h = Math.floor(sec / 3600);
  const m = Math.floor((sec % 3600) / 60);
  const s = Math.floor(sec % 60);
  if (h > 0) return `${h}h ${m}m`;
  if (m > 0) return `${m}m ${s}s`;
  return `${s}s`;
}

function StatTile({ icon: Icon, label, value, sub }: { icon: any; label: string; value: string; sub?: string }) {
  // Compact, content-hugging stat: icon tile + stacked value/label. Deliberately
  // NOT flex-1 — stretched, mostly-empty tiles read as dead space on wide windows.
  return (
    <div className="inline-flex items-center gap-3 rounded-xl border border-border bg-card py-2.5 pl-3 pr-5 shadow-sm">
      <span className="grid h-9 w-9 shrink-0 place-items-center rounded-lg bg-accent text-accent-foreground">
        <Icon className="h-4 w-4" aria-hidden />
      </span>
      <span className="flex flex-col leading-tight">
        <span className="text-lg font-semibold tracking-tight text-foreground tabular-nums">
          {value}
          {sub && <span className="ml-1.5 text-[11px] font-normal text-muted-foreground">{sub}</span>}
        </span>
        <span className="text-[10px] font-medium uppercase tracking-widest text-muted-foreground">{label}</span>
      </span>
    </div>
  );
}

export function ReportHeader({
  title,
  createdAt,
  transcripts,
  actionItemCount,
}: {
  title: string;
  createdAt?: string;
  transcripts: Transcript[];
  actionItemCount?: number;
}) {
  const durationSec =
    transcripts.reduce((max, t) => Math.max(max, t.audio_end_time ?? 0), 0) ||
    transcripts.reduce((sum, t) => sum + (t.duration ?? 0), 0);

  const words = transcripts.reduce(
    (n, t) => n + (t.text ? t.text.trim().split(/\s+/).filter(Boolean).length : 0),
    0,
  );
  const segments = transcripts.length;

  let dateLabel: string | null = null;
  if (createdAt) {
    const d = new Date(createdAt);
    if (!Number.isNaN(d.getTime())) {
      dateLabel = d.toLocaleDateString(undefined, { weekday: 'short', day: 'numeric', month: 'short', year: 'numeric' });
    }
  }

  const wpm = durationSec > 0 ? Math.round(words / (durationSec / 60)) : 0;

  return (
    <header className="border-b border-border bg-background px-6 py-4">
      {/* Full-width, left-aligned so the header shares a grid with the panels
          below (a centered max-w column over full-width panels reads off-grid).
          Title block left, stat tiles right; wraps on narrow windows. */}
      <div className="flex flex-wrap items-center justify-between gap-x-8 gap-y-3">
        <div className="min-w-0">
          <div className="text-[10px] font-medium uppercase tracking-[0.18em] text-primary">Meeting report</div>
          <h1 className="mt-0.5 truncate text-xl font-semibold tracking-tight text-foreground">{title || 'Untitled meeting'}</h1>
          <div className="mt-0.5 flex flex-wrap items-center gap-x-2.5 gap-y-1 text-[13px] text-muted-foreground">
            {dateLabel && <span>{dateLabel}</span>}
            {dateLabel && durationSec > 0 && <span aria-hidden>·</span>}
            {durationSec > 0 && <span>{formatDuration(durationSec)}</span>}
            <span aria-hidden>·</span>
            <span>{segments} segments</span>
          </div>
        </div>

        <div className="flex flex-wrap items-center gap-2.5">
          <StatTile icon={Clock} label="Duration" value={formatDuration(durationSec)} />
          <StatTile icon={FileText} label="Words" value={words.toLocaleString()} sub={wpm > 0 ? `~${wpm} wpm` : undefined} />
          <StatTile icon={Hash} label="Segments" value={String(segments)} />
          {actionItemCount != null && (
            <StatTile icon={ListChecks} label="Action items" value={String(actionItemCount)} />
          )}
        </div>
      </div>
    </header>
  );
}
