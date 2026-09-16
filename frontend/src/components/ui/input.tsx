import * as React from "react"

import { cn } from "@/lib/utils"
import { focusRing } from "@/components/ui/focus-ring"

/**
 * Input — DESIGN_SYSTEM.md §5.5. Six states, all declared here so no field re-invents them.
 *
 * The boundary is `--input` / `--input-hover`, never `--border-strong`: that token is
 * decorative at 1.65:1 and using it for hover would drop the field's ONLY identifier below
 * 3:1 (SC 1.4.11, §4.4.5). `--input` was re-tuned in WP1 precisely so a field keeps its
 * boundary on all six declared grounds — inside a `Well` (`--surface-2`, 3.52/3.64) and in
 * the `bg-background` Meetings pane (3.71/4.12) as much as on a card.
 *
 * The placeholder is `--subtle-foreground`, which now clears 4.5:1 on every declared
 * surface; at its previous value it measured 4.34 inside a Well.
 */
const Input = React.forwardRef<HTMLInputElement, React.ComponentProps<"input">>(
  ({ className, type, ...props }, ref) => {
    return (
      <input
        type={type}
        className={cn(
          "flex h-9 w-full rounded-sm border border-input bg-card px-3 py-1 text-body text-foreground",
          "transition-colors duration-instant ease-out",
          "placeholder:text-subtle-foreground",
          "file:border-0 file:bg-transparent file:text-label file:text-foreground",
          "hover:border-input-hover",
          // Focus shows the ring AND moves the boundary, so the field is identifiable even
          // where a high-contrast mode drops the box-shadow.
          "focus-visible:border-ring",
          focusRing("card"),
          // Invalid is never colour-only: the call site pairs this with a role="alert"
          // message in --destructive-ink (§5.5).
          "aria-[invalid=true]:border-destructive",
          // Disabled keeps border-input (3.52 light / 3.55 dark on --muted) rather than
          // fading the boundary away with the text.
          "disabled:cursor-not-allowed disabled:bg-muted disabled:text-muted-foreground disabled:hover:border-input",
          // Read-only is inert content, not a broken control: a well, no boundary.
          "read-only:border-transparent read-only:bg-surface-2 read-only:hover:border-transparent",
          className
        )}
        ref={ref}
        {...props}
      />
    )
  }
)
Input.displayName = "Input"

export { Input }
