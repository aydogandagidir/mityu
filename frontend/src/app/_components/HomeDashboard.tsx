'use client';

/**
 * HomeDashboard — Phase C (docs/DESIGN_READAI.md): the "For You" analogue, shown on
 * the home route while idle (no recording in progress, no live transcript). Recent
 * meetings render as report cards; clicking one opens its meeting report. Purely
 * on-device: it reads the already-loaded meetings list, nothing else.
 *
 * The wire (`api_get_meetings`) carries only { id, title }, so the date meta is
 * best-effort parsed from auto-generated titles ("Meeting YYYY-MM-DD_HH-MM-SS");
 * renamed meetings simply show no date. Deliberately no fabricated metrics.
 */

import { useEffect, useState } from 'react';
import { useRouter } from 'next/navigation';
import { ChevronRight, FileText, ListChecks, Mic, NotebookPen } from 'lucide-react';
import { useSidebar } from '@/components/Sidebar/SidebarProvider';
import { summaryDraftService, OpenActionItem } from '@/services/summaryDraftService';
import { isTauri } from '@/lib/isTauri';

function parseTitleDate(title: string): string | null {
  const m = title.match(/(\d{4})-(\d{2})-(\d{2})_(\d{2})-(\d{2})-(\d{2})/);
  if (!m) return null;
  const d = new Date(
    Number(m[1]), Number(m[2]) - 1, Number(m[3]),
    Number(m[4]), Number(m[5]), Number(m[6]),
  );
  if (Number.isNaN(d.getTime())) return null;
  return d.toLocaleDateString(undefined, {
    weekday: 'short', day: 'numeric', month: 'short',
  }) + ' · ' + d.toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit' });
}

export function HomeDashboard() {
  const router = useRouter();
  const { meetings, setCurrentMeeting } = useSidebar();

  const recent = meetings.slice(0, 9);

  // Open action items across meetings (api_get_open_action_items). Failure is
  // silent by design — the dashboard degrades to meetings-only.
  const [actionItems, setActionItems] = useState<OpenActionItem[]>([]);
  useEffect(() => {
    if (!isTauri()) return;
    let cancelled = false;
    summaryDraftService
      .getOpenActionItems(8)
      .then((items) => { if (!cancelled) setActionItems(items); })
      .catch(() => {});
    return () => { cancelled = true; };
  }, [meetings.length]);

  const openMeeting = (id: string, title: string) => {
    setCurrentMeeting({ id, title });
    router.push(`/meeting-details?id=${id}`);
  };

  return (
    <div className="flex-1 overflow-y-auto">
      <div className="mx-auto w-full max-w-4xl px-8 py-10">
        {/* Hero */}
        <div className="mb-8">
          <h1 className="text-2xl font-semibold tracking-tight text-foreground">
            Welcome back
          </h1>
          <p className="mt-1 flex items-center gap-1.5 text-sm text-muted-foreground">
            <Mic className="h-3.5 w-3.5" aria-hidden />
            Start a recording below, or pick up where you left off.
          </p>
        </div>

        {recent.length > 0 && (
          <section>
            <div className="mb-3 flex items-center gap-2">
              <NotebookPen className="h-4 w-4 text-muted-foreground" aria-hidden />
              <h2 className="text-sm font-semibold text-foreground">Recent meetings</h2>
              <span className="rounded-full bg-muted px-2 py-0.5 text-xs tabular-nums text-muted-foreground">
                {meetings.length}
              </span>
            </div>

            <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
              {recent.map((m) => {
                const date = parseTitleDate(m.title);
                return (
                  <button
                    key={m.id}
                    type="button"
                    onClick={() => openMeeting(m.id, m.title)}
                    className="group flex items-start gap-3 rounded-2xl border border-border bg-card p-4 text-left shadow-sm transition-all hover:-translate-y-0.5 hover:border-primary/40 hover:shadow-md"
                  >
                    <span className="grid h-9 w-9 shrink-0 place-items-center rounded-lg bg-accent text-accent-foreground transition-colors group-hover:bg-primary group-hover:text-primary-foreground">
                      <FileText className="h-4 w-4" aria-hidden />
                    </span>
                    <span className="min-w-0 flex-1">
                      <span className="block truncate text-sm font-medium text-foreground">
                        {m.title}
                      </span>
                      <span className="mt-0.5 block text-xs text-muted-foreground">
                        {date ?? 'Meeting report'}
                      </span>
                    </span>
                    <ChevronRight
                      className="mt-1 h-4 w-4 shrink-0 text-muted-foreground/50 transition-all group-hover:translate-x-0.5 group-hover:text-primary"
                      aria-hidden
                    />
                  </button>
                );
              })}
            </div>
          </section>
        )}

        {actionItems.length > 0 && (
          <section className="mt-8">
            <div className="mb-3 flex items-center gap-2">
              <ListChecks className="h-4 w-4 text-muted-foreground" aria-hidden />
              <h2 className="text-sm font-semibold text-foreground">Open action items</h2>
              <span className="rounded-full bg-muted px-2 py-0.5 text-xs tabular-nums text-muted-foreground">
                {actionItems.length}
              </span>
            </div>
            <div className="overflow-hidden rounded-2xl border border-border bg-card shadow-sm">
              <ul className="divide-y divide-border">
                {actionItems.map((item) => (
                  <li key={item.id}>
                    <button
                      type="button"
                      onClick={() => openMeeting(item.meeting_id, item.meeting_title)}
                      className="group flex w-full items-start gap-3 px-4 py-3 text-left transition-colors hover:bg-muted/40"
                    >
                      <span
                        className={`mt-1.5 h-2 w-2 shrink-0 rounded-full ${
                          item.status === 'approved' ? 'bg-green-500' : 'bg-primary'
                        }`}
                        title={item.status === 'approved' ? 'Approved' : 'Awaiting review'}
                      />
                      <span className="min-w-0 flex-1">
                        <span className="block truncate text-sm text-foreground">{item.text}</span>
                        <span className="mt-0.5 block truncate text-xs text-muted-foreground">
                          {item.meeting_title}
                          {item.due ? ` · due ${item.due}` : ''}
                        </span>
                      </span>
                      <ChevronRight className="mt-1 h-4 w-4 shrink-0 text-muted-foreground/50 transition-all group-hover:translate-x-0.5 group-hover:text-primary" aria-hidden />
                    </button>
                  </li>
                ))}
              </ul>
            </div>
          </section>
        )}
      </div>
    </div>
  );
}
