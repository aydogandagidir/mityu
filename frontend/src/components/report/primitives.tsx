'use client';

/**
 * Report primitives — the read.ai-referenced building blocks (see docs/DESIGN_READAI.md
 * and the prototype at /design/report), extracted so the real, data-bound summary views
 * can use the same visual language.
 *
 * All semantic tokens: adapts light/dark and inherits the bluedev brand.
 */

import type { ReactNode } from 'react';
import { Clock, Sparkles } from 'lucide-react';

/** Non-hideable EU AI Act Art. 50 transparency label. */
export function AiLabel() {
  return (
    <span className="inline-flex shrink-0 items-center gap-1 rounded-full bg-accent px-2 py-0.5 text-[10px] font-medium text-accent-foreground">
      <Sparkles className="h-3 w-3" aria-hidden />
      AI-generated · review required
    </span>
  );
}

/** A chip that jumps to the backing transcript segment. */
export function SourceChip({
  label = 'Source',
  onClick,
  title,
}: {
  label?: string;
  onClick?: () => void;
  title?: string;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      title={title ?? 'Jump to the source transcript segment'}
      aria-label="Jump to source transcript segment"
      className="inline-flex shrink-0 items-center gap-1 rounded-full border border-border bg-muted/60 px-2 py-0.5 text-xs text-muted-foreground transition-colors hover:border-primary/40 hover:text-foreground"
    >
      <Clock className="h-3 w-3" aria-hidden />
      {label}
    </button>
  );
}

/**
 * A read.ai-style report section: rounded card, icon + title + optional count in the
 * header, and (optionally) the AI transparency label pinned to the right.
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
  icon: any;
  title: string;
  count?: number;
  /** Fill the icon tile with the brand color (use for the primary "Summary" card). */
  accent?: boolean;
  aiLabel?: boolean;
  actions?: ReactNode;
  children: ReactNode;
}) {
  return (
    <section className="overflow-hidden rounded-2xl border border-border bg-card shadow-sm">
      <header className="flex items-center gap-2 border-b border-border px-4 py-3">
        <span
          className={`grid h-7 w-7 shrink-0 place-items-center rounded-lg ${
            accent ? 'bg-primary text-primary-foreground' : 'bg-accent text-accent-foreground'
          }`}
        >
          <Icon className="h-4 w-4" aria-hidden />
        </span>
        <h3 className="truncate text-sm font-semibold text-foreground">{title}</h3>
        {count != null && (
          <span className="shrink-0 rounded-full bg-muted px-2 py-0.5 text-xs text-muted-foreground">
            {count}
          </span>
        )}
        <div className="ml-auto flex shrink-0 items-center gap-2">
          {actions}
          {aiLabel && <AiLabel />}
        </div>
      </header>
      <div className="p-4">{children}</div>
    </section>
  );
}
