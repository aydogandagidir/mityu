'use client';

/**
 * /design/shell — the application shell, Tauri-free (DESIGN_SYSTEM.md §11.1).
 *
 * WHY IT PROVIDES CONTEXTS BY HAND. `AppRail` and `MeetingsPane` are the REAL shipping
 * components, not mocks — that is the whole point of shooting them. They read four
 * contexts whose providers each call `invoke()` on mount, which throws outside the Tauri
 * runtime, so this page supplies literal context values instead. Product code still uses
 * the hooks, and the hooks still throw outside a provider; only the fixtures reach for
 * the context objects.
 *
 * The casts below are deliberate and confined to this file: a fixture needs the handful
 * of fields the shell actually reads, not a second implementation of four providers.
 *
 * Shot at three widths (900 / 1100 / 1440) because the pane's default follows the
 * window, and at 900 the content pane is the thing that must not collapse.
 */

import React, { useMemo, useState } from 'react';
import { AppRail } from '@/components/shell/AppRail';
import { MeetingsPane } from '@/components/shell/MeetingsPane';
import { PageHeader } from '@/components/shell/PageHeader';
import {
  SidebarContext,
  type SidebarContextType,
  type CurrentMeeting,
} from '@/components/Sidebar/SidebarProvider';
import {
  RecordingStateContext,
  RecordingStatus,
  type RecordingStateContextType,
} from '@/contexts/RecordingStateContext';
import { ConfigContext, type ConfigContextType } from '@/contexts/ConfigContext';
import {
  ImportDialogContext,
  type ImportDialogContextType,
} from '@/contexts/ImportDialogContext';
import { TooltipProvider } from '@/components/ui/tooltip';
import { Button } from '@/components/ui/button';
import { StatTile } from '@/components/ui/stat-tile';
import { SpeakerChips, TalkTimePanel } from '@/components/report/SpeakerTurns';
import { type SpeakerTurn } from '@/lib/speakerTurns';
import { Clock, Cpu, FileText, ListChecks, Quote, Sparkles, Users } from 'lucide-react';
import { cn } from '@/lib/utils';
import { focusRing } from '@/components/ui/focus-ring';

const SHELL_TURNS: SpeakerTurn[] = [
  { start_ms: 0, end_ms: 14_000, speaker_label: 'Speaker 1', confidence: 0.96 },
  { start_ms: 14_000, end_ms: 42_000, speaker_label: 'Speaker 2', confidence: 0.95 },
  { start_ms: 40_000, end_ms: 58_000, speaker_label: 'Speaker 3', confidence: 0.94 },
  { start_ms: 58_000, end_ms: 74_000, speaker_label: 'Speaker 4', confidence: 0.95 },
];

const MEETINGS: CurrentMeeting[] = [
  { id: 'meeting-0001-ankara', title: 'Ankara site visit — retention policy' },
  { id: 'meeting-0002-pricing', title: 'Q3 pricing and packaging' },
  { id: 'meeting-0003-security', title: 'Security review kick-off' },
  { id: 'meeting-0004-vendor', title: 'Vendor call — conveyor line quote' },
];

const EVIDENCE_SEGMENTS = [
  {
    id: 'seg-1',
    time: '00:04',
    speakers: ['Speaker 1'],
    text: 'We agreed the Ankara pilot starts first so the regional compliance sign-off is isolated.',
    highlighted: false,
    crosstalk: false,
  },
  {
    id: 'seg-2',
    time: '00:14',
    speakers: ['Speaker 2'],
    text: 'Retention stays locked at 90 days on local encrypted storage until the audit closes.',
    highlighted: true,
    crosstalk: false,
  },
  {
    id: 'seg-3',
    time: '00:40',
    speakers: ['Speaker 2', 'Speaker 3'],
    text: 'Can we share the retention schedule with the Ankara operations lead before Friday?',
    highlighted: false,
    crosstalk: true,
  },
];

