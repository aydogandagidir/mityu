'use client';

import React, { createContext, useCallback, useContext, useState, useEffect, useRef } from 'react';
import { usePathname, useRouter } from 'next/navigation';
import Analytics from '@/lib/analytics';
import { invoke } from '@tauri-apps/api/core';
import { useRecordingState } from '@/contexts/RecordingStateContext';
import {
  isEvidenceQuerySearchable,
  searchService,
  type TranscriptSearchResult,
} from '@/services/search';


/** §3.2 — the meetings pane is a user preference, not a per-launch accident. */
const MEETINGS_PANE_STORAGE_KEY = 'mityu.ui.meetingsPane';

interface SidebarItem {
  id: string;
  title: string;
  type: 'folder' | 'file';
  children?: SidebarItem[];
}

export interface CurrentMeeting {
  id: string;
  title: string;
}

// Search result type for transcript search: single typed definition lives in
// '@/services/search' (re-exported here for consumers of this provider).
export type { TranscriptSearchResult };

export interface SidebarContextType {
  currentMeeting: CurrentMeeting | null;
  setCurrentMeeting: (meeting: CurrentMeeting | null) => void;
  sidebarItems: SidebarItem[];
  isCollapsed: boolean;
  toggleCollapse: () => void;
  meetings: CurrentMeeting[];
  setMeetings: (meetings: CurrentMeeting[]) => void;
  isMeetingActive: boolean;
  setIsMeetingActive: (active: boolean) => void;
  handleRecordingToggle: () => void;
  /**
   * True once first-launch initialisation has run. It replaces `serverAddress` as the
   * gate on the meeting fetch: that string was a leftover from the archived Python
   * backend, and gating data loading on "is a localhost URL set" made the timing of the
   * first DB read an accident of an unrelated legacy field.
   */
  ready: boolean;
  searchTranscripts: (query: string) => void;
  searchResults: TranscriptSearchResult[];
  isSearching: boolean;
  searchError: string | null;
  setServerAddress: (address: string) => void;
  serverAddress: string;
  transcriptServerAddress: string;
  setTranscriptServerAddress: (address: string) => void;
  // Summary polling management
  activeSummaryPolls: Map<string, NodeJS.Timeout>;
  startSummaryPolling: (meetingId: string, processId: string, onUpdate: (result: any) => void) => void;
  stopSummaryPolling: (meetingId: string) => void;
  // Refetch meetings from backend
  refetchMeetings: () => Promise<void>;

}

/** Exported for the `/design/*` fixtures only — see RecordingStateContext. */
export const SidebarContext = createContext<SidebarContextType | null>(null);

export const useSidebar = () => {
  const context = useContext(SidebarContext);
  if (!context) {
    throw new Error('useSidebar must be used within a SidebarProvider');
  }
  return context;
};

