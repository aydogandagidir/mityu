import { describe, expect, it } from 'vitest';
import { formatMeetingDate, groupMeetingsByDate } from './meetingDates';

// Wednesday 5 Nov 2026, 15:00 local time.
const NOW = new Date(2026, 10, 5, 15, 0, 0);
const at = (y: number, m: number, d: number, h = 10, min = 0) =>
  new Date(y, m, d, h, min).toISOString();

describe('groupMeetingsByDate', () => {
  it('returns no groups for an empty list', () => {
    expect(groupMeetingsByDate([], NOW)).toEqual([]);
  });

  it('groups by Today / Yesterday / Previous 7 days / Earlier, newest first', () => {
    const rows = [
      { id: 'old', title: 'Old', createdAt: at(2026, 9, 1) },
      { id: 'today', title: 'Today', createdAt: at(2026, 10, 5, 9) },
      { id: 'week', title: 'Week', createdAt: at(2026, 10, 1) },
      { id: 'yday', title: 'Yesterday', createdAt: at(2026, 10, 4, 23, 30) },
    ];
    const groups = groupMeetingsByDate(rows, NOW);
    expect(groups.map((g) => g.label)).toEqual(['Today', 'Yesterday', 'Previous 7 days', 'Earlier']);
    expect(groups.map((g) => g.rows.map((r) => r.id))).toEqual([['today'], ['yday'], ['week'], ['old']]);
  });

  it('treats midnight as a calendar boundary, not a 24h window', () => {
    const groups = groupMeetingsByDate(
      [{ id: 'early', title: 'x', createdAt: at(2026, 10, 5, 0, 1) }],
      NOW
    );
    expect(groups[0].label).toBe('Today');
  });

  it('keeps the old single group when the payload carries no dates (older backend)', () => {
    const groups = groupMeetingsByDate(
      [
        { id: 'a', title: 'A' },
        { id: 'b', title: 'B' },
      ],
      NOW
    );
    expect(groups).toEqual([{ label: 'All meetings', rows: [{ id: 'a', title: 'A' }, { id: 'b', title: 'B' }] }]);
  });

  it('puts unparseable or missing dates last, in an Undated group, without dropping rows', () => {
    const groups = groupMeetingsByDate(
      [
        { id: 'bad', title: 'x', createdAt: 'not-a-date' },
        { id: 'ok', title: 'y', createdAt: at(2026, 10, 5) },
        { id: 'none', title: 'z' },
      ],
      NOW
    );
    expect(groups.map((g) => g.label)).toEqual(['Today', 'Undated']);
    expect(groups[1].rows.map((r) => r.id)).toEqual(['bad', 'none']);
  });

  it('files a clock-skewed future date under Today rather than losing it', () => {
    const groups = groupMeetingsByDate([{ id: 'f', title: 'x', createdAt: at(2026, 10, 9) }], NOW);
    expect(groups[0].label).toBe('Today');
  });
});

describe('formatMeetingDate', () => {
  it('returns null for missing or invalid input', () => {
    expect(formatMeetingDate(undefined, NOW)).toBeNull();
    expect(formatMeetingDate('', NOW)).toBeNull();
    expect(formatMeetingDate('nope', NOW)).toBeNull();
  });

  it('shows a time for today', () => {
    const label = formatMeetingDate(at(2026, 10, 5, 9, 5), NOW);
    expect(label).toMatch(/9|09/);
    expect(label).toMatch(/05/);
  });

  it('says Yesterday for yesterday', () => {
    expect(formatMeetingDate(at(2026, 10, 4), NOW)).toBe('Yesterday');
  });

  it('shows a date (with a day number) for anything older', () => {
    expect(formatMeetingDate(at(2026, 9, 1), NOW)).toMatch(/1/);
    expect(formatMeetingDate(at(2026, 9, 1), NOW)).not.toBe('Yesterday');
  });

  it('adds the year only for another calendar year', () => {
    expect(formatMeetingDate(at(2025, 0, 15), NOW)).toMatch(/2025/);
    expect(formatMeetingDate(at(2026, 0, 15), NOW)).not.toMatch(/2026/);
  });
});

