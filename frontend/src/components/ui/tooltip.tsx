"use client"

import * as React from "react"
import * as TooltipPrimitive from "@radix-ui/react-tooltip"

import { cn } from "@/lib/utils"

/**
 * Tooltip — DESIGN_SYSTEM.md §5.18.
 *
 * NO LONGER BRAND-BLUE INVERTED. The shipped tooltip was `bg-primary text-primary-foreground`,
 * so every hover anywhere in the app flashed a blue plate — including over the red record
 * button, which read as a second, competing action. A tooltip is a surface, not an action:
 * `--popover` with a hairline, `--elev-1`.
 *
 * IT OPENS ON FOCUS AS WELL AS HOVER — that is Radix's own `onFocus` behaviour on
 * `Tooltip.Trigger`, and it is the reason icon-only controls may carry one. It is also why
 * §5 forbids a tooltip on a DISABLED control: a disabled button is not focusable, so the
 * tooltip never opens for keyboard or AT users and the reason has to be a visible sibling
 * sentence or `aria-describedby` instead.
 *
 * `delayDuration` 300 / `skipDelayDuration` 0 are the §5.18 defaults, set on the provider so
 * one `TooltipProvider` in `AppShell` governs the whole app.
 */
const TooltipProvider = ({
  delayDuration = 300,
  skipDelayDuration = 0,
  ...props
}: React.ComponentPropsWithoutRef<typeof TooltipPrimitive.Provider>) => (
  <TooltipPrimitive.Provider
    delayDuration={delayDuration}
    skipDelayDuration={skipDelayDuration}
    {...props}
  />
)
TooltipProvider.displayName = "TooltipProvider"

const Tooltip = TooltipPrimitive.Root

const TooltipTrigger = TooltipPrimitive.Trigger

/** z-70 shares the dialog band on purpose — see `select.tsx` for why §4.10's "50" row is not enough. */
const TooltipContent = React.forwardRef<
  React.ElementRef<typeof TooltipPrimitive.Content>,
  React.ComponentPropsWithoutRef<typeof TooltipPrimitive.Content>
>(({ className, sideOffset = 6, ...props }, ref) => (
  <TooltipPrimitive.Portal>
    <TooltipPrimitive.Content
      ref={ref}
      sideOffset={sideOffset}
      className={cn(
        "z-70 overflow-hidden rounded-sm border border-border bg-popover px-2 py-1 text-caption text-popover-foreground shadow-elev-1",
        "dark:border-border-strong",
        "animate-in fade-in-0 zoom-in-95 data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=closed]:zoom-out-95 data-[side=bottom]:slide-in-from-top-2 data-[side=left]:slide-in-from-right-2 data-[side=right]:slide-in-from-left-2 data-[side=top]:slide-in-from-bottom-2 origin-[--radix-tooltip-content-transform-origin]",
        className
      )}
      {...props}
    />
  </TooltipPrimitive.Portal>
))
TooltipContent.displayName = TooltipPrimitive.Content.displayName

export { Tooltip, TooltipTrigger, TooltipContent, TooltipProvider }
