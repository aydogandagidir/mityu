"use client"

import * as React from "react"
import * as TabsPrimitive from "@radix-ui/react-tabs"

import { cn } from "@/lib/utils"
import { focusRing, type FocusSurface } from "@/components/ui/focus-ring"

/**
 * Tabs — DESIGN_SYSTEM.md §5.7. Two shapes, one primitive, plus an unstyled default:
 *   `plain`      🔒 THE DEFAULT. No chrome at all — no track, no height, no `::after`.
 *   `underline`  page level — 2px `--primary` rule under the active tab, `border-b` track
 *   `segmented`  Theme-style switches — `--surface-2` track, active pill `--card` + `--elev-1`
 *
 * WHY THE DEFAULT IS `plain` AND NOT `underline`. The app's only live tabs consumer,
 * `app/settings/page.tsx`, still owns its own framer-motion `layoutId="underline"` bar and
 * sizes its triggers with `py-4`. An `underline` DEFAULT gave that screen two 2px primary
 * rules — the CSS one and the measured one — and collapsed the trigger box from ~52px to
 * `h-9` while it kept 16px of vertical padding, and the call site could not opt out
 * (`data-[state=active]:after:bg-primary` sits in a modifier group nothing at the call site
 * can merge away). A variant is a migration a package performs, not something a primitive
 * performs on every consumer at once: WP12 opts `/settings` into `underline` in the same
 * change that deletes the framer-motion bar.
 *
 * THE INDICATOR IS CSS. It is an `::after` on the trigger, driven by `data-state`, not a
 * framer-motion `layoutId` positioned from `offsetLeft` in a `useLayoutEffect` (which is
 * what `app/settings/page.tsx:57-65` does today, and why that underline misaligns on window
 * resize and again when the webfont swaps in — the measurement runs once, before Inter has
 * loaded, and nothing re-runs it). A rule that is part of the tab's own box cannot drift
 * from the tab, needs no measurement, no ref array and no re-render, and it honours
 * `prefers-reduced-motion` through the global CSS rule rather than through a hook.
 */
type TabsVariant = "plain" | "underline" | "segmented"

const TabsVariantContext = React.createContext<TabsVariant>("plain")

const Tabs = TabsPrimitive.Root

const TabsList = React.forwardRef<
  React.ElementRef<typeof TabsPrimitive.List>,
  React.ComponentPropsWithoutRef<typeof TabsPrimitive.List> & {
    variant?: TabsVariant
  }
>(({ className, variant = "plain", ...props }, ref) => (
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
  React.ComponentPropsWithoutRef<typeof TabsPrimitive.Trigger> & {
    /** What the tab strip SITS ON, for the §4.4.6 focus offset. Defaults per variant. */
    surface?: FocusSurface
  }
>(({ className, surface, ...props }, ref) => {
  const variant = React.useContext(TabsVariantContext)
  // A `segmented` track is --surface-2; `underline` and `plain` sit on whatever holds them.
  const offset: FocusSurface =
    surface ?? (variant === "segmented" ? "surface-2" : "card")
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
            focusRing(offset)
          ),
        variant === "segmented" &&
          cn(
            "h-8 flex-1 rounded-sm px-3",
            "data-[state=active]:bg-card data-[state=active]:shadow-elev-1",
            focusRing(offset)
          ),
        // `plain` contributes the focus treatment and nothing else: geometry stays the call
        // site's until it opts into a shape.
        variant === "plain" && focusRing(offset),
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
