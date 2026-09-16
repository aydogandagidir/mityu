'use client';

/**
 * The 280px meeting library — DESIGN_SYSTEM.md §5.2.
 *
 * WHY IT IS A PANE AND NOT A ROUTE. The rail's Meetings control toggles this pane;
 * it never navigates. In the old shell the library and the navigation shared one
 * collapsible column, so opening the library moved the content pane 200px sideways
 * and closing it hid the only list of meetings. Here the rail never moves, and the
 * pane is the user's choice, remembered across launches.
 *
 * 🔒 FROZEN CONTRACTS THIS FILE CARRIES, verbatim from the old Sidebar:
 * - search placeholder `Search meeting evidence…`, `aria-label="Search meeting evidence"`,
 *   clear `aria-label="Clear meeting search"`;
 * - `searchTranscripts()` from the provider owns the ≥2-alphanumeric gate, the 275ms
 *   debounce and stale-response suppression, and the query is NEVER logged;
 * - every `SearchResultsList` role and string, rendered by that component unchanged;
 * - the jump deep link `?id=&segment=&source=search&jump=<nonce>`;
 * - the ADR-0026 delete disclosure, and the IndexedDB + sessionStorage purge BEFORE
 *   `api_delete_meeting`;
 * - `Analytics.trackMeetingDeleted()` and
 *   `Analytics.trackButtonClick('edit_meeting_title', 'sidebar')`.
 *
 * WHAT IS DELIBERATELY NOT HERE YET. §5.2 asks for date-grouped rows. `api_get_meetings`
 * returns `{id, title}` only, so the only way to group by date today is to parse the
 * date out of the generated title — which is exactly the defect the audit recorded
 * (a renamed meeting loses its date, and the Home dashboard already does this). The
 * sticky-group machinery is therefore built and rendered with ONE group; when the list
 * payload carries `created_at`, grouping becomes a change to `groupMeetings()` alone.
 */

import React, { useCallback, useEffect, useId, useRef, useState } from 'react';
import { useRouter } from 'next/navigation';
import { Mic, Pencil, Search as SearchIcon, Trash2, X } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { toast } from 'sonner';
import Analytics from '@/lib/analytics';
import { indexedDBService } from '@/services/indexedDBService';
import { useSidebar, type CurrentMeeting } from '@/components/Sidebar/SidebarProvider';
import { SearchResultsList } from '@/components/Sidebar/SearchResultsList';
import {
  isEvidenceQuerySearchable,
  type TranscriptSearchResult,
} from '@/services/search';
import { ConfirmationModal } from '@/components/ConfirmationModel/confirmation-modal';
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogTitle,
} from '@/components/ui/dialog';
import { VisuallyHidden } from '@/components/ui/visually-hidden';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { EmptyState } from '@/components/ui/empty-state';
import { focusRing } from '@/components/ui/focus-ring';
import { FOCUS_MEETING_SEARCH_EVENT } from './AppRail';
import { cn } from '@/lib/utils';

/** 🔒 ADR-0026. The disclosure a user reads before a meeting is destroyed. */
const DELETE_DISCLOSURE =
  'This removes the meeting from Mityu-managed local database and search data, its Mityu-managed recording artifacts, and recovery cache. Unknown files you placed in the recording folder are retained. This cannot be undone. SSD wear-leveling, copy-on-write filesystems, snapshots, backups, exports, and WebView/browser storage may retain physical traces or separate copies that Mityu cannot erase.';

/** A real meeting, as opposed to the `intro-call` placeholder the provider seeds. */
function isMeetingRow(id: string) {
  return id.includes('-') && !id.startsWith('intro-call');
}

/**
 * One group today, by design (see the docblock). The shape is the contract: a label and
 * its rows, so a `created_at` on the payload turns this into a real date grouping
 * without touching the renderer.
 */
function groupMeetings(meetings: CurrentMeeting[]) {
  const rows = meetings.filter((m) => isMeetingRow(m.id));
  if (rows.length === 0) return [];
  return [{ label: 'All meetings', rows }];
}

