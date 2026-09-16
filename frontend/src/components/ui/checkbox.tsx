"use client"

import * as React from "react"
import { Check } from "lucide-react"

import { cn } from "@/lib/utils"
import { focusRing } from "@/components/ui/focus-ring"

/**
 * Checkbox — DESIGN_SYSTEM.md §5.5: 16×16, `rounded-xs`, `--input` boundary, `--primary`
 * when checked. It replaces the two raw `<input type="checkbox">` that shipped their own
 * styling in `consent/RecordingConsentDialog.tsx` and `lib/recordingNotification.tsx` —
 * the consent surfaces, i.e. exactly the controls that must not look improvised.
 *
 * IT IS A NATIVE INPUT, DELIBERATELY. Radix's Checkbox is not a direct dependency here, and
 * adding one — or reaching through the `radix-ui` umbrella that ADR-C is in the business of
 * removing — buys nothing a native checkbox does not already give: real form participation,
 * the platform's own AT semantics, `:checked` / `:disabled` / `:focus-visible` for free, and
 * no JS to re-implement a toggle. `appearance-none` removes the platform painting; the
 * `peer-checked` sibling draws the mark.
 *
 * `onCheckedChange` is offered alongside `onChange` so call sites read like the rest of the
 * primitive layer; both fire.
 */
export interface CheckboxProps
  extends Omit<React.ComponentPropsWithoutRef<"input">, "type"> {
  onCheckedChange?: (checked: boolean) => void
  /** Class names for the 16×16 box itself. */
  className?: string
  /** Class names for the inline wrapper that positions the mark. */
  wrapperClassName?: string
}

const Checkbox = React.forwardRef<HTMLInputElement, CheckboxProps>(
  ({ className, wrapperClassName, onChange, onCheckedChange, ...props }, ref) => (
    <span
      className={cn(
        "relative inline-flex h-4 w-4 shrink-0 items-center justify-center",
        wrapperClassName
      )}
    >
      <input
        type="checkbox"
        ref={ref}
        onChange={(event) => {
          onChange?.(event)
          onCheckedChange?.(event.target.checked)
        }}
        className={cn(
          "peer h-4 w-4 shrink-0 cursor-pointer appearance-none rounded-xs border border-input bg-card m-0",
          "transition-colors duration-instant ease-out",
          "hover:border-input-hover",
          "checked:border-primary checked:bg-primary",
          focusRing("card"),
          "aria-[invalid=true]:border-destructive",
          "disabled:cursor-not-allowed disabled:bg-muted disabled:opacity-50 disabled:hover:border-input",
          className
        )}
        {...props}
      />
      <Check
        aria-hidden="true"
        className="pointer-events-none absolute h-3 w-3 text-primary-foreground opacity-0 peer-checked:opacity-100"
      />
    </span>
  )
)
Checkbox.displayName = "Checkbox"

export { Checkbox }
