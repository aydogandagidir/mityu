'use client';

/**
 * /design/actions — the Action Center, Tauri-free (DESIGN_SYSTEM.md §11.1).
 *
 * The real screen with an injected loader, across the four states that matter: a loaded
 * page with more to fetch, the empty case, the total failure, and the case this screen
 * used to get wrong — an item whose recording predates stored segment positions, where
 * the worker's UTC fallback must never be printed as if it were a position in the audio.
 */

import React, { useMemo } from 'react';
import { ActionCenter } from '@/components/actions/ActionCenter';
import type {
  ApprovedActionItem,
  ApprovedActionItemsPage,
} from '@/services/actionCenterService';
import {
  SidebarContext,
  type SidebarContextType,
} from '@/components/Sidebar/SidebarProvider';

const ITEMS: ApprovedActionItem[] = [
  {
    id: 'a1',
    meetingId: 'm1',
    meetingTitle: 'Ankara site visit — retention policy',
    meetingCreatedAt: '2026-09-09T09:30:00Z',
    text: 'Send the revised retention policy to the site lead before Friday.',
    assignee: 'Deniz',
    due: '2026-09-19',
    reviewStatus: 'approved',
    sourceChunkId: 'chunk-12',
    sourceTimestamp: '2026-09-09T09:42:11Z',
    audioStartTime: 724,
  },
  {
    id: 'a2',
    meetingId: 'm2',
    meetingTitle: 'Q3 pricing and packaging',
    meetingCreatedAt: '2026-09-04T13:00:00Z',
    text: 'Lead with the managed tier; usage-based add-ons stay optional.',
    assignee: null,
    due: null,
    reviewStatus: 'approved',
    sourceChunkId: 'chunk-48',
    sourceTimestamp: '2026-09-04T13:08:03Z',
    audioStartTime: 3003,
  },
  {
    id: 'a3',
    meetingId: 'm3',
    meetingTitle: 'Security review kick-off',
    meetingCreatedAt: '2026-08-28T08:15:00Z',
    text: 'Confirm the data-residency answer before the next call.',
    assignee: null,
    due: null,
    reviewStatus: 'approved',
    sourceChunkId: 'chunk-61',
    // No stored offset: the chip must read "Source" and say why, never print this.
    sourceTimestamp: '2026-08-28T08:41:55Z',
    audioStartTime: null,
  },
];

function loader(page: ApprovedActionItemsPage | 'error') {
  return async (): Promise<ApprovedActionItemsPage> => {
    if (page === 'error') throw new Error('fixture: local read failed');
    return page;
  };
}

function Harness({ children }: { children: React.ReactNode }) {
  const value = useMemo(
    () =>
      ({
        currentMeeting: null,
        setCurrentMeeting: () => {},
        sidebarItems: [],
        isCollapsed: true,
        toggleCollapse: () => {},
        meetings: [],
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
    []
  );
  return <SidebarContext.Provider value={value}>{children}</SidebarContext.Provider>;
}

function Frame({
  label,
  height = 560,
  children,
}: {
  label: string;
  height?: number;
  children: React.ReactNode;
}) {
  return (
    <section className="space-y-2">
      <h2 className="text-title">{label}</h2>
      <div
        className="flex overflow-hidden rounded-lg border border-border bg-background"
        style={{ height }}
      >
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
          <h1 className="text-display">Action Center</h1>
          <p className="text-body text-muted-foreground">
            §6.4. Approved, read-only, source-linked — and on tokens in both themes.
          </p>
        </header>

        <Frame label="Loaded · more available" height={680}>
          <Harness>
            <ActionCenter
              loadPage={loader({ items: ITEMS, hasMore: true, nextOffset: 100 })}
            />
          </Harness>
        </Frame>

        <Frame label="Empty">
          <Harness>
            <ActionCenter loadPage={loader({ items: [], hasMore: false, nextOffset: null })} />
          </Harness>
        </Frame>

        <Frame label="Failed to load">
          <Harness>
            <ActionCenter loadPage={loader('error')} />
          </Harness>
        </Frame>
      </div>
    </div>
  );
}

export default function DesignActionsPage() {
  return (
    <div>
      <Panel theme="light" />
      <Panel theme="dark" />
    </div>
  );
}
