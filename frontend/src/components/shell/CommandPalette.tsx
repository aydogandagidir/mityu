'use client';

/**
 * The command palette and the shortcuts sheet — DESIGN_SYSTEM.md §3.4.
 *
 * This app had no keyboard model at all. Enter and Escape worked inside inputs; there
 * was no way to start or stop a recording, reach a meeting, open a settings section or
 * even find out what the keys were — while the beta copilot registered global OS
 * shortcuts. A desktop app that people leave open all day and that is FULL of text is
 * the wrong place to be mouse-only.
 *
 * Built on the `cmdk` dependency the project already carries, so nothing is added.
 *
 * WHAT IT DELIBERATELY DOES NOT DO. It never starts a recording itself: the record
 * command calls the same `handleRecordingToggle` every other entry point calls, which
 * dispatches or navigates depending on the route. One path to capture, always.
 */

import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { useRouter } from 'next/navigation';
import { House, ListChecks, Mic, Search, Settings2, FileText } from 'lucide-react';
import {
  CommandDialog,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
  CommandSeparator,
  CommandShortcut,
} from '@/components/ui/command';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { useSidebar } from '@/components/Sidebar/SidebarProvider';
import { useRecordingState } from '@/contexts/RecordingStateContext';
import { FOCUS_MEETING_SEARCH_EVENT } from './AppRail';

const SETTINGS_SECTIONS = [
  'general',
  'recording',
  'transcription',
  'summary',
  'privacy',
  'learning',
  'beta',
  'license',
  'about',
] as const;

/** Rows of the shortcuts sheet. Kept beside the handlers so the two cannot drift. */
export const SHORTCUTS: { keys: string; action: string }[] = [
  { keys: '⌘K / Ctrl+K', action: 'Command palette' },
  { keys: '⌘⇧R / Ctrl+Shift+R', action: 'Start or open the recording' },
  { keys: '⌘B / Ctrl+B', action: 'Show or hide the meetings pane' },
  { keys: '/', action: 'Search meeting evidence' },
  { keys: '⌘, / Ctrl+,', action: 'Settings' },
  { keys: '?', action: 'This list' },
  { keys: 'Esc', action: 'Close the evidence drawer, a dialog, or the palette' },
];

/** True when focus is in something that takes typed text. */
function isTyping(target: EventTarget | null): boolean {
  const el = target as HTMLElement | null;
  if (!el) return false;
  const tag = el.tagName?.toLowerCase();
  return (
    tag === 'input' ||
    tag === 'textarea' ||
    tag === 'select' ||
    el.isContentEditable === true
  );
}

