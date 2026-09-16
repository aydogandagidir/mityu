'use client';

/**
 * The evidence drawer — DESIGN_SYSTEM.md §5.20, §6.3.
 *
 * The report reads as one document; the transcript is the evidence you open beside a
 * claim, not a second column competing with it for the window. This is where the
 * transcript lives.
 *
 * WHY IT IS NOT A `Dialog`/`Sheet`. Three of this screen's contracts rule that out.
 * (1) The transcript pane must stay MOUNTED at all times — unmounting loses its scroll
 * position and breaks jump-to-source, which retries a scroll after pagination brings the
 * segment in. A Radix dialog unmounts its content when closed. (2) Focus must NOT be
 * trapped and the report must stay interactive, because the point of opening evidence is
 * to read it next to the claim. (3) The first-run tour anchors on the transcript pane and
 * needs the anchored element visible, not merely present.
 *
 * So this is a plain `aside` that is always mounted, animates its width, and takes its
 * content out of the tab order when closed — the same trick the meetings pane uses, for
 * the same reason: a zero-width box still hands focus to what it contains.
 *
 * Esc closes it and returns focus to whatever opened it, which the caller supplies.
 */

import React, { useEffect, useRef } from 'react';
import { X } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { cn } from '@/lib/utils';

export interface EvidenceDrawerProps {
  open: boolean;
  onClose: () => void;
  /** Focused when the drawer closes, so Esc returns the user where they were. */
  returnFocusTo?: React.RefObject<HTMLElement>;
  title?: string;
  children: React.ReactNode;
  id?: string;
}

export function EvidenceDrawer({
  open,
  onClose,
  returnFocusTo,
  title = 'Transcript',
  children,
  id = 'evidence-drawer',
}: EvidenceDrawerProps) {
  const wasOpen = useRef(open);

  useEffect(() => {
    if (!open) return;
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        event.stopPropagation();
        onClose();
      }
    };
    // Capture phase so the drawer answers Esc before a parent does, while still not
    // trapping focus or blocking anything else on the page.
    window.addEventListener('keydown', onKeyDown, true);
    return () => window.removeEventListener('keydown', onKeyDown, true);
  }, [open, onClose]);

  useEffect(() => {
    if (wasOpen.current && !open) returnFocusTo?.current?.focus();
    wasOpen.current = open;
  }, [open, returnFocusTo]);

  return (
    <aside
      id={id}
      aria-label={title}
      aria-hidden={!open}
      className={cn(
        'h-full shrink-0 overflow-hidden border-l border-border bg-card',
        'transition-[width] duration-slow ease-emphasis motion-reduce:transition-none',
        open ? 'w-[min(32rem,45vw)]' : 'w-0 border-l-0'
      )}
    >
      <div
        className={cn(
          'flex h-full w-[min(32rem,45vw)] flex-col',
          // Out of the tab order the moment it closes; the width keeps animating.
          !open && 'invisible'
        )}
      >
        <div className="flex h-11 shrink-0 items-center justify-between gap-2 border-b border-border px-3">
          <h2 className="text-title-sm text-foreground">{title}</h2>
          <Button variant="ghost" size="icon" onClick={onClose} aria-label="Close transcript">
            <X className="size-4" aria-hidden="true" />
          </Button>
        </div>
        <div className="flex min-h-0 flex-1 flex-col">{children}</div>
      </div>
    </aside>
  );
}

export default EvidenceDrawer;
