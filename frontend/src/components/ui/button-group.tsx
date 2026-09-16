import * as React from "react"
import { Slot } from "@radix-ui/react-slot"
import { cva, type VariantProps } from "class-variance-authority"

import { cn } from "@/lib/utils"
import { Separator } from "@/components/ui/separator"

/**
 * ButtonGroup — rewritten onto the v3 `forwardRef` generation (ADR-C), so a consumer can
 * take a ref and so every primitive in `ui/` reads the same way.
 *
 * `shadow-xs` is gone from `ButtonGroupText`: it is a Tailwind v4 class name and compiles to
 * NOTHING on Tailwind 3.4 — the exact phantom-class failure ADR-0037 caught (`text-medium`,
 * `text-s`). It looked like an elevation decision and was a no-op. §4.8 is hairline-first
 * anyway: a toolbar sits at `--elev-0`.
 */
const buttonGroupVariants = cva(
  "flex w-fit items-stretch [&>*]:focus-visible:relative [&>*]:focus-visible:z-10 [&>input]:flex-1",
  {
    variants: {
      orientation: {
        horizontal:
          "[&>*:not(:first-child)]:rounded-l-none [&>*:not(:first-child)]:border-l-0 [&>*:not(:last-child)]:rounded-r-none",
        vertical:
          "flex-col [&>*:not(:first-child)]:rounded-t-none [&>*:not(:first-child)]:border-t-0 [&>*:not(:last-child)]:rounded-b-none",
      },
    },
    defaultVariants: {
      orientation: "horizontal",
    },
  }
)

const ButtonGroup = React.forwardRef<
  HTMLDivElement,
  React.ComponentPropsWithoutRef<"div"> &
    VariantProps<typeof buttonGroupVariants>
>(({ className, orientation, ...props }, ref) => (
  <div
    ref={ref}
    role="group"
    data-slot="button-group"
    data-orientation={orientation ?? "horizontal"}
    className={cn(buttonGroupVariants({ orientation }), className)}
    {...props}
  />
))
ButtonGroup.displayName = "ButtonGroup"

const ButtonGroupText = React.forwardRef<
  HTMLDivElement,
  React.ComponentPropsWithoutRef<"div"> & { asChild?: boolean }
>(({ className, asChild = false, ...props }, ref) => {
  const Comp = asChild ? Slot : "div"

  return (
    <Comp
      ref={ref}
      className={cn(
        "flex items-center gap-2 rounded-md border border-input bg-muted px-4 text-label text-muted-foreground",
        "[&_svg]:pointer-events-none [&_svg:not([class*='size-'])]:size-4",
        className
      )}
      {...props}
    />
  )
})
ButtonGroupText.displayName = "ButtonGroupText"

const ButtonGroupSeparator = React.forwardRef<
  React.ElementRef<typeof Separator>,
  React.ComponentPropsWithoutRef<typeof Separator>
>(({ className, orientation = "vertical", ...props }, ref) => (
  <Separator
    ref={ref}
    data-slot="button-group-separator"
    orientation={orientation}
    className={cn(
      "relative !m-0 self-stretch bg-input data-[orientation=vertical]:h-auto",
      className
    )}
    {...props}
  />
))
ButtonGroupSeparator.displayName = "ButtonGroupSeparator"

export {
  ButtonGroup,
  ButtonGroupSeparator,
  ButtonGroupText,
  buttonGroupVariants,
}
