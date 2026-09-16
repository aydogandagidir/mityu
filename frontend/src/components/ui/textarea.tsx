import * as React from "react"

import { cn } from "@/lib/utils"
import { focusRing, type FocusSurface } from "@/components/ui/focus-ring"

/** Textarea — DESIGN_SYSTEM.md §5.5. Same six states as `Input`, `min-h-20` instead of `h-9`. */
export interface TextareaProps extends React.ComponentProps<"textarea"> {
  /** What the field SITS ON, for the §4.4.6 focus offset. */
  surface?: FocusSurface
}

const Textarea = React.forwardRef<HTMLTextAreaElement, TextareaProps>(
  ({ className, surface = "card", ...props }, ref) => {
  return (
    <textarea
      className={cn(
        "flex min-h-20 w-full rounded-sm border border-input bg-card px-3 py-2 text-body text-foreground",
        "transition-colors duration-instant ease-out",
        "placeholder:text-subtle-foreground",
        "hover:border-input-hover",
        "focus-visible:border-ring",
        focusRing(surface),
        "aria-[invalid=true]:border-destructive",
        "disabled:cursor-not-allowed disabled:bg-muted disabled:text-muted-foreground disabled:hover:border-input",
        // 🔒 `:not(:disabled)` — see the note in input.tsx: `:read-only` also matches a
        // DISABLED control, and the read-only variant outranks `disabled` by source order, so the
        // unscoped form ships a disabled textarea with no boundary (§5.5 requires
        // `border-input`).
        "[&:read-only:not(:disabled)]:border-transparent",
        "[&:read-only:not(:disabled)]:bg-surface-2",
        "[&:read-only:not(:disabled)]:hover:border-transparent",
        className
      )}
      ref={ref}
      {...props}
    />
  )
})
Textarea.displayName = "Textarea"

export { Textarea }
