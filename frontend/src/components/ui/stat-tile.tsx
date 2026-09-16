import * as React from "react"
import { type LucideIcon } from "lucide-react"

import { cn } from "@/lib/utils"
import { Skeleton } from "@/components/ui/skeleton"

/**
 * StatTile — DESIGN_SYSTEM.md §5.11. A description list, because that is what these are:
 * `StatTileGroup` is the `<dl>`, each tile a `<dt>` label + `<dd>` value.
 *
 * 🔒 DESCRIPTIVE METRICS ONLY. Duration · Words · Segments · Action items · Approved 3/7 ·
 * Speakers. There is no sentiment, engagement, charisma, meeting score or
 * period-over-period delta in this system, and adding one is a product decision, not a
 * component decision: this product's claim is that it records what was said, and a score
 * invented about the people in the room is the opposite of that.
 *
 * FOUR STATES, and the fourth is the interesting one:
 *   value              the number, `text-title-lg tabular-nums` (Inter's true tnum, §4.1,
 *                      so a column of durations does not jitter)
 *   empty              `—`, never `0`
 *   loading            a skeleton in place of the value
 *   NOT APPLICABLE     the tile is OMITTED by the call site. Diarization that never ran is
 *                      not "0 speakers" — printing a zero for a thing that was never
 *                      measured is a false statement in a record people rely on.
 */
export interface StatTileProps {
  label: string
  /** `null`/`undefined` renders `—`. Never pass `0` for "not measured" — omit the tile. */
  value?: React.ReactNode
  /** 14px icon beside the label. */
  icon?: LucideIcon
  /** Optional `text-caption` sub-line, e.g. "142 wpm". */
  sub?: React.ReactNode
  loading?: boolean
  className?: string
}

export function StatTile({
  label,
  value,
  icon: Icon,
  sub,
  loading = false,
  className,
}: StatTileProps) {
  return (
    <div className={cn("rounded-lg border border-border bg-card p-3", className)}>
      <dt className="flex items-center gap-1.5 text-eyebrow uppercase text-muted-foreground">
        {Icon ? <Icon className="size-3.5 shrink-0" aria-hidden /> : null}
        <span className="truncate">{label}</span>
      </dt>
      <dd className="mt-1 text-title-lg tabular-nums text-foreground">
        {loading ? <Skeleton className="h-6 w-16" /> : (value ?? "—")}
      </dd>
      {sub ? <dd className="text-caption text-subtle-foreground">{sub}</dd> : null}
    </div>
  )
}

/**
 * The `<dl>`. A `<div>` wrapper per tile is valid inside a description list (HTML5 grouping)
 * and is what lets each tile be a card.
 */
export const StatTileGroup = React.forwardRef<
  HTMLDListElement,
  React.HTMLAttributes<HTMLDListElement>
>(({ className, ...props }, ref) => (
  <dl
    ref={ref}
    className={cn("grid grid-cols-2 gap-3 sm:grid-cols-3 lg:grid-cols-5", className)}
    {...props}
  />
))
StatTileGroup.displayName = "StatTileGroup"
