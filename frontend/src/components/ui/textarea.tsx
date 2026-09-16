import * as React from "react"

import { cn } from "@/lib/utils"
import { focusRing } from "@/components/ui/focus-ring"

/** Textarea — DESIGN_SYSTEM.md §5.5. Same six states as `Input`, `min-h-20` instead of `h-9`. */
const Textarea = React.forwardRef<
  HTMLTextAreaElement,
  React.ComponentProps<"textarea">
>(({ className, ...props }, ref) => {
  return (
    <textarea
      className={cn(
        "flex min-h-20 w-full rounded-sm border border-input bg-card px-3 py-2 text-body text-foreground",
        "transition-colors duration-instant ease-out",
        "placeholder:text-subtle-foreground",
        "hover:border-input-hover",
        "focus-visible:border-ring",
        focusRing("card"),
        "aria-[invalid=true]:border-destructive",
        "disabled:cursor-not-allowed disabled:bg-muted disabled:text-muted-foreground disabled:hover:border-input",
        "read-only:border-transparent read-only:bg-surface-2 read-only:hover:border-transparent",
        className
      )}
      ref={ref}
      {...props}
    />
  )
})
Textarea.displayName = "Textarea"

export { Textarea }
