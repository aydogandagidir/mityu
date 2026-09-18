import * as React from "react"

import { cn } from "@/lib/utils"

/**
 * Skeleton — DESIGN_SYSTEM.md §5.19. Replaces the five competing loading patterns and the
 * full-screen spinner at `meeting-details/page.tsx:342-363`.
 *
 * TWO PARTS, AND THE SECOND ONE IS THE POINT. `Skeleton` is a decorative block; a screen
 * reader must never be handed "twelve grey rectangles". `SkeletonRegion` is the wrapper
 * that carries 🔒 `role="status" aria-live="polite"` plus one `sr-only` SENTENCE
 * ("Loading meeting report…"), which is the whole announcement. Blocks inside it are
 * `aria-hidden`, so the region says one thing and the shapes say nothing.
 *
 * MOTION. §4.9 asks for a 1.4s shimmer that is flat under reduced motion. The cadence is
 * spelled out on the utility (`animate-[pulse_1.4s_…]`) rather than taken from Tailwind's
 * 2s default; the reduced-motion half is already global — globals.css neutralises every
 * animation to 1ms / 1 iteration, which lands this on a flat `--surface-2` with no
 * information lost, because the sentence above carries the state. A GRADIENT SWEEP would
 * need a new keyframe in `tailwind.config.js` / `globals.css`, which belong to WP1; the
 * pulse is the honest version of §5.19 inside this package's file scope.
 */
const Skeleton = React.forwardRef<HTMLDivElement, React.HTMLAttributes<HTMLDivElement>>(
  ({ className, ...props }, ref) => (
    <div
      ref={ref}
      aria-hidden
      className={cn(
        "rounded-sm bg-surface-2 animate-[pulse_1.4s_cubic-bezier(0.4,0,0.6,1)_infinite]",
        className
      )}
      {...props}
    />
  )
)
Skeleton.displayName = "Skeleton"

export interface SkeletonRegionProps extends React.HTMLAttributes<HTMLDivElement> {
  /** The whole announcement, e.g. "Loading meeting report…". Required for a reason. */
  label: string
}

const SkeletonRegion = React.forwardRef<HTMLDivElement, SkeletonRegionProps>(
  ({ className, label, children, ...props }, ref) => (
    <div ref={ref} role="status" aria-live="polite" className={className} {...props}>
      {children}
      <span className="sr-only">{label}</span>
    </div>
  )
)
SkeletonRegion.displayName = "SkeletonRegion"

/* ── Composed shapes (§5.19) ──────────────────────────────────────────────────────────
 * They exist so a loading screen is one import instead of eight divs invented per screen,
 * and so every loading state announces itself the same way.
 */

/** The Meetings pane while its list loads. */
export function MeetingListSkeleton({ rows = 8 }: { rows?: number }) {
  return (
    <SkeletonRegion label="Loading meetings…" className="space-y-1">
      {Array.from({ length: rows }, (_, i) => (
        <div key={i} className="flex items-center gap-3 rounded-sm px-2 py-2">
          <Skeleton className="size-8 shrink-0" />
          <div className="min-w-0 flex-1 space-y-1.5">
            <Skeleton className="h-3 w-2/3" />
            <Skeleton className="h-2.5 w-1/3" />
          </div>
        </div>
      ))}
    </SkeletonRegion>
  )
}

/** The report column: header, two section cards, six transcript lines. */
export function ReportSkeleton() {
  return (
    <SkeletonRegion label="Loading meeting report…" className="space-y-4">
      <div className="space-y-2">
        <Skeleton className="h-6 w-1/2" />
        <Skeleton className="h-3 w-1/3" />
      </div>
      {[0, 1].map((card) => (
        <div key={card} className="rounded-lg border border-border bg-card">
          <div className="flex h-11 items-center gap-2 border-b border-border px-4">
            <Skeleton className="size-5" />
            <Skeleton className="h-3 w-32" />
          </div>
          <div className="space-y-2 p-4">
            <Skeleton className="h-3 w-full" />
            <Skeleton className="h-3 w-11/12" />
            <Skeleton className="h-3 w-3/5" />
          </div>
        </div>
      ))}
      <div className="space-y-2">
        {Array.from({ length: 6 }, (_, i) => (
          <div key={i} className="flex gap-3">
            <Skeleton className="h-3 w-10 shrink-0" />
            <Skeleton className="h-3 flex-1" />
          </div>
        ))}
      </div>
    </SkeletonRegion>
  )
}

/** A Settings section: three cards. */
export function SettingsSectionSkeleton({ cards = 3 }: { cards?: number }) {
  return (
    <SkeletonRegion label="Loading settings…" className="space-y-3">
      {Array.from({ length: cards }, (_, i) => (
        <div key={i} className="space-y-2 rounded-lg border border-border bg-card p-4">
          <Skeleton className="h-3 w-40" />
          <Skeleton className="h-3 w-full" />
          <Skeleton className="h-8 w-52" />
        </div>
      ))}
    </SkeletonRegion>
  )
}

/** The transcript spine: a timestamp gutter and its lines. */
export function TranscriptSkeleton({ rows = 6 }: { rows?: number }) {
  return (
    <SkeletonRegion label="Loading transcript…" className="space-y-2">
      {Array.from({ length: rows }, (_, i) => (
        <div key={i} className="flex gap-3">
          <Skeleton className="h-3 w-10 shrink-0" />
          <Skeleton className={cn("h-3", i % 3 === 2 ? "w-2/3" : "flex-1")} />
        </div>
      ))}
    </SkeletonRegion>
  )
}

export { Skeleton, SkeletonRegion }
