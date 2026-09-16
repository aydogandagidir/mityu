"use client"

import * as React from "react"
import * as ProgressPrimitive from "@radix-ui/react-progress"

import { cn } from "@/lib/utils"

/**
 * Progress — DESIGN_SYSTEM.md §5.16: one 6px track in `--surface-2` with a `--primary`
 * fill, replacing five hand-rolled bars (h-1.5 / h-2 / h-3, gradient-gray, blue-600) across
 * `DownloadProgressStep`, `UpdateDialog`, `ImportAudioDialog`, `DownloadToastContent`,
 * `ModelDownloadProgress` and `RetranscribeDialog`.
 *
 * 🔒 `role="progressbar"` and `aria-valuenow/min/max` come from Radix. `aria-label` does
 * NOT — §8 requires one everywhere (today: nowhere), so it is a REQUIRED prop here rather
 * than a convention: a bar with no name announces "progressbar, 40%" of nothing.
 *
 * `tone="destructive"` is the failed download / failed import state. It is not decoration:
 * the fill colour is a second channel beside the call site's visible error sentence, never
 * the only one (§8 — status is never colour alone).
 *
 * INDETERMINATE. `indeterminate` drops `value` entirely (Radix then omits `aria-valuenow`,
 * which is exactly the ARIA contract for an unknown-progress bar) and sweeps a half-width
 * fill ACROSS THE WHOLE TRACK over 1.4s, via the `progress-indeterminate` keyframe
 * (`translateX(-100%)` → `translateX(200%)`, i.e. fully off one edge to fully off the
 * other). The previous composition — `animate-in slide-in-from-left-full` — moved the fill
 * by its OWN width, so it never entered the right half, and its arbitrary `duration-`
 * value compiled to NOTHING (that namespace is claimed by BOTH core `transitionDuration`
 * and tailwindcss-animate's `animationDuration`, so Tailwind reported it ambiguous and
 * emitted no rule), leaving the sweep at `.animate-in`'s 150ms — 9× too fast. The literal
 * class is not spelled out anywhere under `content`, because writing it in a comment is
 * enough to bring the build warning back.
 *
 * 🔒 REDUCED MOTION IS NOT "THE SAME BAR, HELD STILL". With the sweep neutralised, a
 * half-width fill parked at the left edge is indistinguishable from a determinate 50% —
 * an unknown-progress indicator reading as a precise number is worse than no indicator.
 * `data-slot="progress-indeterminate"` is what globals.css's reduced-motion block keys on
 * to turn the bar into a FULL-WIDTH STRIPED track (§5.16), which cannot be read as a
 * percentage. The call site's required TEXT state ("Preparing…") carries the rest.
 */
export interface ProgressProps
  extends Omit<
    React.ComponentPropsWithoutRef<typeof ProgressPrimitive.Root>,
    "value" | "aria-label"
  > {
  /** 0-100. Ignored when `indeterminate`. */
  value?: number | null
  /** 🔒 §8: every progressbar carries a name. */
  "aria-label": string
  tone?: "default" | "destructive"
  indeterminate?: boolean
}

const Progress = React.forwardRef<
  React.ElementRef<typeof ProgressPrimitive.Root>,
  ProgressProps
>(({ className, value, tone = "default", indeterminate = false, ...props }, ref) => (
  <ProgressPrimitive.Root
    ref={ref}
    value={indeterminate ? null : value}
    className={cn(
      "relative h-1.5 w-full overflow-hidden rounded-full bg-surface-2",
      className
    )}
    {...props}
  >
    {indeterminate ? (
      <ProgressPrimitive.Indicator
        data-slot="progress-indeterminate"
        className={cn(
          "h-full w-1/2 rounded-full animate-progress-indeterminate",
          tone === "destructive" ? "bg-destructive" : "bg-primary"
        )}
      />
    ) : (
      <ProgressPrimitive.Indicator
        className={cn(
          "h-full w-full flex-1 rounded-full transition-transform duration-base ease-out",
          tone === "destructive" ? "bg-destructive" : "bg-primary"
        )}
        style={{ transform: `translateX(-${100 - (value || 0)}%)` }}
      />
    )}
  </ProgressPrimitive.Root>
))
Progress.displayName = ProgressPrimitive.Root.displayName

export { Progress }