export function SidebarProvider({ children }: { children: React.ReactNode }) {
  const [currentMeeting, setCurrentMeeting] = useState<CurrentMeeting | null>({ id: 'intro-call', title: '+ New Call' });
  // Prerender and first paint agree on `true`; the real preference is read on mount
  // (see the effect below), because a static export has no window at build time and a
  // lazy initialiser would hydrate against a different value.
  const [isCollapsed, setIsCollapsed] = useState(true);
  const [ready, setReady] = useState(false);
  const [meetings, setMeetings] = useState<CurrentMeeting[]>([]);
  const [sidebarItems, setSidebarItems] = useState<SidebarItem[]>([]);
  const [isMeetingActive, setIsMeetingActive] = useState(false);
  const [searchResults, setSearchResults] = useState<TranscriptSearchResult[]>([]);
  const [isSearching, setIsSearching] = useState(false);
  const [searchError, setSearchError] = useState<string | null>(null);
  const searchTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const searchRequestIdRef = useRef(0);
  const [serverAddress, setServerAddress] = useState('');
  const [transcriptServerAddress, setTranscriptServerAddress] = useState('');
  const [activeSummaryPolls, setActiveSummaryPolls] = useState<Map<string, NodeJS.Timeout>>(new Map());

  // Use recording state from RecordingStateContext (single source of truth)
  const { isRecording } = useRecordingState();

  const pathname = usePathname();
  const router = useRouter();

  // Extract fetchMeetings as a reusable function
  const fetchMeetings = React.useCallback(async () => {
    if (ready) {
      try {
        const meetings = await invoke('api_get_meetings') as Array<{ id: string, title: string }>;
        const transformedMeetings = meetings.map((meeting: any) => ({
          id: meeting.id,
          title: meeting.title
        }));
        setMeetings(transformedMeetings);
        Analytics.trackBackendConnection(true);
      } catch (error) {
        console.error('Error fetching meetings:', error);
        setMeetings([]);
        Analytics.trackBackendConnection(false);
      }
    }
  }, [ready]);

  useEffect(() => {
    fetchMeetings();
  }, [ready, fetchMeetings]);

  useEffect(() => {
    // One mount-time initialisation. The two addresses are kept because
    // `ModelSettingsModal` and `useModelConfiguration` still read them; what changed is
    // that nothing gates on them any more.
    setServerAddress('http://localhost:5167');
    setTranscriptServerAddress('http://127.0.0.1:8178/stream');
    setReady(true);
  }, []);

  /**
   * The meetings pane remembers whether it was open, per §3.2 — the old shell reset to
   * collapsed on every launch, so the library was hidden by default forever. A first
   * run with no stored preference follows the window: open at >=1100px, closed below
   * 1000px, and between the two it keeps whichever side of that band the width is on.
   */
  useEffect(() => {
    let collapsed: boolean;
    try {
      const stored = localStorage.getItem(MEETINGS_PANE_STORAGE_KEY);
      collapsed = stored === null ? window.innerWidth < 1100 : stored !== 'open';
    } catch {
      // Private mode or blocked site data: fall back to the width rule.
      collapsed = window.innerWidth < 1100;
    }
    setIsCollapsed(collapsed);
  }, []);

  const baseItems: SidebarItem[] = [
    {
      id: 'meetings',
      title: 'Meeting Notes',
      type: 'folder' as const,
      children: [
        ...meetings.map(meeting => ({ id: meeting.id, title: meeting.title, type: 'file' as const }))
      ]
    },
  ];


  const toggleCollapse = useCallback(() => {
    setIsCollapsed((current) => {
      const next = !current;
      try {
        localStorage.setItem(MEETINGS_PANE_STORAGE_KEY, next ? 'closed' : 'open');
      } catch {
        // Not being able to remember the preference must never break the toggle.
      }
      return next;
    });
  }, []);

  // Update current meeting when on home page
  useEffect(() => {
    if (pathname === '/') {
      setCurrentMeeting({ id: 'intro-call', title: '+ New Call' });
    }
    setSidebarItems(baseItems);
  }, [pathname]);

  // Update sidebar items when meetings change
  useEffect(() => {
    setSidebarItems(baseItems);
  }, [meetings]);

  // Function to handle recording toggle from sidebar
  const handleRecordingToggle = () => {
    if (!isRecording) {
      // Check if already on home page
      if (pathname === '/') {
        // Already on home - trigger recording directly via custom event
        console.log('Triggering recording from sidebar (already on home page)');
        window.dispatchEvent(new CustomEvent('start-recording-from-sidebar'));
      } else {
        // Not on home - navigate and use auto-start mechanism
        console.log('Navigating to home page with auto-start flag');
        sessionStorage.setItem('autoStartRecording', 'true');
        router.push('/');
      }

      // Track recording initiation from sidebar
      Analytics.trackButtonClick('start_recording', 'sidebar');
    }
    // The actual recording start/stop is handled in the Home component
  };

  // Debounce local evidence search while invalidating an in-flight request as
  // soon as the input changes. Tauri invokes are not cancellable, so the
  // monotonically increasing request id prevents a late response from replacing
  // results for a newer query.
  const searchTranscripts = useCallback((query: string) => {
    const requestId = ++searchRequestIdRef.current;

    if (searchTimerRef.current) {
      clearTimeout(searchTimerRef.current);
      searchTimerRef.current = null;
    }

    const normalizedQuery = query.trim();
    setSearchError(null);

    if (!normalizedQuery || !isEvidenceQuerySearchable(normalizedQuery)) {
      setSearchResults([]);
      setIsSearching(false);
      return;
    }

    // Do not leave results from a previous query visible during the debounce.
    setSearchResults([]);
    setIsSearching(true);

    searchTimerRef.current = setTimeout(async () => {
      searchTimerRef.current = null;

      try {
        const results = await searchService.searchEvidence(normalizedQuery);
        if (requestId === searchRequestIdRef.current) {
          // The backend relevance order is authoritative.
          setSearchResults(results);
        }
      } catch {
        if (requestId === searchRequestIdRef.current) {
          // Never include the raw query or meeting content in logs.
          console.error('Local meeting search failed');
          setSearchResults([]);
          setSearchError('Search could not be completed. Please try again.');
        }
      } finally {
        if (requestId === searchRequestIdRef.current) {
          setIsSearching(false);
        }
      }
    }, 275);
  }, []);

  useEffect(() => {
    return () => {
      // Invalidate any invoke already in flight.
      // Increment instead of assigning MAX_SAFE_INTEGER: React Strict Mode can
      // run effect cleanup/setup twice in development, and increments above the
      // safe-integer boundary would stop being unique.
      searchRequestIdRef.current += 1;
      if (searchTimerRef.current) {
        clearTimeout(searchTimerRef.current);
      }
    };
  }, []);

  // Summary polling management
  const startSummaryPolling = React.useCallback((
    meetingId: string,
    processId: string,
    onUpdate: (result: any) => void
  ) => {
    // Stop existing poll for this meeting if any
    if (activeSummaryPolls.has(meetingId)) {
      clearInterval(activeSummaryPolls.get(meetingId)!);
    }

    console.log(`📊 Starting polling for meeting ${meetingId}, process ${processId}`);

    let pollCount = 0;
    const MAX_POLLS = 200; // ~16.5 minutes at 5-second intervals (slightly longer than backend's 15-min timeout to avoid race conditions)

    const pollInterval = setInterval(async () => {
      pollCount++;

      // Timeout safety: Stop after 10 minutes
      if (pollCount >= MAX_POLLS) {
        console.warn(`⏱️ Polling timeout for ${meetingId} after ${MAX_POLLS} iterations`);
        clearInterval(pollInterval);
        setActiveSummaryPolls(prev => {
          const next = new Map(prev);
          next.delete(meetingId);
          return next;
        });
        onUpdate({
          status: 'error',
          error: 'Summary generation timed out after 15 minutes. Please try again or check your model configuration.'
        });
        return;
      }
      try {
        const result = await invoke('api_get_summary', {
          meetingId: meetingId,
        }) as any;

        console.log(`📊 Polling update for ${meetingId}:`, result.status);

        // Call the update callback with result
        onUpdate(result);

        // Stop polling if completed, error, failed, cancelled, or idle (after initial processing)
        if (result.status === 'completed' || result.status === 'error' || result.status === 'failed' || result.status === 'cancelled') {
          console.log(`Polling completed for ${meetingId}, status: ${result.status}`);
          clearInterval(pollInterval);
          setActiveSummaryPolls(prev => {
            const next = new Map(prev);
            next.delete(meetingId);
            return next;
          });
        } else if (result.status === 'idle' && pollCount > 1) {
          // If we get 'idle' after polling started, process completed/disappeared
          console.log(`Process completed or not found for ${meetingId}, stopping poll`);
          clearInterval(pollInterval);
          setActiveSummaryPolls(prev => {
            const next = new Map(prev);
            next.delete(meetingId);
            return next;
          });
        }
      } catch (error) {
        console.error(`Polling error for ${meetingId}:`, error);
        // Report error to callback
        onUpdate({
          status: 'error',
          error: error instanceof Error ? error.message : 'Unknown error'
        });
        clearInterval(pollInterval);
        setActiveSummaryPolls(prev => {
          const next = new Map(prev);
          next.delete(meetingId);
          return next;
        });
      }
    }, 5000); // Poll every 5 seconds

    setActiveSummaryPolls(prev => new Map(prev).set(meetingId, pollInterval));
  }, [activeSummaryPolls]);

  const stopSummaryPolling = React.useCallback((meetingId: string) => {
    const pollInterval = activeSummaryPolls.get(meetingId);
    if (pollInterval) {
      console.log(`⏹️ Stopping polling for meeting ${meetingId}`);
      clearInterval(pollInterval);
      setActiveSummaryPolls(prev => {
        const next = new Map(prev);
        next.delete(meetingId);
        return next;
      });
    }
  }, [activeSummaryPolls]);

  // Cleanup all polling intervals on unmount
  useEffect(() => {
    return () => {
      console.log('🧹 Cleaning up all summary polling intervals');
      activeSummaryPolls.forEach(interval => clearInterval(interval));
    };
  }, [activeSummaryPolls]);



  return (
    <SidebarContext.Provider value={{
      currentMeeting,
      setCurrentMeeting,
      sidebarItems,
      isCollapsed,
      toggleCollapse,
      meetings,
      setMeetings,
      isMeetingActive,
      setIsMeetingActive,
      handleRecordingToggle,
      ready,
      searchTranscripts,
      searchResults,
      isSearching,
      searchError,
      setServerAddress,
      serverAddress,
      transcriptServerAddress,
      setTranscriptServerAddress,
      activeSummaryPolls,
      startSummaryPolling,
      stopSummaryPolling,
      refetchMeetings: fetchMeetings,

    }}>
      {children}
    </SidebarContext.Provider>
  );
}
