"use client"

import * as React from "react"
import * as PopoverPrimitive from "@radix-ui/react-popover"

import { cn } from "@/lib/utils"
import { focusRing } from "@/components/ui/focus-ring"

const Popover = PopoverPrimitive.Root

const PopoverTrigger = PopoverPrimitive.Trigger

const PopoverAnchor = PopoverPrimitive.Anchor

/**
 * Popover — `--popover` + hairline + `--elev-1` (§4.8), `rounded-xl` (§4.7: panels).
 *
 * The panel used to carry a bare `outline-none`: Radix moves focus onto the content when the
 * popover opens, and stripping the outline left a keyboard user with NO indication of where
 * focus went. It now carries the shared treatment instead — visible only for `focus-visible`,
 * so a mouse user never sees a ring on a panel they clicked open. The offset is
 * `ring-offset-background`: the panel floats over the page, so the page is what the 2px gap
 * is drawn in. Controls INSIDE the panel take `ring-offset-popover` (§5's table).
 */
/** z-70 shares the dialog band on purpose — see `select.tsx` for why §4.10's "50" row is not enough. */
const PopoverContent = React.forwardRef<
  React.ElementRef<typeof PopoverPrimitive.Content>,
  React.ComponentPropsWithoutRef<typeof PopoverPrimitive.Content>
>(({ className, align = "center", sideOffset = 6, ...props }, ref) => (
  <PopoverPrimitive.Portal>
    <PopoverPrimitive.Content
      ref={ref}
      align={align}
      sideOffset={sideOffset}
      className={cn(
        "z-70 w-72 rounded-xl border border-border bg-popover p-4 text-popover-foreground shadow-elev-1",
        "dark:border-border-strong",
        focusRing("background"),
        "data-[state=open]:animate-in data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0 data-[state=closed]:zoom-out-95 data-[state=open]:zoom-in-95 data-[side=bottom]:slide-in-from-top-2 data-[side=left]:slide-in-from-right-2 data-[side=right]:slide-in-from-left-2 data-[side=top]:slide-in-from-bottom-2 origin-[--radix-popover-content-transform-origin]",
        className
      )}
      {...props}
    />
  </PopoverPrimitive.Portal>
))
PopoverContent.displayName = PopoverPrimitive.Content.displayName

export { Popover, PopoverTrigger, PopoverContent, PopoverAnchor }
