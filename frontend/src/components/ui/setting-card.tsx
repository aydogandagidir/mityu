'use client';

/**
 * One setting, with its control and — when it needs one — its explanation
 * folded away (G4).
 *
 * The settings screens were built by copy-pasting the same card string fourteen
 * times, every one at `p-6` with a `text-lg` heading. The result was that a
 * theme toggle carried the same visual weight as the recording-consent gate,
 * and ~590 words of explanation sat permanently on screen at the same size as
 * the things they explained. The user's report was not that anything was wrong,
 * but that "her yer yazı dolu" — every surface is full of text and hard to read.
 *
 * So this primitive enforces three things a copy-pasted div cannot:
 *
 * 1. **A rank.** Title, description and details each get a fixed step of the
 *    type scale, so explanation can never be typeset as loudly as the control.
 * 2. **Progressive disclosure that keeps the claim.** `details` goes inside a
 *    native `<details>` element: collapsed by default, present in the DOM,
 *    findable by the browser's own find-in-page, and open with no JavaScript.
 *    The `summary` line stays visible, so folding an explanation never removes
 *    the fact it carries — it shortens it.
 * 3. **One place to change the look.** The next density pass edits this file
 *    instead of fourteen.
 *
 * NOT for legal or honesty markings. The EU AI Act Art. 50 disclosure and
 * marking, the recording-consent text and the accuracy notice must stay visible
 * without a click (CLAUDE.md §0.5/§0.6, ADR-0032). Those are rendered by their
 * own components and must not be moved behind `details`.
 */

import type { ReactNode } from 'react';
import { cn } from '@/lib/utils';

export interface SettingCardProps {
  title: string;
  /** One line. If it needs two sentences, the second belongs in `details`. */
  description?: ReactNode;
  /** The control: a switch, a button, a select. Sits opposite the title. */
  action?: ReactNode;
  /** Longer content that belongs to this setting and is always visible. */
  children?: ReactNode;
  /** Explanation worth keeping but not worth reading every time. */
  details?: ReactNode;
  /** The label on the disclosure. Say what is inside it. */
  detailsLabel?: string;
  className?: string;
}

export function SettingCard({
  title,
  description,
  action,
  children,
  details,
  detailsLabel = 'Details',
  className,
}: SettingCardProps) {
  return (
    <section className={cn('rounded-lg border border-border bg-card p-5 shadow-sm', className)}>
      <div className="flex items-start justify-between gap-4">
        <div className="min-w-0 space-y-1">
          <h3 className="text-title font-semibold text-foreground">{title}</h3>
          {description && <p className="text-meta text-muted-foreground">{description}</p>}
        </div>
        {action && <div className="shrink-0">{action}</div>}
      </div>

      {children && <div className="mt-4">{children}</div>}

      {details && (
        <details className="group mt-3">
          <summary className="cursor-pointer list-none text-caption text-muted-foreground underline-offset-2 hover:text-foreground hover:underline">
            {detailsLabel}
          </summary>
          <div className="mt-2 space-y-2 text-caption leading-relaxed text-muted-foreground">
            {details}
          </div>
        </details>
      )}
    </section>
  );
}

export default SettingCard;
