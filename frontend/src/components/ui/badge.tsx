import * as React from "react"
import { cva, type VariantProps } from "class-variance-authority"

import { cn } from "@/lib/utils"

/**
 * Badge — DESIGN_SYSTEM.md §5.8. ONE pill primitive for the whole product:
 * `h-5 px-1.5 rounded-xs text-caption`, optional 6px leading dot.
 *
 * NEVER COLOUR-ONLY (§8). A pill carries its word *and* an icon — the icon is passed as a
 * child by the call site, and `report/StatusPill.tsx` is the wrapper that makes that
 * structural for the four review states, so no screen can ship a tint with no word. A badge
 * that is only a colour is unreadable for the ~8% of men with a colour-vision deficiency and
 * invisible in a printed export, which is exactly the audience a compliance record has.
 *
 * TONES are the §5.8 table, not a palette: `warning` means something is WRONG and is never
 * used for AI status — that register is `ai` (violet, 220° away from amber), so "a machine
 * drafted this" can never be read as "something failed". Every ink/tint pair here is
 * certified ≥5.6:1 in both themes in §4.4.3.
 *
 * The 12px `text-caption` floor is deliberate: §4.5 makes 12px the smallest size anything a
 * person must read may take, and a status word is exactly that.
 */
const badgeVariants = cva(
  cn(
    "inline-flex h-5 shrink-0 items-center gap-1 whitespace-nowrap rounded-xs border px-1.5",
    "text-caption font-medium",
    // Icons inside a pill are 12px and never swallow a click on the row beneath.
    "[&_svg]:pointer-events-none [&_svg]:size-3 [&_svg]:shrink-0"
  ),
  {
    variants: {
      tone: {
        // Counts, "Legacy" — structure, not status.
        neutral: "border-transparent bg-surface-2 text-muted-foreground",
        // The machine-draft register. Dashed, so "unreviewed" survives greyscale printing.
        ai: "border-dashed border-ai-border bg-ai-surface text-ai-ink",
        // "A human decided."
        verified: "border-verified-border bg-verified-surface text-verified-ink",
        // Human-edited: the verified ink on no tint, so an edit reads as a lighter-weight
        // decision than an approval without introducing a seventh colour.
        edited: "border-verified-border bg-transparent text-verified-ink",
        // Rejected. 🔒 The block text the call site renders is ALSO `line-through` (§5.8).
        destructive:
          "border-destructive-border bg-destructive-surface text-destructive-ink",
        info: "border-info-border bg-info-surface text-info-ink",
        // Things that are WRONG. Never AI status.
        warning: "border-warning-border bg-warning-surface text-warning-ink",
      },
    },
    defaultVariants: {
      tone: "neutral",
    },
  }
)

export interface BadgeProps
  extends React.HTMLAttributes<HTMLSpanElement>,
    VariantProps<typeof badgeVariants> {
  /** 6px leading dot in `currentColor` — decorative, and never the only differentiator. */
  dot?: boolean
}

const Badge = React.forwardRef<HTMLSpanElement, BadgeProps>(
  ({ className, tone, dot, children, ...props }, ref) => (
    <span ref={ref} className={cn(badgeVariants({ tone }), className)} {...props}>
      {dot ? (
        <span aria-hidden className="size-1.5 shrink-0 rounded-full bg-current" />
      ) : null}
      {children}
    </span>
  )
)
Badge.displayName = "Badge"

export { Badge, badgeVariants }
