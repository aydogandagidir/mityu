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
 * Robustness contract (never blocks or crashes the tour):
 *  - polls for the element for ~2s (async content may not be mounted yet); if it
 *    never appears (or stays hidden), calls `onNotFound` so the caller can skip;
 *  - follows the element through scroll / resize / layout shifts via a rAF loop
 *    (state only updates when the rect actually changes);
 *  - re-resolves / reports missing if the element leaves the DOM mid-step.
 *
 * `getRoot` lets a caller scope the query (the browser dev-preview scopes to its
 * own mock container so it never grabs the real sidebar button). It is read from
 * a ref, so passing an inline arrow does not restart the effect.
 */
export function useTourTarget(
  selector: string,
  active: boolean,
  getRoot: () => ParentNode = () => document,
  onNotFound?: () => void,
): TargetRect | null {
  const [rect, setRect] = useState<TargetRect | null>(null);
  const rectRef = useRef<TargetRect | null>(null);

  const getRootRef = useRef(getRoot);
  getRootRef.current = getRoot;
  const onNotFoundRef = useRef(onNotFound);
  onNotFoundRef.current = onNotFound;

  useEffect(() => {
    rectRef.current = null;
    setRect(null);

    if (!active || !selector) return;

    let cancelled = false;
    let rafId = 0;
    let el: Element | null = null;
    let attempts = 0;
    const MAX_ATTEMPTS = 20; // ~2s at 100ms

    const track = () => {
      if (cancelled) return;
      if (!el || !el.isConnected || !isVisible(el)) {
        // element vanished or went hidden — try to re-resolve once more
        const found = getRootRef.current().querySelector(selector);
        if (found && isVisible(found)) {
          el = found;
        } else {
          onNotFoundRef.current?.();
          return;
        }
      }
      const next = readRect(el);
      if (!sameRect(rectRef.current, next)) {
        rectRef.current = next;
        setRect(next);
      }
      rafId = requestAnimationFrame(track);
    };

    const resolve = () => {
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
      attempts += 1;
      if (attempts >= MAX_ATTEMPTS) {
        onNotFoundRef.current?.();
        return;
      }
      window.setTimeout(resolve, 100);
    };

    resolve();

    return () => {
      cancelled = true;
      if (rafId) cancelAnimationFrame(rafId);
    };
  }, [selector, active]);

  return rect;
}
