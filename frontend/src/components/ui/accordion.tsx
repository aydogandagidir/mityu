"use client"

import * as React from "react"
import * as AccordionPrimitive from "@radix-ui/react-accordion"
import { ChevronDown } from "lucide-react"

import { cn } from "@/lib/utils"
import { focusRing } from "@/components/ui/focus-ring"

/**
 * Accordion — rewritten onto the SINGLE shadcn generation this project uses: v3
 * `React.forwardRef` over `@radix-ui/react-*`, not v4 function components over the
 * `radix-ui` umbrella (ADR-C).
 *
 * Why it mattered. This file was the only v4-generation primitive, and it pulled Radix
 * through a second import path. Two generations coexisting also meant two focus idioms —
 * this trigger used `focus-visible:ring-[3px] ring-ring/50` with no offset, a treatment that
 * exists nowhere else in the app — and a v4 component composed inside a v3 `Dialog` or
 * `Tooltip` can read a different copy of a Radix context than the one its parent provides.
 * The no-op `shadow-xs` (a Tailwind v4 class that compiles to NOTHING here — the ADR-0037
 * phantom-class failure) goes with it.
 */
const Accordion = AccordionPrimitive.Root

const AccordionItem = React.forwardRef<
  React.ElementRef<typeof AccordionPrimitive.Item>,
  React.ComponentPropsWithoutRef<typeof AccordionPrimitive.Item>
>(({ className, ...props }, ref) => (
  <AccordionPrimitive.Item
    ref={ref}
    className={cn("border-b border-border last:border-b-0", className)}
    {...props}
  />
))
AccordionItem.displayName = "AccordionItem"

const AccordionTrigger = React.forwardRef<
  React.ElementRef<typeof AccordionPrimitive.Trigger>,
  React.ComponentPropsWithoutRef<typeof AccordionPrimitive.Trigger>
>(({ className, children, ...props }, ref) => (
  <AccordionPrimitive.Header className="flex">
    <AccordionPrimitive.Trigger
      ref={ref}
      className={cn(
        "flex flex-1 items-center justify-between gap-4 rounded-sm py-4 text-left text-title-sm text-foreground",
        "transition-colors duration-instant ease-out hover:text-primary-ink",
        focusRing("card"),
        "disabled:pointer-events-none disabled:opacity-50",
        "[&[data-state=open]>svg]:rotate-180",
        className
      )}
      {...props}
    >
      {children}
      <ChevronDown
        className="pointer-events-none h-4 w-4 shrink-0 text-subtle-foreground transition-transform duration-base ease-out"
        aria-hidden="true"
      />
    </AccordionPrimitive.Trigger>
  </AccordionPrimitive.Header>
))
AccordionTrigger.displayName = AccordionPrimitive.Trigger.displayName

const AccordionContent = React.forwardRef<
  React.ElementRef<typeof AccordionPrimitive.Content>,
  React.ComponentPropsWithoutRef<typeof AccordionPrimitive.Content>
>(({ className, children, ...props }, ref) => (
  <AccordionPrimitive.Content
    ref={ref}
    className="overflow-hidden text-body data-[state=closed]:animate-accordion-up data-[state=open]:animate-accordion-down"
    {...props}
  >
    <div className={cn("pb-4 pt-0", className)}>{children}</div>
  </AccordionPrimitive.Content>
))
AccordionContent.displayName = AccordionPrimitive.Content.displayName

export { Accordion, AccordionContent, AccordionItem, AccordionTrigger }
