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
import { Clock, FileText, ListChecks } from 'lucide-react';

const MEETINGS: CurrentMeeting[] = [
  { id: 'meeting-0001-ankara', title: 'Ankara site visit — retention policy' },
  { id: 'meeting-0002-pricing', title: 'Q3 pricing and packaging' },
  { id: 'meeting-0003-security', title: 'Security review kick-off' },
  { id: 'meeting-0004-vendor', title: 'Vendor call — conveyor line quote' },
];

function ShellHarness({
  recording = false,
  paneOpen = true,
  children,
}: {
  recording?: boolean;
  paneOpen?: boolean;
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
              <div className="flex h-[560px] overflow-hidden rounded-lg border border-border">
                <AppRail />
                <MeetingsPane />
                <main className="flex min-w-0 flex-1 flex-col overflow-hidden bg-background">
                  {children}
                </main>
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
          <StatTile icon={ListChecks} label="Approved" value="3 / 7" />
        </div>
        <p className="max-w-measure text-read text-muted-foreground">
          The content pane owns its own scroll and its own header. The rail never moves,
          and the meetings pane is the user&rsquo;s choice — opening it narrows this pane
          rather than pushing the window around.
        </p>
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
            §5.1 rail · §5.2 meetings pane · §5.3 page header. Real components, fixture
            data.
          </p>
        </header>

        <section className="space-y-2">
          <h2 className="text-title">Pane open · idle</h2>
          <ShellHarness>
            <ContentSample />
          </ShellHarness>
        </section>

        <section className="space-y-2">
          <h2 className="text-title">Pane closed · recording</h2>
          <ShellHarness paneOpen={false} recording>
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
