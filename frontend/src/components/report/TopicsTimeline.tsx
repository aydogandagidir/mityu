'use client';

/**
 * TopicsTimeline — on-device chapter strip for the meeting report (Phase B,
 * docs/DESIGN_READAI.md). v1 derives chapters deterministically from the local
 * transcript: split on long silence gaps, merge slivers, cap the count. No LLM,
 * nothing leaves the device; labels are the chapter's opening words (verbatim
 * transcript, so no AI-generated-content labeling is required).
 *
 * Clicking a chapter jumps the transcript to its first segment via the existing
 * jump-to-source mechanism.
 */

import { Hash } from 'lucide-react';
import type { Transcript } from '@/types';

export interface Chapter {
  label: string;
  startSec: number;
  endSec: number;
  firstSegmentId: string;
}

function fmt(sec: number): string {
  const m = Math.floor(sec / 60);
  const s = Math.floor(sec % 60);
  const h = Math.floor(m / 60);
  if (h > 0) return `${h}:${String(m % 60).padStart(2, '0')}:${String(s).padStart(2, '0')}`;
  return `${m}:${String(s).padStart(2, '0')}`;
}

/** Pause-based chapter detection. Pure + deterministic. */
export function deriveChapters(transcripts: Transcript[], maxChapters = 8): Chapter[] {
  const segs = transcripts
    .filter((t) => t.audio_start_time != null && t.audio_end_time != null)
    .sort((a, b) => (a.audio_start_time! - b.audio_start_time!));
  if (segs.length < 4) return [];

  // Grow the gap threshold until the chapter count fits.
  let gap = 45;
  let chapters: Chapter[] = [];
  for (let attempt = 0; attempt < 6; attempt++) {
    chapters = [];
    let cur: Transcript[] = [segs[0]];
    for (let i = 1; i < segs.length; i++) {
      const silence = segs[i].audio_start_time! - segs[i - 1].audio_end_time!;
      if (silence >= gap) {
        chapters.push(toChapter(cur));
        cur = [];
      }
      cur.push(segs[i]);
    }
    chapters.push(toChapter(cur));
    if (chapters.length <= maxChapters) break;
    gap *= 1.6;
  }

  // Merge slivers (<45s) into their predecessor so the strip stays legible.
  const merged: Chapter[] = [];
  for (const c of chapters) {
    const prev = merged[merged.length - 1];
    if (prev && c.endSec - c.startSec < 45) {
      prev.endSec = c.endSec;
    } else {
      merged.push({ ...c });
    }
  }
  return merged.length >= 2 ? merged.slice(0, maxChapters) : [];
}

function toChapter(segs: Transcript[]): Chapter {
  const first = segs.find((s) => (s.text ?? '').trim().length > 12) ?? segs[0];
  const words = (first.text ?? '').trim().split(/\s+/).slice(0, 5).join(' ');
  return {
    label: words || 'Chapter',
    startSec: segs[0].audio_start_time!,
    endSec: segs[segs.length - 1].audio_end_time!,
    firstSegmentId: first.id,
  };
}

export function TopicsTimeline({
  transcripts,
  onJumpToSegment,
}: {
  transcripts: Transcript[];
  /** Receives the chapter's first segment id AND its start second (scroll + seek). */
  onJumpToSegment?: (segmentId: string, startSec: number) => void;
}) {
  const chapters = deriveChapters(transcripts);
  if (chapters.length === 0) return null;
  const total = Math.max(...chapters.map((c) => c.endSec));
  if (total < 120) return null; // too short for chapters to mean anything

  return (
    <div className="border-b border-border bg-background px-6 py-2.5">
      <div className="flex items-center gap-3">
        <span className="inline-flex shrink-0 items-center gap-1.5 text-[11px] font-medium uppercase tracking-widest text-muted-foreground">
          <Hash className="h-3.5 w-3.5" aria-hidden />
          Chapters
        </span>
        {/* Chapter chips: content-hugging (proportional widths made short chapters
            unreadable), horizontally scrollable when they overflow; click → jump. */}
        <div
          className="flex flex-1 items-center gap-1.5 overflow-x-auto [scrollbar-width:thin]"
          role="list"
          aria-label="Meeting chapters"
        >
          {chapters.map((c, i) => (
            <button
              key={`${c.firstSegmentId}-${i}`}
              role="listitem"
              type="button"
              onClick={() => onJumpToSegment?.(c.firstSegmentId, c.startSec)}
              title={`${fmt(c.startSec)} — ${c.label}`}
              className="group inline-flex h-6 shrink-0 items-center gap-1.5 rounded-full border border-border bg-card pl-2 pr-2.5 transition-colors hover:border-primary/40 hover:bg-accent"
            >
              <span className="text-[11px] tabular-nums text-primary">{fmt(c.startSec)}</span>
              <span className="max-w-[180px] truncate text-[11px] text-muted-foreground group-hover:text-foreground">
                {c.label}
              </span>
            </button>
          ))}
        </div>
      </div>
    </div>
  );
}
