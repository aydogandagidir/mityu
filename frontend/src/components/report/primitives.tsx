'use client';

/**
 * Report primitives — DESIGN_SYSTEM.md §5.6 / §5.8 / §5.9. The three pieces the report
 * document is made of: the per-card Art. 50 chip, the link from a claim to its proof, and
 * the section card that holds them.
 *
 * Everything here is token-only (§5) and both themes come for free.
 */

import type { ReactNode } from 'react';
import { useId } from 'react';
import { Clock, Sparkles, type LucideIcon } from 'lucide-react';

import { Badge } from '@/components/ui/badge';
import { CARD_SURFACE, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { focusRing, type FocusSurface } from '@/components/ui/focus-ring';
import { toPlainText } from '@/components/ui/notice';
import { cn } from '@/lib/utils';

/**
 * AiLabel — the per-card EU AI Act Art. 50 chip (§5.8).
 *
 * 🔒 IT IS A PLAIN `<span>` WITH NO `role` AND NO `aria-label`, AND THAT IS LOAD-BEARING.
 * The ONE `role="note"` per surface is `Notice tone="ai" as="note"` (ReviewRequiredBanner,
 * the AskPanel disclosure, CopilotInsights' AiMarking, the legacy banner). Giving this chip
 * `role="note"` and the banner's accessible name duplicates that name inside the same tree,
 * and `DraftSummaryView.transparency.test.tsx`'s SINGULAR
 * `getByRole('note', { name: /AI-generated content, human review required/i })` THROWS on
 * the second match. The banner is the accessible marking; this chip is the VISIBLE one —
 * two channels, one name.
 *
 * 12px `text-caption`, up from the 10px it shipped at: §4.5 makes 12px the floor for
 * anything a person must read, and a compliance disclosure is the last string in the product
 * that may be below it.
 *
 * `children` go through the text-only renderer, so the chip structurally cannot contain a
 * control — the same guard `Notice` applies to the banner.
 */
export function AiLabel({ children }: { children?: ReactNode }) {
  const extra = toPlainText(children);
  return (
    <Badge tone="ai" className="border-solid">
      <Sparkles aria-hidden />
      AI-generated · review required
      {extra ? ` ${extra}` : null}
    </Badge>
  );
}

/** `mm:ss` or `h:mm:ss`. Anything else is not a clock time and is not printed as one. */
const CLOCK = /^\d{1,2}:\d{2}(:\d{2})?$/;

export interface SourceChipProps {
  /** 🔒 The visible word. §4.1 of the audit register pins it — the timestamp is ADDED to it. */
  label?: string;
  /** `12:04`. Anything that is not a clock time is dropped (see `CLOCK`). */
  timestamp?: string | null;
  onClick?: () => void;
  /** Native tooltip text. Never the ONLY carrier of a reason (§5 shared rules). */
  title?: string;
  /** 🔒 Defaults to the frozen block/action-item name. */
  ariaLabel?: string;
  /** The evidence drawer currently shows this segment. */
  active?: boolean;
  /** False when the item has no transcript link: disabled + a visible sibling sentence. */
  resolved?: boolean;
  /** The id of the evidence drawer this chip opens. */
  controls?: string;
  /** What the chip sits on, for the §4.4.6 focus offset. `accent` inside a selected row. */
  surface?: FocusSurface;
  className?: string;
}

/**
 * SourceChip — the single most important small component in the product: the link from a
 * claim to its proof (§5.9).
 *
 * ANATOMY. `Clock` 12px, then 🔒 the visible word `Source` in `--primary-ink`, then
 * `· 12:04` in `font-mono text-micro tabular-nums` in `--subtle-foreground`. The boundary is
 * `--input`, not `--border`: this is a CONTROL, and at 1.25:1 `--border` may never be a
 * control's only boundary (§4.4.5). A chip is never distinguished by colour alone — the word
 * and the icon are always there.
 *
 * THE TIMESTAMP IS VALIDATED, NOT TRUSTED. The worker's UTC `sourceTimestamp` fallback is an
 * absolute date, and printing it where `12:04` belongs would tell the user that a claim came
 * from four minutes into a meeting that it did not. Anything that is not `mm:ss` / `h:mm:ss`
 * is dropped: the chip reads `Source`, with no time, and the item counts as UNRESOLVED.
 *
 * UNRESOLVED IS DISABLED PLUS A VISIBLE SENTENCE, NEVER A TOOLTIP. A disabled button is not
 * focusable, so a tooltip on it never opens for a keyboard or AT user (§5 shared rules) —
 * the reason has to be on screen, and `aria-describedby` ties it to the control. Export
 * stays 🔒 fail-closed on these items regardless.
 */
export function SourceChip({
  label = 'Source',
  timestamp,
  onClick,
  title,
  ariaLabel = 'Jump to source transcript segment',
  active = false,
  resolved = true,
  controls = 'transcript-drawer',
  surface = 'card',
  className,
}: SourceChipProps) {
  const reasonId = useId();
  const time = timestamp && CLOCK.test(timestamp) ? timestamp : null;
  const isResolved = resolved && Boolean(time);

  const chip = (
    <button
      type="button"
      onClick={isResolved ? onClick : undefined}
      disabled={!isResolved}
      title={title}
      aria-label={ariaLabel}
      aria-expanded={isResolved ? active : undefined}
      aria-controls={isResolved ? controls : undefined}
      aria-describedby={isResolved ? undefined : reasonId}
      className={cn(
        'inline-flex h-6 shrink-0 items-center gap-1 rounded-xs border border-input bg-card px-2',
        'transition-colors duration-instant ease-out',
        '[&_svg]:pointer-events-none [&_svg]:size-3 [&_svg]:shrink-0',
        'hover:border-input-hover hover:bg-accent',
        'active:bg-surface-3',
        // The ACTIVE chip is marked by a 2px rule, not by a colour change: the drawer being
        // open is a structural fact and survives greyscale.
        active && 'border-l-2 border-l-primary bg-accent',
        'disabled:pointer-events-none disabled:opacity-50',
        focusRing(surface),
        className
      )}
    >
      <Clock aria-hidden className="text-subtle-foreground" />
      <span className="text-caption text-primary-ink">{time ? `${label} ·` : label}</span>
      {time ? (
        <span className="font-mono text-micro tabular-nums text-subtle-foreground">
          {time}
        </span>
      ) : null}
    </button>
  );

  if (isResolved) return chip;

  return (
    <span className="inline-flex flex-wrap items-center gap-2">
      {chip}
      <span id={reasonId} className="text-caption text-subtle-foreground">
        This item has no transcript link
      </span>
    </span>
  );
}

/**
 * SectionCard — a report section (§5.6): the `Card` surface, a 44px header with the icon
 * tile, title, count and the right-hand action cluster, then the body.
 *
 * 🔒 THE PROP CONTRACT IS FROZEN: `icon, title, count, accent, aiLabel, actions, children`.
 * `DraftSummaryView` renders every section and the action-item list through it.
 *
 * It stays a `<section>` (the header carries the heading) and only borrows the Card surface,
 * which is why `CARD_SURFACE` is exported rather than copied.
 */
export function SectionCard({
  icon: Icon,
  title,
  count,
  accent,
  aiLabel,
  actions,
  children,
}: {
  icon: LucideIcon;
  title: string;
  count?: number;
  /** Fill the icon tile with the brand colour (the primary "Summary" card). */
  accent?: boolean;
  aiLabel?: boolean;
  actions?: ReactNode;
  children: ReactNode;
}) {
  return (
    <section className={cn(CARD_SURFACE, 'overflow-hidden')}>
      <CardHeader>
        <span
          className={cn(
            'grid size-7 shrink-0 place-items-center rounded-sm',
            accent ? 'bg-primary text-primary-foreground' : 'bg-accent text-accent-foreground'
          )}
        >
          <Icon className="size-4" aria-hidden />
        </span>
        <CardTitle>{title}</CardTitle>
        {count != null && <Badge tone="neutral">{count}</Badge>}
        <div className="ml-auto flex shrink-0 items-center gap-2">
          {actions}
          {aiLabel && <AiLabel />}
        </div>
      </CardHeader>
      <CardContent>{children}</CardContent>
    </section>
  );
}