export function MeetingsPane() {
  const router = useRouter();
  const {
    meetings,
    setMeetings,
    currentMeeting,
    setCurrentMeeting,
    isCollapsed,
    searchTranscripts,
    searchResults,
    isSearching,
    searchError,
  } = useSidebar();

  const [query, setQuery] = useState('');
  const [deleteState, setDeleteState] = useState<{ open: boolean; id: string | null; title: string }>({
    open: false,
    id: null,
    title: '',
  });
  const [isDeletePending, setIsDeletePending] = useState(false);
  const [renameState, setRenameState] = useState<{ open: boolean; id: string | null }>({
    open: false,
    id: null,
  });
  const [renameValue, setRenameValue] = useState('');

  const searchRef = useRef<HTMLInputElement>(null);
  const jumpNonceRef = useRef(0);
  const searchFieldId = useId();

  // The rail's Search control opens the pane and focuses this field.
  useEffect(() => {
    const focus = () => searchRef.current?.focus();
    window.addEventListener(FOCUS_MEETING_SEARCH_EVENT, focus);
    return () => window.removeEventListener(FOCUS_MEETING_SEARCH_EVENT, focus);
  }, []);

  const onQueryChange = useCallback(
    (value: string) => {
      setQuery(value);
      searchTranscripts(value);
    },
    [searchTranscripts]
  );

  const onSelectResult = useCallback(
    (result: TranscriptSearchResult) => {
      setCurrentMeeting({ id: result.id, title: result.title });
      const nonce = ++jumpNonceRef.current;
      const params = new URLSearchParams({
        id: result.id,
        segment: result.sourceChunkId,
        source: 'search',
        jump: `${Date.now()}-${nonce}`,
      });
      router.push(`/meeting-details?${params.toString()}`);
    },
    [router, setCurrentMeeting]
  );

  const openMeeting = useCallback(
    (meeting: CurrentMeeting) => {
      setCurrentMeeting(meeting);
      router.push(`/meeting-details?id=${meeting.id}`);
    },
    [router, setCurrentMeeting]
  );

  const confirmDelete = useCallback(async () => {
    const id = deleteState.id;
    if (!id || isDeletePending) return;
    setIsDeletePending(true);
    try {
      // 🔒 Browser recovery data lives outside the native SQLite transaction, so it is
      // purged BEFORE the native delete: a failure here must not leave a plaintext copy
      // behind after the durable record is gone.
      await indexedDBService.purgeSavedMeetings();
      await indexedDBService.deleteMeeting(id);
      if (sessionStorage.getItem('indexeddb_current_meeting_id') === id) {
        sessionStorage.removeItem('indexeddb_current_meeting_id');
      }
      await invoke('api_delete_meeting', { meetingId: id });

      setMeetings(meetings.filter((m) => m.id !== id));
      Analytics.trackMeetingDeleted();
      toast.success('Meeting deleted successfully', {
        description:
          'Mityu-managed database, search, recording, and recovery data was removed.',
      });
      if (currentMeeting?.id === id) {
        setCurrentMeeting({ id: 'intro-call', title: '+ New Call' });
        router.push('/');
      }
      setDeleteState({ open: false, id: null, title: '' });
    } catch (error) {
      // Never log the meeting's content or title.
      console.error('Failed to delete meeting');
      toast.error('Failed to delete meeting', {
        description: error instanceof Error ? error.message : String(error),
      });
    } finally {
      setIsDeletePending(false);
    }
  }, [
    currentMeeting?.id,
    deleteState.id,
    isDeletePending,
    meetings,
    router,
    setCurrentMeeting,
    setMeetings,
  ]);

  const confirmRename = useCallback(async () => {
    const id = renameState.id;
    const title = renameValue.trim();
    if (!id) return;
    if (!title) {
      toast.error('Meeting title cannot be empty');
      return;
    }
    try {
      await invoke('api_save_meeting_title', { meetingId: id, title });
      setMeetings(meetings.map((m) => (m.id === id ? { ...m, title } : m)));
      if (currentMeeting?.id === id) setCurrentMeeting({ id, title });
      Analytics.trackButtonClick('edit_meeting_title', 'sidebar');
      toast.success('Meeting title updated successfully');
      setRenameState({ open: false, id: null });
      setRenameValue('');
    } catch (error) {
      console.error('Failed to update meeting title');
      toast.error('Failed to update meeting title', {
        description: error instanceof Error ? error.message : String(error),
      });
    }
  }, [currentMeeting?.id, meetings, renameState.id, renameValue, setCurrentMeeting, setMeetings]);

  const searching = query.trim().length > 0;
  const groups = groupMeetings(meetings);

  return (
    <>
      <aside
        id="meetings-pane"
        aria-label="Meetings"
        aria-hidden={isCollapsed}
        className={cn(
          // `h-full` for the same reason as the rail: height comes from the container.
          'h-full shrink-0 overflow-hidden border-r border-border bg-background',
          'transition-[width] duration-base ease-out motion-reduce:transition-none',
          isCollapsed ? 'w-0 border-r-0' : 'w-pane'
        )}
      >
        {/* `invisible` rather than unmounting: it takes the pane's controls out of the
            tab order the instant it closes — a zero-width overflow-hidden box still
            hands focus to what it contains — while the width keeps animating and the
            scroll position survives the round trip. */}
        <div className={cn('flex h-full w-pane flex-col', isCollapsed && 'invisible')}>
          <div className="flex h-11 shrink-0 items-center border-b border-border px-3">
            <h2 className="text-eyebrow text-subtle-foreground">Meetings</h2>
          </div>

          <div className="shrink-0 p-3">
            <div className="relative">
              <SearchIcon
                className="pointer-events-none absolute left-2.5 top-1/2 size-4 -translate-y-1/2 text-subtle-foreground"
                aria-hidden="true"
              />
              <Input
                ref={searchRef}
                id={searchFieldId}
                value={query}
                onChange={(e) => onQueryChange(e.target.value)}
                placeholder="Search meeting evidence…"
                aria-label="Search meeting evidence"
                className="pl-8 pr-8"
              />
              {query && (
                <button
                  type="button"
                  aria-label="Clear meeting search"
                  onClick={() => onQueryChange('')}
                  className={cn(
                    'absolute right-1.5 top-1/2 grid size-6 -translate-y-1/2 place-items-center rounded-sm',
                    'text-subtle-foreground hover:bg-muted hover:text-foreground',
                    focusRing('background')
                  )}
                >
                  <X className="size-4" aria-hidden="true" />
                </button>
              )}
            </div>
          </div>

          <div className="min-h-0 flex-1 overflow-y-auto custom-scrollbar">
            {searching ? (
              <>
                <div className="sticky top-0 z-10 flex items-center justify-between gap-2 bg-background/95 px-3 py-1.5 backdrop-blur">
                  <span className="text-eyebrow text-subtle-foreground">
                    {isEvidenceQuerySearchable(query)
                      ? `Results · ${searchResults.length}`
                      : 'Results'}
                  </span>
                  <Button variant="ghost" size="xs" onClick={() => onQueryChange('')}>
                    Clear
                  </Button>
                </div>
                <SearchResultsList
                  results={searchResults}
                  isSearching={isSearching}
                  isQueryTooShort={!isEvidenceQuerySearchable(query)}
                  error={searchError}
                  onSelect={onSelectResult}
                />
              </>
            ) : groups.length === 0 ? (
              <div className="p-3">
                <EmptyState
                  icon={Mic}
                  title="No meetings yet"
                  description="Recordings you make appear here, newest first."
                />
              </div>
            ) : (
              groups.map((group) => (
                <section key={group.label} aria-label={group.label}>
                  <h3 className="sticky top-0 z-10 bg-background/95 px-3 py-1.5 text-eyebrow text-subtle-foreground backdrop-blur">
                    {group.label}
                  </h3>
                  <ul role="list" className="px-2 pb-2">
                    {group.rows.map((meeting) => {
                      const active = currentMeeting?.id === meeting.id;
                      return (
                        <li key={meeting.id} className="group relative">
                          <button
                            type="button"
                            onClick={() => openMeeting(meeting)}
                            aria-current={active ? 'page' : undefined}
                            className={cn(
                              'w-full rounded-md py-2 pl-3 pr-16 text-left text-body',
                              'transition-colors duration-fast ease-out',
                              focusRing('background'),
                              active
                                ? 'bg-accent font-medium text-accent-foreground'
                                : 'text-foreground hover:bg-muted'
                            )}
                          >
                            <span className="line-clamp-2 break-words">{meeting.title}</span>
                          </button>
                          {/* Row actions are revealed on hover or keyboard focus, and are
                              ALWAYS visible on a coarse pointer, where there is no hover
                              (§8). They sit outside the row button so they are their own
                              tab stops rather than nested controls. */}
                          <div
                            className={cn(
                              'absolute right-1 top-1/2 flex -translate-y-1/2 items-center gap-0.5',
                              'opacity-0 transition-opacity duration-fast ease-out',
                              'group-hover:opacity-100 group-focus-within:opacity-100',
                              '[@media(pointer:coarse)]:opacity-100'
                            )}
                          >
                            <button
                              type="button"
                              aria-label={`Rename ${meeting.title}`}
                              onClick={() => {
                                setRenameState({ open: true, id: meeting.id });
                                setRenameValue(meeting.title);
                              }}
                              className={cn(
                                'grid size-8 place-items-center rounded-sm text-subtle-foreground',
                                'hover:bg-accent hover:text-accent-foreground',
                                focusRing('background')
                              )}
                            >
                              <Pencil className="size-4" aria-hidden="true" />
                            </button>
                            <button
                              type="button"
                              aria-label={`Delete ${meeting.title}`}
                              onClick={() =>
                                setDeleteState({ open: true, id: meeting.id, title: meeting.title })
                              }
                              className={cn(
                                'grid size-8 place-items-center rounded-sm text-subtle-foreground',
                                'hover:bg-destructive-surface hover:text-destructive-ink',
                                focusRing('background')
                              )}
                            >
                              <Trash2 className="size-4" aria-hidden="true" />
                            </button>
                          </div>
                        </li>
                      );
                    })}
                  </ul>
                </section>
              ))
            )}
          </div>
        </div>
      </aside>

      {/* The confirmation now NAMES the meeting — the old modal recited the disclosure
          without ever saying what was about to be deleted. */}
      <ConfirmationModal
        isOpen={deleteState.open}
        title={deleteState.title ? `Delete “${deleteState.title}”?` : undefined}
        text={DELETE_DISCLOSURE}
        onConfirm={confirmDelete}
        onCancel={() => {
          if (!isDeletePending) setDeleteState({ open: false, id: null, title: '' });
        }}
        isBusy={isDeletePending}
      />

      <Dialog
        open={renameState.open}
        onOpenChange={(open) => {
          if (!open) {
            setRenameState({ open: false, id: null });
            setRenameValue('');
          }
        }}
      >
        <DialogContent className="sm:max-w-[425px]">
          <VisuallyHidden>
            <DialogTitle>Edit Meeting Title</DialogTitle>
          </VisuallyHidden>
          <div className="space-y-3 py-2">
            <h3 className="text-title-sm">Edit Meeting Title</h3>
            <div className="space-y-2">
              <label htmlFor="meeting-title" className="text-label text-foreground">
                Meeting Title
              </label>
              <Input
                id="meeting-title"
                value={renameValue}
                onChange={(e) => setRenameValue(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === 'Enter') void confirmRename();
                }}
                placeholder="Enter meeting title"
                autoFocus
              />
            </div>
          </div>
          <DialogFooter>
            <Button
              variant="outline"
              onClick={() => {
                setRenameState({ open: false, id: null });
                setRenameValue('');
              }}
            >
              Cancel
            </Button>
            <Button onClick={() => void confirmRename()}>Save</Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  );
}

export default MeetingsPane;