function EvidenceDrawerSample() {
  return (
    <aside
      aria-label="Transcript"
      className="flex w-72 shrink-0 flex-col border-l border-border bg-card"
    >
      <header className="flex items-center justify-between border-b border-border bg-surface-2/50 px-3 py-2.5">
        <div className="space-y-0.5">
          <div className="text-eyebrow uppercase text-subtle-foreground">Zone 4 · Evidence</div>
          <h3 className="text-label font-semibold text-foreground">Transcript drawer</h3>
        </div>
        <span className="inline-flex items-center gap-1 rounded-full border border-verified-border bg-verified-surface px-2 py-0.5 font-mono text-micro font-medium tabular-nums text-verified-ink">
          <Quote className="h-2.5 w-2.5" aria-hidden />
          Source · 00:14
        </span>
      </header>
      <div className="flex-1 space-y-2 overflow-y-auto p-3">
        {EVIDENCE_SEGMENTS.map((seg) => (
          <div
            key={seg.id}
            className={cn(
              'rounded-md border p-2.5 transition-colors duration-fast',
              seg.highlighted
                ? 'border-verified-border border-l-2 border-l-primary bg-accent/60 shadow-elev-0'
                : 'border-border/60 bg-surface-2/30'
            )}
          >
            <div className="mb-1 flex items-center justify-between gap-1.5">
              <span className="font-mono text-micro font-medium tabular-nums text-primary-ink">
                [{seg.time}]
              </span>
              <SpeakerChips
                speakers={seg.speakers}
                order={['Speaker 1', 'Speaker 2', 'Speaker 3', 'Speaker 4']}
                crosstalk={seg.crosstalk}
              />
            </div>
            <p className="text-caption leading-snug text-foreground">{seg.text}</p>
          </div>
        ))}
      </div>
      <footer className="border-t border-border bg-surface-2/40 px-3 py-2 text-micro text-muted-foreground">
        Non-modal evidence drawer (`modal=false`) keeps the report interactive while verifying citations.
      </footer>
    </aside>
  );
}

function ShellHarness({
  recording = false,
  paneOpen = true,
  drawerOpen = true,
  children,
}: {
  recording?: boolean;
  paneOpen?: boolean;
  drawerOpen?: boolean;
  children: React.ReactNode;
}) {
  const [collapsed, setCollapsed] = useState(!paneOpen);
  const [current, setCurrent] = useState<CurrentMeeting | null>(MEETINGS[0]);

  const sidebar = useMemo(
    () =>
      ({
        currentMeeting: current,
        setCurrentMeeting: setCurrent,
        sidebarItems: [],
        isCollapsed: collapsed,
        toggleCollapse: () => setCollapsed((c) => !c),
        meetings: MEETINGS,
        setMeetings: () => {},
        isMeetingActive: false,
        setIsMeetingActive: () => {},
        handleRecordingToggle: () => {},
        ready: true,
        searchTranscripts: () => {},
        searchResults: [],
        isSearching: false,
        searchError: null,
        setServerAddress: () => {},
        serverAddress: '',
        transcriptServerAddress: '',
        setTranscriptServerAddress: () => {},
        activeSummaryPolls: new Map(),
        startSummaryPolling: () => {},
        stopSummaryPolling: () => {},
        refetchMeetings: async () => {},
      }) as SidebarContextType,
    [collapsed, current]
  );

  const recordingState = useMemo(
    () =>
      ({
        isRecording: recording,
        isPaused: false,
        isActive: recording,
        recordingDuration: recording ? 754 : null,
        activeDuration: recording ? 754 : null,
        status: recording ? RecordingStatus.RECORDING : RecordingStatus.IDLE,
        setStatus: () => {},
        isStopping: false,
        isProcessing: false,
        isSaving: false,
      }) as RecordingStateContextType,
    [recording]
  );

  const config = useMemo(
    () =>
      ({
        betaFeatures: { importAndRetranscribe: true },
      }) as unknown as ConfigContextType,
    []
  );

  const importDialog = useMemo(
    () => ({ openImportDialog: () => {} }) as unknown as ImportDialogContextType,
    []
  );

  return (
    <SidebarContext.Provider value={sidebar}>
      <RecordingStateContext.Provider value={recordingState}>
        <ConfigContext.Provider value={config}>
          <ImportDialogContext.Provider value={importDialog}>
            <TooltipProvider>
              <div className="flex h-[580px] overflow-hidden rounded-xl border border-border bg-card shadow-elev-2">
                <AppRail />
                <MeetingsPane />
                <main className="flex min-w-0 flex-1 flex-col overflow-hidden bg-background">
                  {children}
                </main>
                {drawerOpen && <EvidenceDrawerSample />}
              </div>
            </TooltipProvider>
          </ImportDialogContext.Provider>
        </ConfigContext.Provider>
      </RecordingStateContext.Provider>
    </SidebarContext.Provider>
  );
}

