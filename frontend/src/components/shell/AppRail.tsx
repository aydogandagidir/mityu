'use client';

/**
 * The 56px application rail — DESIGN_SYSTEM.md §5.1.
 *
 * Replaces the collapsed/expanded halves of the old 894-line `Sidebar`, which was
 * two different navigations sharing one file: a rail of icons when collapsed and a
 * list with its own duplicate Home/Actions/record controls when expanded. The rail is
 * now ALWAYS the navigation, and the meeting library is a pane the user opens beside
 * it (`MeetingsPane`). That is what makes the content pane stop moving under the user
 * every time they look something up.
 *
 * 🔒 FROZEN CONTRACTS THIS FILE CARRIES
 * - `data-tour="record-button"` — exactly ONE such element may be in the DOM at a
 *   time; `TourProvider` resolves it by selector. The old file rendered it in two
 *   branches (`:466` collapsed, `:779` expanded) and relied on only one branch being
 *   mounted. Here there is one element, in one place, in every state.
 * - `handleRecordingToggle` comes from `useSidebar()` and is called, never
 *   re-implemented: it dispatches `start-recording-from-sidebar` on `/` and otherwise
 *   sets `sessionStorage.autoStartRecording` and navigates. The rail must never call
 *   `recordingService` itself.
 * - `Analytics.trackButtonClick('start_recording', 'sidebar')` fires inside that
 *   provider method, so the identifier survives this move unchanged.
 * - The brand mark is `/mityu-mark.svg` with alt text `Mityu`.
 */

import React from 'react';
import { usePathname, useRouter } from 'next/navigation';
import { House, ListChecks, Library, Mic, Search, Settings2, Upload } from 'lucide-react';
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from '@/components/ui/tooltip';
import { Dialog, DialogContent, DialogTitle, DialogTrigger } from '@/components/ui/dialog';
import { VisuallyHidden } from '@/components/ui/visually-hidden';
import { About } from '@/components/About';
import { useSidebar } from '@/components/Sidebar/SidebarProvider';
import { useRecordingState } from '@/contexts/RecordingStateContext';
import { useImportDialog } from '@/contexts/ImportDialogContext';
import { useConfig } from '@/contexts/ConfigContext';
import { TOUR_ANCHORS } from '@/lib/tour';
import { APP_VERSION } from '@/lib/appVersion';
import { focusRing } from '@/components/ui/focus-ring';
import { cn } from '@/lib/utils';

/** Custom event the rail's Search button fires; `MeetingsPane` focuses its field. */
export const FOCUS_MEETING_SEARCH_EVENT = 'mityu:focus-meeting-search';

/** 40×40 hit target (§8), icon 20. One shape for every rail control. */
const RAIL_ITEM =
  'grid h-10 w-10 shrink-0 place-items-center rounded-lg transition-colors duration-fast ease-out';

function RailButton({
  label,
  active,
  onClick,
  children,
  className,
  ...rest
}: {
  label: string;
  active?: boolean;
  onClick?: () => void;
  children: React.ReactNode;
  className?: string;
} & Omit<React.ButtonHTMLAttributes<HTMLButtonElement>, 'onClick' | 'children' | 'className'>) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <button
          type="button"
          onClick={onClick}
          aria-label={label}
          className={cn(
            RAIL_ITEM,
            focusRing('sidebar'),
            active
              ? 'bg-sidebar-accent text-sidebar-accent-foreground'
              : 'text-subtle-foreground hover:bg-sidebar-accent/60 hover:text-sidebar-foreground',
            className
          )}
          {...rest}
        >
          {children}
        </button>
      </TooltipTrigger>
      <TooltipContent side="right">{label}</TooltipContent>
    </Tooltip>
  );
}

/** A nav destination: adds the active pill and the 2px brand rule of §5.1. */
function RailNavItem({
  label,
  href,
  active,
  children,
}: {
  label: string;
  href: string;
  active: boolean;
  children: React.ReactNode;
}) {
  const router = useRouter();
  return (
    <div className="relative">
      {active && (
        <span
          aria-hidden="true"
          className="absolute left-0 top-1/2 h-5 w-0.5 -translate-y-1/2 rounded-r-full bg-primary"
        />
      )}
      <RailButton
        label={label}
        active={active}
        onClick={() => router.push(href)}
        aria-current={active ? 'page' : undefined}
      >
        {children}
      </RailButton>
    </div>
  );
}

