import * as React from "react"
import { Slot } from "@radix-ui/react-slot"
import { cva, type VariantProps } from "class-variance-authority"

import { cn } from "@/lib/utils"
import { focusRing } from "@/components/ui/focus-ring"

/**
 * Button — DESIGN_SYSTEM.md §5.4, with the contrast arithmetic in §4.4.4 / §4.4.5.
 *
 * THE RULE THIS FILE ENCODES: the default button is INK, and blue appears only where a
 * human signs. `verified` is the one filled blue action per viewport (`Approve summary`,
 * the frozen consent CTA). Because `buttonVariants` is `cva` + `VariantProps`, that is a
 * type-level constraint rather than a review convention — you cannot reach for blue by
 * accident, you have to type `verified`.
 *
 * REMOVED: `green`, `blue`, `red`, `gray` (raw palette; `green` and `gray` even had
 * hover === idle, i.e. no hover state at all) and `secondary` (zero call sites, and
 * `outline` is the secondary action in this system). Migration, all seven sites in the
 * commit that deletes the keys: green → `outline` for a row-level Approve, `default` for a
 * neutral primary, `verified` for the single `Approve summary`; red → `destructive`.
 *
 * EVERY VARIANT DECLARES IDLE, HOVER, ACTIVE AND DISABLED. A variant shipped with an idle
 * fill only forces the next implementer to invent the interaction colour of the product's
 * most compliance-critical controls, and an invented value cannot be checked against §4.4.
 * The dark hover/active steps go DOWN in lightness for the blue and red families — there is
 * no global "hover = darker" rule, the direction is per-token and measured (ADR-0051).
 */
const buttonVariants = cva(
  cn(
    "inline-flex items-center justify-center gap-2 whitespace-nowrap rounded-md text-label",
    "transition-colors duration-instant ease-out",
    // §5.4 base: icons are 16px and never swallow the click.
    "[&_svg]:pointer-events-none [&_svg]:size-4 [&_svg]:shrink-0",
    // A disabled button is not focusable, so a tooltip on it never opens for keyboard or AT
    // users: §5 requires a visible sibling sentence or aria-describedby at the CALL SITE.
    "disabled:pointer-events-none",
    focusRing("card")
  ),
  {
    variants: {
      variant: {
        // Neutral primary — Generate, Continue, Save, Retry. Ink, not brand blue.
        default: cn(
          "bg-foreground text-background",
          "hover:bg-foreground-hover active:bg-foreground-active",
          "disabled:opacity-50"
        ),
        // "A human decided." The ONE filled blue action in a viewport.
        verified: cn(
          "bg-verified text-verified-foreground",
          "hover:bg-verified-hover active:bg-verified-active",
          "disabled:opacity-50"
        ),
        // Secondary / toolbar / form, and the per-row `Approve`. The boundary is --input,
        // never --border-strong: at 1.65:1 that token is decorative and may not be a
        // control's only boundary (§4.4.5).
        outline: cn(
          "border border-input bg-card",
          "hover:bg-muted hover:border-input-hover",
          "active:bg-surface-3 active:border-input-hover",
          "disabled:opacity-50"
        ),
        // Row and toolbar icon actions.
        ghost: cn(
          "bg-transparent",
          "hover:bg-muted active:bg-surface-3",
          "disabled:opacity-50"
        ),
        // Inline links: the COLOUR never moves, so it stays ≥4.67:1 on every surface it is
        // allowed to sit on. Only the decoration changes.
        link: cn(
          "text-primary-ink no-underline underline-offset-4",
          "hover:underline active:underline active:decoration-2",
          "disabled:opacity-50"
        ),
        // Delete, Deactivate, Confirm reject.
        destructive: cn(
          "bg-destructive text-destructive-foreground",
          "hover:bg-destructive-hover active:bg-destructive-active",
          "disabled:opacity-50"
        ),
        // The record / stop CTA ONLY. The label is --recording-foreground and never
        // --primary-foreground: white on the dark --recording (#EA473E) is 3.84 and fails
        // AA. `disabled:` is declared for the pre-flight states (no device, no permission);
        // 🔒 the call site must never disable this control WHILE RECORDING — stop must
        // always be reachable.
        record: cn(
          "rounded-full bg-recording text-recording-foreground",
          "hover:bg-recording-hover active:bg-recording-active",
          "disabled:opacity-50"
        ),
      },
      size: {
        xs: "h-7 gap-1.5 px-2",
        sm: "h-8 px-3",
        default: "h-9 px-4",
        // Row actions that keep a visible label (§5.15, §8 hit targets: 36px).
        row: "h-9 gap-1.5 px-3",
        lg: "h-10 px-6",
        // 32×32 — toolbar / row icon buttons (§8). Needs an aria-label at the call site.
        icon: "h-8 w-8 p-0",
        // 40×40 — rail items and the record control (§8).
        "icon-lg": "h-10 w-10 p-0",
      },
    },
    defaultVariants: {
      variant: "default",
      size: "default",
    },
  }
)

export interface ButtonProps
  extends React.ButtonHTMLAttributes<HTMLButtonElement>,
    VariantProps<typeof buttonVariants> {
  asChild?: boolean
}

const Button = React.forwardRef<HTMLButtonElement, ButtonProps>(
  ({ className, variant, size, asChild = false, ...props }, ref) => {
    const Comp = asChild ? Slot : "button"
    return (
      <Comp
        className={cn(buttonVariants({ variant, size, className }))}
        ref={ref}
        {...props}
      />
    )
  }
)
Button.displayName = "Button"

export { Button, buttonVariants }