export function CommandPalette() {
  const router = useRouter();
  const { meetings, setCurrentMeeting, handleRecordingToggle, isCollapsed, toggleCollapse } =
    useSidebar();
  const { isRecording } = useRecordingState();
  const [open, setOpen] = useState(false);
  const [showShortcuts, setShowShortcuts] = useState(false);

  const openSearch = useCallback(() => {
    if (isCollapsed) toggleCollapse();
    requestAnimationFrame(() =>
      window.dispatchEvent(new CustomEvent(FOCUS_MEETING_SEARCH_EVENT))
    );
  }, [isCollapsed, toggleCollapse]);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      const mod = event.metaKey || event.ctrlKey;

      if (mod && event.key.toLowerCase() === 'k') {
        event.preventDefault();
        setOpen((v) => !v);
        return;
      }
      if (mod && event.key.toLowerCase() === 'b') {
        event.preventDefault();
        toggleCollapse();
        return;
      }
      if (mod && event.key === ',') {
        event.preventDefault();
        router.push('/settings');
        return;
      }
      if (mod && event.shiftKey && event.key.toLowerCase() === 'r') {
        event.preventDefault();
        if (isRecording) router.push('/');
        else handleRecordingToggle();
        return;
      }
      // The single-character keys must never steal a keystroke from a text field —
      // this app is full of them, and `?` is a character people type.
      if (isTyping(event.target)) return;
      if (event.key === '?') {
        event.preventDefault();
        setShowShortcuts(true);
        return;
      }
      if (event.key === '/') {
        event.preventDefault();
        openSearch();
      }
    };

    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [handleRecordingToggle, isRecording, openSearch, router, toggleCollapse]);

  const recentMeetings = useMemo(
    () => meetings.filter((m) => m.id.includes('-') && !m.id.startsWith('intro-call')).slice(0, 8),
    [meetings]
  );

  const run = (action: () => void) => {
    setOpen(false);
    action();
  };

  return (
    <>
      <CommandDialog open={open} onOpenChange={setOpen}>
        <CommandInput placeholder="Search meetings, actions and settings…" />
        <CommandList>
          <CommandEmpty>Nothing matches that.</CommandEmpty>

          <CommandGroup heading="Go">
            <CommandItem onSelect={() => run(() => router.push('/'))}>
              <House className="size-4" aria-hidden="true" />
              Home
            </CommandItem>
            <CommandItem onSelect={() => run(() => router.push('/actions'))}>
              <ListChecks className="size-4" aria-hidden="true" />
              Actions
            </CommandItem>
            <CommandItem onSelect={() => run(() => router.push('/settings'))}>
              <Settings2 className="size-4" aria-hidden="true" />
              Settings
              <CommandShortcut>⌘,</CommandShortcut>
            </CommandItem>
          </CommandGroup>

          <CommandSeparator />

          <CommandGroup heading="Do">
            <CommandItem
              onSelect={() =>
                run(() => (isRecording ? router.push('/') : handleRecordingToggle()))
              }
            >
              <Mic className="size-4" aria-hidden="true" />
              {isRecording ? 'Open the recording' : 'Start recording'}
              <CommandShortcut>⌘⇧R</CommandShortcut>
            </CommandItem>
            <CommandItem onSelect={() => run(openSearch)}>
              <Search className="size-4" aria-hidden="true" />
              Search meeting evidence
              <CommandShortcut>/</CommandShortcut>
            </CommandItem>
          </CommandGroup>

          {recentMeetings.length > 0 && (
            <>
              <CommandSeparator />
              <CommandGroup heading="Meetings">
                {recentMeetings.map((meeting) => (
                  <CommandItem
                    key={meeting.id}
                    value={`meeting ${meeting.title}`}
                    onSelect={() =>
                      run(() => {
                        setCurrentMeeting(meeting);
                        router.push(`/meeting-details?id=${meeting.id}`);
                      })
                    }
                  >
                    <FileText className="size-4" aria-hidden="true" />
                    {meeting.title}
                  </CommandItem>
                ))}
              </CommandGroup>
            </>
          )}

          <CommandSeparator />

          <CommandGroup heading="Settings sections">
            {SETTINGS_SECTIONS.map((section) => (
              <CommandItem
                key={section}
                value={`settings ${section}`}
                onSelect={() => run(() => router.push(`/settings?section=${section}`))}
              >
                <Settings2 className="size-4" aria-hidden="true" />
                <span className="capitalize">{section}</span>
              </CommandItem>
            ))}
          </CommandGroup>
        </CommandList>
      </CommandDialog>

      <Dialog open={showShortcuts} onOpenChange={setShowShortcuts}>
        <DialogContent className="sm:max-w-[480px]">
          <DialogHeader>
            <DialogTitle>Keyboard shortcuts</DialogTitle>
            <DialogDescription>
              Press ? at any time to see this list.
            </DialogDescription>
          </DialogHeader>
          <dl className="divide-y divide-border">
            {SHORTCUTS.map((s) => (
              <div key={s.keys} className="flex items-center justify-between gap-4 py-2">
                <dt className="text-body text-foreground">{s.action}</dt>
                <dd>
                  <kbd className="rounded-sm border border-border bg-surface-2 px-2 py-0.5 font-mono text-caption text-muted-foreground">
                    {s.keys}
                  </kbd>
                </dd>
              </div>
            ))}
          </dl>
        </DialogContent>
      </Dialog>
    </>
  );
}

export default CommandPalette;