function ContentSample() {
  return (
    <div className="flex min-h-0 flex-1 flex-col overflow-y-auto">
      <PageHeader
        eyebrow="Meeting report"
        title="Ankara site visit — retention policy"
        meta="Tue 9 Sep · 1h 24m · 146 segments"
        actions={
          <>
            <Button variant="outline" size="sm">
              Export
            </Button>
            <Button variant="verified" size="sm">
              Approve summary
            </Button>
          </>
        }
      />
      <div className="space-y-4 p-gutter">
        <div className="flex flex-wrap gap-3">
          <StatTile icon={Clock} label="Duration" value="1h 24m" />
          <StatTile icon={FileText} label="Segments" value="146" />
          <StatTile icon={Users} label="Speakers" value="4" sub="Nemotron 3.5" />
          <StatTile icon={ListChecks} label="Approved" value="3 / 7" />
        </div>

        {/* Source-linked AI draft block connected to Zone 4 Evidence Drawer */}
        <div className="space-y-2 rounded-lg border border-border bg-card p-3.5 shadow-elev-0">
          <div className="flex flex-wrap items-center justify-between gap-2">
            <span className="inline-flex items-center gap-1.5 rounded-md border border-ai-border bg-ai-surface px-2 py-0.5 text-caption font-medium text-ai-ink">
              <Sparkles className="h-3 w-3 shrink-0" aria-hidden />
              AI-generated · review required
            </span>
            <button
              type="button"
              aria-label="Jump to source transcript segment at 00:14"
              className={cn(
                'inline-flex min-h-[28px] items-center gap-1 rounded-full border border-verified-border bg-verified-surface px-2.5 py-0.5 font-mono text-caption font-medium tabular-nums text-verified-ink transition-colors duration-fast ease-out hover:bg-verified-surface/80',
                focusRing('card')
              )}
            >
              <Quote className="h-2.5 w-2.5" aria-hidden />
              Source · 00:14
            </button>
          </div>
          <p className="text-body text-foreground">
            The Ankara site pilot launches first with a strict 90-day local encrypted retention
            window before expanding to the second facility.
          </p>
        </div>

        <TalkTimePanel
          state={{ kind: 'done', diarizedAt: '2026-11-01T10:00:00Z', turns: SHELL_TURNS }}
        />

        <div className="space-y-2 rounded-lg border border-border bg-card p-3.5 shadow-elev-0">
          <div className="flex flex-wrap items-center justify-between gap-2">
            <span className="inline-flex items-center gap-1.5 text-label font-semibold text-foreground">
              <Cpu className="h-3.5 w-3.5 text-primary-ink" aria-hidden />
              Live speaker attribution · NVIDIA Nemotron 3.5 Transcribe
            </span>
            <SpeakerChips
              speakers={['Speaker 2', 'Speaker 3']}
              order={['Speaker 1', 'Speaker 2', 'Speaker 3', 'Speaker 4']}
              crosstalk
            />
          </div>
          <p className="text-body text-muted-foreground">
            The 4-zone desktop shell isolates navigation (`AppRail`), library search
            (`MeetingsPane`), primary reading (`Workspace`), and source-linked transcript
            verification (`EvidenceDrawer`) with zero layout shift.
          </p>
        </div>
      </div>
    </div>
  );
}

function Panel({ theme }: { theme: 'light' | 'dark' }) {
  return (
    <div className={theme === 'dark' ? 'dark' : ''}>
      <div className="space-y-6 bg-background p-6 text-foreground">
        <header className="space-y-1">
          <div className="text-eyebrow uppercase text-subtle-foreground">{theme} theme</div>
          <h1 className="text-display">Application shell</h1>
          <p className="text-body text-muted-foreground">
            4-Zone Desktop Architecture: §5.1 AppRail · §5.2 MeetingsPane · §5.3 Workspace · §6.3
            EvidenceDrawer. Real components, fixture data.
          </p>
        </header>

        <section className="space-y-2">
          <h2 className="text-title">4-Zone shell · Pane open · Evidence drawer active</h2>
          <ShellHarness drawerOpen>
            <ContentSample />
          </ShellHarness>
        </section>

        <section className="space-y-2">
          <h2 className="text-title">Pane closed · recording active · Evidence drawer open</h2>
          <ShellHarness paneOpen={false} recording drawerOpen>
            <ContentSample />
          </ShellHarness>
        </section>

        {/* The rail is icons plus tooltips, so its labels live in `aria-label`. This
            legend puts the same names on screen, which is what makes a screenshot of
            an icon rail reviewable at all. */}
        <section className="space-y-2">
          <h2 className="text-title">Rail labels</h2>
          <ul className="grid grid-cols-2 gap-x-6 gap-y-1 text-body text-muted-foreground sm:grid-cols-3">
            <li>About Mityu</li>
            <li>Show meetings / Hide meetings</li>
            <li>Home</li>
            <li>Actions</li>
            <li>Start recording (Record)</li>
            <li>Import audio</li>
            <li>Search meetings</li>
            <li>Settings</li>
          </ul>
          <p className="text-caption text-subtle-foreground">
            Pane search field: “Search meeting evidence…”
          </p>
        </section>
      </div>
    </div>
  );
}

export default function DesignShellPage() {
  return (
    // Stacked, not side by side: the shell is 336px of rail + pane before any content,
    // so two panels in one row leave a content pane narrower than the app can ever be
    // and every overflow they show would be an artifact of the fixture.
    <div>
      <Panel theme="light" />
      <Panel theme="dark" />
    </div>
  );
}
