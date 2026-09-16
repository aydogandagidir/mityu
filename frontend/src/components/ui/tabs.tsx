"use client"

import * as React from "react"
import * as TabsPrimitive from "@radix-ui/react-tabs"

import { cn } from "@/lib/utils"
import { focusRing } from "@/components/ui/focus-ring"

/**
 * Tabs — DESIGN_SYSTEM.md §5.7. Two shapes, one primitive:
 *   `underline`  page level — 2px `--primary` rule under the active tab, `border-b` track
 *   `segmented`  Theme-style switches — `--surface-2` track, active pill `--card` + `--elev-1`
 *
 * THE INDICATOR IS CSS. It is an `::after` on the trigger, driven by `data-state`, not a
 * framer-motion `layoutId` positioned from `offsetLeft` in a `useLayoutEffect` (which is
 * what `app/settings/page.tsx:57-65` does today, and why that underline misaligns on window
 * resize and again when the webfont swaps in — the measurement runs once, before Inter has
 * loaded, and nothing re-runs it). A rule that is part of the tab's own box cannot drift
 * from the tab, needs no measurement, no ref array and no re-render, and it honours
 * `prefers-reduced-motion` through the global CSS rule rather than through a hook.
 */
type TabsVariant = "underline" | "segmented"

const TabsVariantContext = React.createContext<TabsVariant>("underline")

const Tabs = TabsPrimitive.Root

const TabsList = React.forwardRef<
  React.ElementRef<typeof TabsPrimitive.List>,
  React.ComponentPropsWithoutRef<typeof TabsPrimitive.List> & {
    variant?: TabsVariant
  }
>(({ className, variant = "underline", ...props }, ref) => (
  <TabsVariantContext.Provider value={variant}>
    <TabsPrimitive.List
      ref={ref}
      data-variant={variant}
      className={cn(
        "relative inline-flex items-center text-muted-foreground",
        variant === "underline" && "h-9 gap-1 border-b border-border",
        variant === "segmented" && "h-9 gap-0.5 rounded-md bg-surface-2 p-0.5",
        className
      )}
      {...props}
    />
  </TabsVariantContext.Provider>
))
TabsList.displayName = TabsPrimitive.List.displayName

const TabsTrigger = React.forwardRef<
  React.ElementRef<typeof TabsPrimitive.Trigger>,
  React.ComponentPropsWithoutRef<typeof TabsPrimitive.Trigger>
>(({ className, ...props }, ref) => {
  const variant = React.useContext(TabsVariantContext)
  return (
    <TabsPrimitive.Trigger
      ref={ref}
      className={cn(
        "relative inline-flex items-center justify-center gap-2 whitespace-nowrap text-label",
        "transition-colors duration-instant ease-out",
        "text-muted-foreground hover:text-foreground data-[state=active]:text-foreground",
        "disabled:pointer-events-none disabled:opacity-50",
        variant === "underline" &&
          cn(
            "h-9 px-3",
            // The 2px rule. `-bottom-px` lands it ON the list's hairline, so the track and
            // the indicator share one line instead of stacking two.
            "after:absolute after:inset-x-0 after:-bottom-px after:h-0.5 after:rounded-full",
            "after:bg-transparent after:transition-colors after:duration-base after:ease-out",
            "data-[state=active]:after:bg-primary",
            focusRing("card")
          ),
        variant === "segmented" &&
          cn(
            "h-8 flex-1 rounded-sm px-3",
            "data-[state=active]:bg-card data-[state=active]:shadow-elev-1",
            focusRing("surface-2")
          ),
        className
      )}
      {...props}
    />
  )
})
TabsTrigger.displayName = TabsPrimitive.Trigger.displayName

const TabsContent = React.forwardRef<
  React.ElementRef<typeof TabsPrimitive.Content>,
  React.ComponentPropsWithoutRef<typeof TabsPrimitive.Content>
>(({ className, ...props }, ref) => (
  <TabsPrimitive.Content
    ref={ref}
    className={cn("mt-2", focusRing("card"), className)}
    {...props}
  />
))
TabsContent.displayName = TabsPrimitive.Content.displayName

export { Tabs, TabsList, TabsTrigger, TabsContent }
