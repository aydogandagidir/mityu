'use client';

/**
 * One approved action — DESIGN_SYSTEM.md §5.15, §6.4.
 *
 * 🔒 ADR-0025: this card is READ-ONLY. It carries no complete, no snooze, no due-date
 * editor and no overdue styling. Approval is a review state, not a work state, and the
 * moment this screen grows a checkbox it starts making a claim about work it cannot see.
 *
 * 🔒 The source control keeps its accessible name `Open source in ${title} at ${time}`.
 *
 * THE TIMESTAMP PROBLEM, HANDLED. `audioStartTime` is a real offset into the recording
 * and formats as `mm:ss`. `sourceTimestamp` is the worker's UTC fallback — an absolute
 * date. Printing the latter where the former belongs tells the reader an action came
 * from four minutes into a meeting when it did not. So a non-clock value is never shown
 * as one: the chip reads `Source`, and a sentence beside it says the position in the
 * recording was not stored.
 */

import React from 'react';
import { CalendarDays, UserRound } from 'lucide-react';
import { SourceChip } from '@/components/report/primitives';
import { StatusPill } from '@/components/report/StatusPill';
import { Card } from '@/components/ui/card';
import type { ApprovedActionItem } from '@/services/actionCenterService';

/** `mm:ss` / `h:mm:ss` from a second offset, or null when there is no offset. */
export function formatRecordingTime(seconds: number | null): string | null {
  if (seconds === null || !Number.isFinite(seconds) || seconds < 0) return null;
  const total = Math.floor(seconds);
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  if (h > 0) return [h, m, s].map((p) => p.toString().padStart(2, '0')).join(':');
  return `${m.toString().padStart(2, '0')}:${s.toString().padStart(2, '0')}`;
}

export function formatMeetingDate(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return new Intl.DateTimeFormat(undefined, { dateStyle: 'medium' }).format(date);
}

export interface ApprovedActionCardProps {
  item: ApprovedActionItem;
  onOpenSource: (item: ApprovedActionItem) => void;
}

export function ApprovedActionCard({ item, onOpenSource }: ApprovedActionCardProps) {
  const recordingTime = formatRecordingTime(item.audioStartTime);
  // 🔒 The accessible name keeps the value it always had, including the fallback —
  // it names the target, it is not a claim about a position in the audio.
  const sourceLabel = recordingTime || item.sourceTimestamp;

  return (
    <Card className="p-4">
      <div className="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
        <div className="min-w-0 flex-1 space-y-2">
          <div className="flex flex-wrap items-center gap-x-2 gap-y-1 text-caption text-subtle-foreground">
            <span className="min-w-0 max-w-full break-words font-medium text-foreground">
              {item.meetingTitle}
            </span>
            <span aria-hidden="true">·</span>
            <span>{formatMeetingDate(item.meetingCreatedAt)}</span>
            <StatusPill status="approved" />
          </div>

          <p className="whitespace-pre-wrap break-words text-read text-foreground">
            {item.text}
          </p>

          {(item.assignee || item.due) && (
            <dl className="flex flex-wrap gap-2 text-caption">
              {item.assignee && (
                <div className="inline-flex min-w-0 max-w-full items-center gap-1.5 rounded-sm border border-border bg-surface-2 px-2 py-1">
                  <UserRound className="size-3.5 text-subtle-foreground" aria-hidden="true" />
                  <dt className="shrink-0 text-subtle-foreground">Assignee</dt>
                  <dd className="min-w-0 break-words text-foreground">{item.assignee}</dd>
                </div>
              )}
              {item.due && (
                <div className="inline-flex min-w-0 max-w-full items-center gap-1.5 rounded-sm border border-border bg-surface-2 px-2 py-1">
                  <CalendarDays className="size-3.5 text-subtle-foreground" aria-hidden="true" />
                  <dt className="shrink-0 text-subtle-foreground">Due</dt>
                  <dd className="min-w-0 break-words text-foreground">{item.due}</dd>
                </div>
              )}
            </dl>
          )}

          {!recordingTime && (
            <p className="text-caption text-subtle-foreground">
              This recording was saved before segment positions were stored, so the source
              opens the segment without a time.
            </p>
          )}
        </div>

        <SourceChip
          timestamp={recordingTime}
          ariaLabel={`Open source in ${item.meetingTitle} at ${sourceLabel}`}
          onClick={() => onOpenSource(item)}
          className="shrink-0"
        />
      </div>
    </Card>
  );
}

export default ApprovedActionCard;