export function AppRail() {
  const pathname = usePathname();
  const router = useRouter();
  const { isCollapsed, toggleCollapse, handleRecordingToggle } = useSidebar();
  const { isRecording } = useRecordingState();
  const { openImportDialog } = useImportDialog();
  const { betaFeatures } = useConfig();

  const paneOpen = !isCollapsed;

  return (
    <nav
      aria-label="Primary"
      // `h-full`, not `h-screen`: the rail must take its height from whatever contains
      // it. Assuming the viewport clipped its lower half — Record, Search, Settings and
      // the version — inside any box shorter than the window.
      className="flex h-full w-rail shrink-0 flex-col items-center gap-1 border-r border-sidebar-border bg-sidebar py-3"
    >
      {/* Brand mark → About. One entry point, where the old shell had three
          (logo, a collapsed Info button and a second Info in the footer). */}
      <Dialog>
        <Tooltip>
          <TooltipTrigger asChild>
            <DialogTrigger asChild>
              <button
                type="button"
                aria-label="About Mityu"
                className={cn(RAIL_ITEM, focusRing('sidebar'), 'hover:bg-sidebar-accent/60')}
              >
                {/* eslint-disable-next-line @next/next/no-img-element */}
                <img src="/mityu-mark.svg" alt="Mityu" width={28} height={28} />
              </button>
            </DialogTrigger>
          </TooltipTrigger>
          <TooltipContent side="right">About Mityu</TooltipContent>
        </Tooltip>
        <DialogContent>
          <VisuallyHidden>
            <DialogTitle>About Mityu</DialogTitle>
          </VisuallyHidden>
          <About />
        </DialogContent>
      </Dialog>

      <div className="my-1 h-px w-6 bg-sidebar-border" aria-hidden="true" />

      <RailButton
        label={paneOpen ? 'Hide meetings' : 'Show meetings'}
        active={paneOpen}
        onClick={toggleCollapse}
        aria-expanded={paneOpen}
        aria-controls="meetings-pane"
      >
        <Library className="size-5" aria-hidden="true" />
      </RailButton>

      <RailNavItem label="Home" href="/" active={pathname === '/'}>
        <House className="size-5" aria-hidden="true" />
      </RailNavItem>

      <RailNavItem label="Actions" href="/actions" active={pathname === '/actions'}>
        <ListChecks className="size-5" aria-hidden="true" />
      </RailNavItem>

      <div className="flex-1" />

      {/* 🔒 The single record anchor. While recording it stays enabled and takes the
          user to the session, because a control that cannot be reached cannot be
          stopped; the stop control itself lives on that screen. */}
      <Tooltip>
        <TooltipTrigger asChild>
          <button
            type="button"
            data-tour={TOUR_ANCHORS.recordButton}
            onClick={() => {
              if (isRecording) {
                router.push('/');
                return;
              }
              handleRecordingToggle();
            }}
            aria-label={isRecording ? 'Recording — open the session' : 'Start recording'}
            className={cn(
              RAIL_ITEM,
              focusRing('sidebar'),
              'bg-recording text-recording-foreground shadow-elev-1 hover:bg-recording-hover active:bg-recording-active'
            )}
          >
            {isRecording ? (
              <span className="relative flex size-3">
                <span className="absolute inline-flex size-3 animate-ping rounded-full bg-recording-foreground/70" />
                <span className="relative inline-flex size-3 rounded-full bg-recording-foreground" />
              </span>
            ) : (
              <Mic className="size-5" aria-hidden="true" />
            )}
          </button>
        </TooltipTrigger>
        <TooltipContent side="right">
          {isRecording ? 'Recording — open the session' : 'Start recording'}
        </TooltipContent>
      </Tooltip>

      {betaFeatures.importAndRetranscribe && (
        <RailButton label="Import audio" onClick={() => openImportDialog()}>
          <Upload className="size-5" aria-hidden="true" />
        </RailButton>
      )}

      <RailButton
        label="Search meetings"
        onClick={() => {
          if (isCollapsed) toggleCollapse();
          // The pane may be mounting this frame; let it commit before focusing.
          requestAnimationFrame(() =>
            window.dispatchEvent(new CustomEvent(FOCUS_MEETING_SEARCH_EVENT))
          );
        }}
      >
        <Search className="size-5" aria-hidden="true" />
      </RailButton>

      <RailNavItem label="Settings" href="/settings" active={pathname === '/settings'}>
        <Settings2 className="size-5" aria-hidden="true" />
      </RailNavItem>

      <span className="pt-1 text-micro tabular-nums text-subtle-foreground">
        v{APP_VERSION}
      </span>
    </nav>
  );
}

export default AppRail;
