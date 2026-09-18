'use client';

/**
 * The one page header — DESIGN_SYSTEM.md §5.3.
 *
 * Replaces `components/MainNav`, which was `h-0` and rendered nothing while still
 * being the only thing in the tree that looked like a page title. Every screen gets
 * the same shape: an optional eyebrow, one `h1`, and at most one primary plus one
 * secondary action on the right.
 *
 * SCROLLED STATE WITHOUT A SCROLL LISTENER. Each screen owns its own scroll container
 * (`body` has `overflow: hidden`), so there is no window scroll to listen to and no
 * single element this component could subscribe to. It renders a 1px sentinel above
 * itself instead and watches it with an IntersectionObserver: when the sentinel leaves
 * its scrollport the header has content under it and takes its border and elevation.
 * That works in any scroll container, including none.
 */

import React, { useEffect, useRef, useState } from 'react';
import { cn } from '@/lib/utils';

export interface PageHeaderProps {
  /** Small uppercase line above the title, e.g. `MEETING REPORT`. */
  eyebrow?: React.ReactNode;
  title: React.ReactNode;
  /** One line of context under the title — date, duration, counts. */
  meta?: React.ReactNode;
  /** At most one primary + one secondary control, plus an overflow menu. */
  actions?: React.ReactNode;
  className?: string;
  children?: React.ReactNode;
}

export function PageHeader({
  eyebrow,
  title,
  meta,
  actions,
  className,
  children,
}: PageHeaderProps) {
  const sentinelRef = useRef<HTMLDivElement>(null);
  const [scrolled, setScrolled] = useState(false);

  useEffect(() => {
    const el = sentinelRef.current;
    if (!el || typeof IntersectionObserver === 'undefined') return;
    const observer = new IntersectionObserver(
      ([entry]) => setScrolled(!entry.isIntersecting),
      { threshold: 1 }
    );
    observer.observe(el);
    return () => observer.disconnect();
  }, []);

  return (
    <>
      <div ref={sentinelRef} aria-hidden="true" className="h-px w-full shrink-0" />
      <header
        className={cn(
          // `min-h-header`, not `h-header`: with an eyebrow AND a meta line the content
          // is ~60px and a fixed 56px box makes the title collide with what follows it.
          // A screen that passes only a title still measures exactly 56px.
          'sticky top-0 z-20 flex min-h-header shrink-0 items-center gap-3 bg-card/85 px-gutter py-2 backdrop-blur',
          'transition-shadow duration-fast ease-out',
          scrolled ? 'border-b border-border shadow-elev-1' : 'border-b border-transparent',
          className
        )}
      >
        <div className="min-w-0 flex-1">
          {eyebrow ? (
            <div className="text-eyebrow uppercase text-subtle-foreground">{eyebrow}</div>
          ) : null}
          {/* Title and meta share a row and wrap together, so a long meta line never
              pushes the actions off the right edge of a 900px window. */}
          <div className="flex min-w-0 flex-wrap items-baseline gap-x-2">
            <h1 className="truncate text-title-lg text-foreground">{title}</h1>
            {meta ? (
              <span className="truncate text-caption text-subtle-foreground">{meta}</span>
            ) : null}
          </div>
        </div>
        {actions ? <div className="flex shrink-0 items-center gap-2">{actions}</div> : null}
        {children}
      </header>
    </>
  );
}

export default PageHeader;
