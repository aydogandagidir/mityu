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
  return (
    <div className="flex-1 min-w-[120px] rounded-xl border border-border bg-card p-3.5">
      <div className="flex items-center gap-1.5 text-muted-foreground">
        <Icon className="h-3.5 w-3.5" />
        <span className="text-[11px] uppercase tracking-wide">{label}</span>
      </div>
      <div className="mt-1.5 text-xl font-semibold tracking-tight text-foreground">{value}</div>
      {sub && <div className="text-[11px] text-muted-foreground mt-0.5">{sub}</div>}
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
    <header className="border-b border-border bg-background px-6 py-5">
      <div className="mx-auto max-w-5xl space-y-4">
        <div>
          <div className="text-[11px] uppercase tracking-widest text-muted-foreground">Meeting report</div>
          <h1 className="mt-0.5 text-2xl font-semibold tracking-tight text-foreground truncate">{title || 'Untitled meeting'}</h1>
          <div className="mt-1 flex flex-wrap items-center gap-x-3 gap-y-1 text-sm text-muted-foreground">
            {dateLabel && <span>{dateLabel}</span>}
            {dateLabel && durationSec > 0 && <span aria-hidden>·</span>}
            {durationSec > 0 && <span>{formatDuration(durationSec)}</span>}
            <span aria-hidden>·</span>
            <span>{segments} segments</span>
          </div>
        </div>

        <div className="flex flex-wrap gap-3">
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
