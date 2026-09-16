"use client"

import * as React from "react"
import * as SwitchPrimitives from "@radix-ui/react-switch"

import { cn } from "@/lib/utils"
import { focusRing, type FocusSurface } from "@/components/ui/focus-ring"

/**
 * Switch — DESIGN_SYSTEM.md §5.5: 36×20 track, 16px thumb, `--input` off / `--primary` on.
 * The thumb is `--primary-foreground` (white in both themes) so it reads on the grey track
 * and on the blue fill alike. State is never conveyed by colour alone — every switch in
 * this product carries a label, and the consent and analytics switches carry a sentence.
 */
const Switch = React.forwardRef<
  React.ElementRef<typeof SwitchPrimitives.Root>,
  React.ComponentPropsWithoutRef<typeof SwitchPrimitives.Root> & {
    /** What the switch SITS ON, for the §4.4.6 focus offset. */
    surface?: FocusSurface
  }
>(({ className, surface = "card", ...props }, ref) => (
  <SwitchPrimitives.Root
    className={cn(
      "peer inline-flex h-5 w-9 shrink-0 cursor-pointer items-center rounded-full border-2 border-transparent",
      "transition-colors duration-fast ease-out",
      "data-[state=unchecked]:bg-input data-[state=checked]:bg-primary",
      "hover:data-[state=unchecked]:bg-input-hover hover:data-[state=checked]:bg-primary-hover",
      "active:data-[state=checked]:bg-primary-active",
      focusRing(surface),
      "disabled:cursor-not-allowed disabled:opacity-50",
      className
    )}
    {...props}
    ref={ref}
  >
    <SwitchPrimitives.Thumb
      className={cn(
        "pointer-events-none block h-4 w-4 rounded-full bg-primary-foreground ring-0",
        "transition-transform duration-fast ease-out",
        "data-[state=checked]:translate-x-4 data-[state=unchecked]:translate-x-0"
      )}
    />
  </SwitchPrimitives.Root>
))
Switch.displayName = SwitchPrimitives.Root.displayName

export { Switch }
