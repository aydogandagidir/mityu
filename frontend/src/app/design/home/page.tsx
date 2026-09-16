'use client';

/**
 * /design/home — Home's review queue, Tauri-free (DESIGN_SYSTEM.md §11.1).
 *
 * The real `HomeDashboard` with an injected service, so every state can be seen without
 * the desktop app: the queue with drafts, the load failure, the "nothing waiting" case
 * and the first run. The injected service resolves `true` so the optimistic path
 * settles, and one item's reject resolves `false` so the revert is visible too.
 */

import React, { useMemo } from 'react';
import type { OpenActionItem } from '@/services/summaryDraftService';
import { HomeDashboard } from '@/app/_components/HomeDashboard';
import {
  SidebarContext,
  type SidebarContextType,
  type CurrentMeeting,
} from '@/components/Sidebar/SidebarProvider';

const MEETINGS: CurrentMeeting[] = [
  { id: 'meeting-0001-ankara', title: 'Ankara site visit — retention policy' },
  { id: 'meeting-0002-pricing', title: 'Q3 pricing and packaging' },
  { id: 'meeting-0003-security', title: 'Security review kick-off' },
];

const ITEMS: OpenActionItem[] = [
  {
    id: 'ai-1',
    meeting_id: 'meeting-0001-ankara',
    meeting_title: 'Ankara site visit — retention policy',
    text: 'Send the revised retention policy to the site lead before Friday.',
    assignee: null,
    due: 'Fri',
    status: 'draft',
    source_chunk_id: 'chunk-12',
  },
  {
    id: 'ai-2',
    meeting_id: 'meeting-0002-pricing',
    meeting_title: 'Q3 pricing and packaging',
    text: 'Book the security review with the platform team.',
    assignee: 'Amira',
    due: null,
    status: 'draft',
    source_chunk_id: 'chunk-48',
  },
  {
    id: 'ai-3',
    meeting_id: 'meeting-0003-security',
    meeting_title: 'Security review kick-off',
    text: 'Confirm the data-residency answer before the next call.',
    assignee: null,
    due: null,
    status: 'edited',
    source_chunk_id: 'chunk-61',
  },
];

function service(items: OpenActionItem[], failing = false) {
  return {
    getOpenActionItems: async () => {
      if (failing) throw new Error('fixture: local read failed');
      return items;
    },
    approveActionItem: async () => true,
    // The third item refuses, so the revert-and-tell path is visible in the fixture.
    rejectActionItem: async (id: string) => id !== 'ai-3',
    editActionItem: async () => true,
  };
}

function Harness({
  meetings,
  children,
}: {
  meetings: CurrentMeeting[];
  children: React.ReactNode;
}) {
  const value = useMemo(
    () =>
      ({
        currentMeeting: null,
        setCurrentMeeting: () => {},
        sidebarItems: [],
        isCollapsed: true,
        toggleCollapse: () => {},
        meetings,
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
    [meetings]
  );
  return <SidebarContext.Provider value={value}>{children}</SidebarContext.Provider>;
}

function Frame({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <section className="space-y-2">
      <h2 className="text-title">{label}</h2>
      <div className="flex h-[520px] overflow-hidden rounded-lg border border-border bg-background">
        {children}
      </div>
    </section>
  );
}

function Panel({ theme }: { theme: 'light' | 'dark' }) {
  return (
    <div className={theme === 'dark' ? 'dark' : ''}>
      <div className="space-y-6 bg-background p-6 text-foreground">
        <header className="space-y-1">
          <div className="text-eyebrow uppercase text-subtle-foreground">{theme} theme</div>
          <h1 className="text-display">Home — review queue</h1>
          <p className="text-body text-muted-foreground">
            §6.1. Every action item carries its marking, its source link and its decision.
          </p>
        </header>

        <Frame label="Queue · drafts awaiting a decision">
          <Harness meetings={MEETINGS}>
            <HomeDashboard service={service(ITEMS)} forceLoad />
          </Harness>
        </Frame>

        <Frame label="Queue · load failed">
          <Harness meetings={MEETINGS}>
            <HomeDashboard service={service([], true)} forceLoad />
          </Harness>
        </Frame>

        <Frame label="Queue · nothing waiting">
          <Harness meetings={MEETINGS}>
            <HomeDashboard service={service([])} forceLoad />
          </Harness>
        </Frame>

        <Frame label="First run · nothing recorded yet">
          <Harness meetings={[]}>
            <HomeDashboard service={service([])} forceLoad />
          </Harness>
        </Frame>
      </div>
    </div>
  );
}

export default function DesignHomePage() {
  return (
    <div>
      <Panel theme="light" />
      <Panel theme="dark" />
    </div>
  );
}
