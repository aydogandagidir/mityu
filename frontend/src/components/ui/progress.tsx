"use client"

import * as React from "react"
import * as ProgressPrimitive from "@radix-ui/react-progress"

import { cn } from "@/lib/utils"

/**
 * Progress — DESIGN_SYSTEM.md §5.16: one 6px track in `--surface-2` with a `--primary`
 * fill, replacing five hand-rolled bars (h-1.5 / h-2 / h-3, gradient-gray, blue-600).
 *
 * 🔒 `role="progressbar"` and `aria-valuenow/min/max` come from Radix. `aria-label` does
 * NOT — §8 requires one everywhere, so every call site passes it.
 */
const Progress = React.forwardRef<
  React.ElementRef<typeof ProgressPrimitive.Root>,
  React.ComponentPropsWithoutRef<typeof ProgressPrimitive.Root>
>(({ className, value, ...props }, ref) => (
  <ProgressPrimitive.Root
    ref={ref}
    className={cn(
      "relative h-1.5 w-full overflow-hidden rounded-full bg-surface-2",
      className
    )}
    {...props}
  >
    <ProgressPrimitive.Indicator
      className="h-full w-full flex-1 rounded-full bg-primary transition-transform duration-base ease-out"
      style={{ transform: `translateX(-${100 - (value || 0)}%)` }}
    />
  </ProgressPrimitive.Root>
))
Progress.displayName = ProgressPrimitive.Root.displayName

export { Progress }
