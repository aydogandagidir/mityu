import * as React from "react"

import { cn } from "@/lib/utils"

/**
 * Surfaces — DESIGN_SYSTEM.md §5.6. Three of them, and only three:
 *
 *   `Card`    `bg-card border border-border rounded-lg`, header `px-4 h-11`, body `p-4`,
 *             footer `px-4 py-3 border-t`. NO SHADOW — §4.8 is hairline-first, and a
 *             shadow on a card that sits on `--background` adds a second boundary that
 *             says nothing the border does not.
 *   `Section` headerless grouping: a `text-eyebrow` label and `space-y-3`.
 *   `Well`    `bg-surface-2 rounded-sm p-3` for INERT content — a path, a code block, the
 *             original text of an edited block.
 *
 * MAXIMUM NESTING DEPTH 2 (card → well). The Beta tab currently nests three levels of
 * bordered box, which reads as a UI that does not know what it is emphasising. If a well
 * needs a well, the outer one is a layout container and should lose its surface.
 *
 * This replaces the ~20 literal `"bg-card rounded-lg border border-border p-6 shadow-sm"`
 * strings in Settings. A component, not a string, is what lets §4.8 change once.
 */
/**
 * The Card surface as a string, for the few places that need the same surface on an element
 * this component cannot be (a `<section>` with its own semantics, e.g. `SectionCard`).
 * Exported so there is still exactly ONE definition of what a card looks like.
 */
export const CARD_SURFACE =
  "rounded-lg border border-border bg-card text-card-foreground"

const Card = React.forwardRef<HTMLDivElement, React.HTMLAttributes<HTMLDivElement>>(
  ({ className, ...props }, ref) => (
    <div ref={ref} className={cn(CARD_SURFACE, className)} {...props} />
  )
)
Card.displayName = "Card"

/** 44px tall, so a header with only a title lines up with one that carries row actions. */
const CardHeader = React.forwardRef<HTMLDivElement, React.HTMLAttributes<HTMLDivElement>>(
  ({ className, ...props }, ref) => (
    <div
      ref={ref}
      className={cn(
        "flex h-11 items-center gap-2 border-b border-border px-4",
        className
      )}
      {...props}
    />
  )
)
CardHeader.displayName = "CardHeader"

const CardTitle = React.forwardRef<
  HTMLHeadingElement,
  React.HTMLAttributes<HTMLHeadingElement>
>(({ className, ...props }, ref) => (
  // `min-w-0` is not decoration: a flex item's default `min-width: auto` stops `truncate`
  // from ever firing, so a long section title would push the count and the AiLabel out of
  // the header instead of ellipsizing.
  <h3
    ref={ref}
    className={cn("min-w-0 truncate text-title text-foreground", className)}
    {...props}
  />
))
CardTitle.displayName = "CardTitle"

const CardDescription = React.forwardRef<
  HTMLParagraphElement,
  React.HTMLAttributes<HTMLParagraphElement>
>(({ className, ...props }, ref) => (
  <p ref={ref} className={cn("text-caption text-muted-foreground", className)} {...props} />
))
CardDescription.displayName = "CardDescription"

const CardContent = React.forwardRef<HTMLDivElement, React.HTMLAttributes<HTMLDivElement>>(
  ({ className, ...props }, ref) => (
    <div ref={ref} className={cn("p-4", className)} {...props} />
  )
)
CardContent.displayName = "CardContent"

const CardFooter = React.forwardRef<HTMLDivElement, React.HTMLAttributes<HTMLDivElement>>(
  ({ className, ...props }, ref) => (
    <div
      ref={ref}
      className={cn("flex items-center gap-2 border-t border-border px-4 py-3", className)}
      {...props}
    />
  )
)
CardFooter.displayName = "CardFooter"

export interface SectionProps extends React.HTMLAttributes<HTMLElement> {
  /** Rendered as an uppercase `text-eyebrow` label. Omit for an unlabelled grouping. */
  label?: string
}

const Section = React.forwardRef<HTMLElement, SectionProps>(
  ({ className, label, children, ...props }, ref) => (
    <section ref={ref} className={cn("space-y-3", className)} {...props}>
      {label ? (
        <h2 className="text-eyebrow uppercase text-muted-foreground">{label}</h2>
      ) : null}
      {children}
    </section>
  )
)
Section.displayName = "Section"

/** Inert content only. A Well never contains a Card, and never a second Well. */
const Well = React.forwardRef<HTMLDivElement, React.HTMLAttributes<HTMLDivElement>>(
  ({ className, ...props }, ref) => (
    <div
      ref={ref}
      className={cn("rounded-sm bg-surface-2 p-3 text-body text-foreground", className)}
      {...props}
    />
  )
)
Well.displayName = "Well"

export {
  Card,
  CardHeader,
  CardTitle,
  CardDescription,
  CardContent,
  CardFooter,
  Section,
  Well,
}
