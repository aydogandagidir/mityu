'use client';

import { useEffect, useRef, useState } from 'react';

/** Viewport-relative rectangle of a tour target (from getBoundingClientRect). */
export interface TargetRect {
  top: number;
  left: number;
  width: number;
  height: number;
}

function readRect(el: Element): TargetRect {
  const r = el.getBoundingClientRect();
  return { top: r.top, left: r.left, width: r.width, height: r.height };
}

/** Visible = actually laid out with a non-trivial box (handles display:none tabs). */
function isVisible(el: Element): boolean {
  const r = el.getBoundingClientRect();
  return r.width > 1 && r.height > 1;
}

function sameRect(a: TargetRect | null, b: TargetRect | null): boolean {
  if (a === b) return true;
  if (!a || !b) return false;
  return (
    a.top === b.top &&
    a.left === b.left &&
    a.width === b.width &&
    a.height === b.height
  );
}

/**
 * Resolves a `data-tour` target and tracks its rectangle while `active`.
 *
 * Robustness contract (never blocks, skips, or crashes the tour):
 *  - polls indefinitely (every 150ms) while the step is active — async content
 *    (a summary draft still loading) or a target being revealed (a collapsed
 *    panel expanding, a tab switching) will resolve as soon as it appears;
 *  - returns `null` while the target is missing/hidden. The caller shows the
 *    step centered instead of skipping it, so a hidden target NEVER makes a step
 *    disappear or breaks Back/Next;
 *  - follows the element through scroll / resize / layout shifts via a rAF loop
 *    (state only updates when the rect actually changes); if the element leaves
 *    the DOM or goes hidden mid-step, it drops the rect (→ centered) and resumes
 *    polling until it returns.
 *
 * `getRoot` lets a caller scope the query (the browser dev-preview scopes to its
 * own mock container so it never grabs the real sidebar button). It is read from
 * a ref, so passing an inline arrow does not restart the effect.
 */
export function useTourTarget(
  selector: string,
  active: boolean,
  getRoot: () => ParentNode = () => document,
): TargetRect | null {
  const [rect, setRect] = useState<TargetRect | null>(null);
  const rectRef = useRef<TargetRect | null>(null);

  const getRootRef = useRef(getRoot);
  getRootRef.current = getRoot;

  useEffect(() => {
    rectRef.current = null;
    setRect(null);

    if (!active || !selector) return;

    let cancelled = false;
    let rafId = 0;
    let pollId = 0;
    let el: Element | null = null;

    const clearRect = () => {
      if (rectRef.current !== null) {
        rectRef.current = null;
        setRect(null);
      }
    };

    const track = () => {
      if (cancelled) return;
      if (!el || !el.isConnected || !isVisible(el)) {
        // Target left the DOM or went hidden (collapsed panel, tab switch, a
        // re-render): drop the spotlight and resume polling until it comes back.
        el = null;
        clearRect();
        poll();
        return;
      }
      const next = readRect(el);
      if (!sameRect(rectRef.current, next)) {
        rectRef.current = next;
        setRect(next);
      }
      rafId = requestAnimationFrame(track);
    };

    const poll = () => {
      if (cancelled) return;
      const found = getRootRef.current().querySelector(selector);
      if (found && isVisible(found)) {
        el = found;
        try {
          (el as HTMLElement).scrollIntoView({ block: 'nearest', inline: 'nearest' });
        } catch {
          /* scrollIntoView unsupported: ignore, rect still tracks */
        }
        rafId = requestAnimationFrame(track);
        return;
      }
      // Not resolved yet — keep polling. Never gives up while active, so a
      // late-appearing or just-revealed target still gets spotlighted.
      pollId = window.setTimeout(poll, 150);
    };

    poll();

    return () => {
      cancelled = true;
      if (rafId) cancelAnimationFrame(rafId);
      if (pollId) clearTimeout(pollId);
    };
  }, [selector, active]);

  return rect;
}
