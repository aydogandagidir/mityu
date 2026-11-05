/**
 * Calendar grouping and short date labels for meeting lists — DESIGN_SYSTEM.md §5.2.
 *
 * `api_get_meetings` carries `created_at` since v1.2.6 (the database always had it; the
 * list payload dropped it). Dates come ONLY from that column, never from the generated
 * title: a renamed meeting keeps its date, and a title that merely looks like a date
 * cannot invent one. Pure functions with an injectable `now`, so they are testable and
 * the grouping is decided in local calendar days (midnight is the boundary, not 24h).
 */

export interface DatedMeeting {
  id: string;
  title: string;
  /** RFC 3339 / ISO 8601 creation time from the database, when the backend sends it. */
  createdAt?: string;
}

export interface MeetingGroup<T extends DatedMeeting> {
  label: string;
  rows: T[];
}

/** Ordered calendar buckets; `Undated` is always last. */
const GROUP_LABELS = ['Today', 'Yesterday', 'Previous 7 days', 'Earlier'] as const;
const UNDATED_LABEL = 'Undated';
/** The pre-v1.2.6 single group, kept for a backend that sends no dates at all. */
const ALL_MEETINGS_LABEL = 'All meetings';
/** Days back (from today's midnight) that still count as "Previous 7 days". */
const RECENT_WINDOW_DAYS = 7;
const MS_PER_DAY = 24 * 60 * 60 * 1000;

function parseDate(value: string | undefined): Date | null {
  if (!value) return null;
  const date = new Date(value);
  return Number.isNaN(date.getTime()) ? null : date;
}

function startOfDay(date: Date): number {
  return new Date(date.getFullYear(), date.getMonth(), date.getDate()).getTime();
}

/** Whole local calendar days between `date` and `now` (0 = today, negative = future). */
function calendarDaysAgo(date: Date, now: Date): number {
  return Math.round((startOfDay(now) - startOfDay(date)) / MS_PER_DAY);
}

function bucketFor(date: Date, now: Date): (typeof GROUP_LABELS)[number] {
  const days = calendarDaysAgo(date, now);
  // A clock-skewed future date is still the newest thing the user has.
  if (days <= 0) return 'Today';
  if (days === 1) return 'Yesterday';
  if (days <= RECENT_WINDOW_DAYS) return 'Previous 7 days';
  return 'Earlier';
}

/**
 * Group meetings into calendar buckets, preserving the input order inside each bucket
 * (the backend already returns newest first). Empty buckets are omitted.
 */
export function groupMeetingsByDate<T extends DatedMeeting>(
  meetings: T[],
  now: Date = new Date()
): MeetingGroup<T>[] {
  if (meetings.length === 0) return [];
  if (!meetings.some((m) => parseDate(m.createdAt))) {
    return [{ label: ALL_MEETINGS_LABEL, rows: meetings }];
  }

  const buckets = new Map<string, T[]>();
  for (const meeting of meetings) {
    const date = parseDate(meeting.createdAt);
    const label = date ? bucketFor(date, now) : UNDATED_LABEL;
    const rows = buckets.get(label);
    if (rows) rows.push(meeting);
    else buckets.set(label, [meeting]);
  }

  return [...GROUP_LABELS, UNDATED_LABEL]
    .filter((label) => buckets.has(label))
    .map((label) => ({ label, rows: buckets.get(label)! }));
}

/**
 * Short, glanceable label: a time for today, "Yesterday", otherwise a day and month
 * (plus the year only when it differs from this one). `null` when there is no usable
 * date — callers print nothing rather than a guess.
 */
export function formatMeetingDate(
  createdAt: string | undefined,
  now: Date = new Date()
): string | null {
  const date = parseDate(createdAt);
  if (!date) return null;
  const days = calendarDaysAgo(date, now);
  if (days <= 0) {
    return date.toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit' });
  }
  if (days === 1) return 'Yesterday';
  return date.toLocaleDateString(undefined, {
    day: 'numeric',
    month: 'short',
    ...(date.getFullYear() !== now.getFullYear() ? { year: 'numeric' } : {}),
  });
}

