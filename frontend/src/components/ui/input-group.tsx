"use client"

import * as React from "react"
import { cva, type VariantProps } from "class-variance-authority"

import { cn } from "@/lib/utils"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Textarea } from "@/components/ui/textarea"
import { focusRingWithin, type FieldGroupSurface } from "@/components/ui/focus-ring"

/**
 * InputGroup — a field with leading/trailing adornments (the Meetings-pane search, the
 * locked API-key field). Rewritten onto the v3 `forwardRef` generation (ADR-C).
 *
 * Two real defects go with the rewrite, not just the generation:
 *   • `shadow-xs` and `dark:bg-input/30` were Tailwind-v4 idioms — `shadow-xs` compiles to
 *     NOTHING here, and `bg-input/30` painted a translucent BOUNDARY colour as a FILL.
 *   • The focus treatment was `ring-1` with no offset (1px fails SC 2.4.13's 2px perimeter,
 *     and without the offset the ring against the fill is 1.22:1). The ring now belongs to
 *     the GROUP — a composed field reads as one control — through `focusRingWithin`, which
 *     keys on the control (`[data-slot=input-group-control]`) rather than on any focusable
 *     descendant, so tabbing to the clear button draws ONE ring, around the button.
 * The inner control keeps its own ring suppressed for the same reason: one control, one ring.
 */
const InputGroup = React.forwardRef<
  HTMLDivElement,
  React.ComponentPropsWithoutRef<"div"> & {
    /** What the composed field SITS ON, for the §4.4.6 focus offset. */
    surface?: FieldGroupSurface
  }
>(({ className, surface = "card", ...props }, ref) => (
  <div
    ref={ref}
    data-slot="input-group"
    role="group"
    className={cn(
      "group/input-group relative flex w-full items-center rounded-sm border border-input bg-card",
      "transition-colors duration-instant ease-out hover:border-input-hover",
      "h-9 has-[>textarea]:h-auto",

      // Alignment-driven layout.
      "has-[>[data-align=inline-start]]:[&>input]:pl-2",
      "has-[>[data-align=inline-end]]:[&>input]:pr-2",
      "has-[>[data-align=block-start]]:h-auto has-[>[data-align=block-start]]:flex-col has-[>[data-align=block-start]]:[&>input]:pb-3",
      "has-[>[data-align=block-end]]:h-auto has-[>[data-align=block-end]]:flex-col has-[>[data-align=block-end]]:[&>input]:pt-3",

      focusRingWithin(surface),
      "has-[[data-slot=input-group-control]:focus-visible]:border-ring",

      // Invalid is a boundary change, never colour alone: the message is the call site's.
      "has-[[data-slot][aria-invalid=true]]:border-destructive",

      className
    )}
    {...props}
  />
))
InputGroup.displayName = "InputGroup"

const inputGroupAddonVariants = cva(
  "flex h-auto cursor-text select-none items-center justify-center gap-2 py-1.5 text-label text-subtle-foreground group-data-[disabled=true]/input-group:opacity-50 [&>kbd]:rounded-xs [&>svg:not([class*='size-'])]:size-4",
  {
    variants: {
      align: {
        "inline-start":
          "order-first pl-3 has-[>button]:ml-[-0.45rem] has-[>kbd]:ml-[-0.35rem]",
        "inline-end":
          "order-last pr-3 has-[>button]:mr-[-0.4rem] has-[>kbd]:mr-[-0.35rem]",
        "block-start":
          "[.border-b]:pb-3 order-first w-full justify-start px-3 pt-3 group-has-[>input]/input-group:pt-2.5",
        "block-end":
          "[.border-t]:pt-3 order-last w-full justify-start px-3 pb-3 group-has-[>input]/input-group:pb-2.5",
      },
    },
    defaultVariants: {
      align: "inline-start",
    },
  }
)

const InputGroupAddon = React.forwardRef<
  HTMLDivElement,
  React.ComponentPropsWithoutRef<"div"> &
    VariantProps<typeof inputGroupAddonVariants>
>(({ className, align = "inline-start", onClick, ...props }, ref) => (
  <div
    ref={ref}
    role="group"
    data-slot="input-group-addon"
    data-align={align}
    className={cn(inputGroupAddonVariants({ align }), className)}
    onClick={(event) => {
      onClick?.(event)
      if ((event.target as HTMLElement).closest("button")) {
        return
      }
      event.currentTarget.parentElement?.querySelector("input")?.focus()
    }}
    {...props}
  />
))
InputGroupAddon.displayName = "InputGroupAddon"

const inputGroupButtonVariants = cva("flex items-center gap-2", {
  variants: {
    size: {
      xs: "h-6 gap-1 rounded-xs px-2 has-[>svg]:px-2 [&>svg:not([class*='size-'])]:size-3.5",
      sm: "h-8 gap-1.5 rounded-sm px-2.5 has-[>svg]:px-2.5",
      "icon-xs": "size-6 rounded-xs p-0 has-[>svg]:p-0",
      "icon-sm": "size-8 p-0 has-[>svg]:p-0",
    },
  },
  defaultVariants: {
    size: "xs",
  },
})

const InputGroupButton = React.forwardRef<
  HTMLButtonElement,
  Omit<React.ComponentPropsWithoutRef<typeof Button>, "size"> &
    VariantProps<typeof inputGroupButtonVariants>
>(({ className, type = "button", variant = "ghost", size = "xs", ...props }, ref) => (
  <Button
    ref={ref}
    type={type}
    data-size={size}
    variant={variant}
    className={cn(inputGroupButtonVariants({ size }), className)}
    {...props}
  />
))
InputGroupButton.displayName = "InputGroupButton"

const InputGroupText = React.forwardRef<
  HTMLSpanElement,
  React.ComponentPropsWithoutRef<"span">
>(({ className, ...props }, ref) => (
  <span
    ref={ref}
    className={cn(
      "flex items-center gap-2 text-label text-muted-foreground [&_svg:not([class*='size-'])]:size-4 [&_svg]:pointer-events-none",
      className
    )}
    {...props}
  />
))
InputGroupText.displayName = "InputGroupText"

const InputGroupInput = React.forwardRef<
  HTMLInputElement,
  React.ComponentPropsWithoutRef<"input">
>(({ className, ...props }, ref) => (
  <Input
    ref={ref}
    data-slot="input-group-control"
    className={cn(
      "flex-1 rounded-none border-0 bg-transparent",
      "hover:border-0 focus-visible:border-0 focus-visible:ring-0 focus-visible:ring-offset-0",
      className
    )}
    {...props}
  />
))
InputGroupInput.displayName = "InputGroupInput"

const InputGroupTextarea = React.forwardRef<
  HTMLTextAreaElement,
  React.ComponentPropsWithoutRef<"textarea">
>(({ className, ...props }, ref) => (
  <Textarea
    ref={ref}
    data-slot="input-group-control"
    className={cn(
      "flex-1 resize-none rounded-none border-0 bg-transparent py-3",
      "hover:border-0 focus-visible:border-0 focus-visible:ring-0 focus-visible:ring-offset-0",
      className
    )}
    {...props}
  />
))
InputGroupTextarea.displayName = "InputGroupTextarea"

export {
  InputGroup,
  InputGroupAddon,
  InputGroupButton,
  InputGroupText,
  InputGroupInput,
  InputGroupTextarea,
}
