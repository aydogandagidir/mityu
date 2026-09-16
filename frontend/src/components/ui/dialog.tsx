"use client"

import * as React from "react"
import * as DialogPrimitive from "@radix-ui/react-dialog"
import { cva, type VariantProps } from "class-variance-authority"
import { X } from "lucide-react"

import { cn } from "@/lib/utils"
import { focusRing } from "@/components/ui/focus-ring"

/**
 * Dialog — DESIGN_SYSTEM.md §5.14.
 *
 * 🔒 THE CLOSE BUTTON KEEPS `<span class="sr-only">Close</span>` AND CARRIES NO
 * `aria-label`. This is load-bearing, not styling: `AskThisMeeting/AskPanel.test.tsx`
 * asserts that the panel's container holds NO descendant whose `aria-label` contains
 * "close" or "dismiss", because the Art. 50 disclosure must be non-dismissable. Adding an
 * `aria-label="Close"` here — the "obvious" a11y improvement — fails that suite the moment
 * anything like `AskPanel` is rendered inside a Dialog. The sr-only span is already a
 * complete accessible name.
 *
 * The overlay is `hsl(var(--overlay)/0.55)` (0.65 in dark), replacing `bg-black/80` and the
 * three hand-rolled `bg-black bg-opacity-50` scrims. z-60 overlay / z-70 content (§4.10).
 */
const Dialog = DialogPrimitive.Root

const DialogTrigger = DialogPrimitive.Trigger

const DialogPortal = DialogPrimitive.Portal

const DialogClose = DialogPrimitive.Close

const DialogOverlay = React.forwardRef<
  React.ElementRef<typeof DialogPrimitive.Overlay>,
  React.ComponentPropsWithoutRef<typeof DialogPrimitive.Overlay>
>(({ className, ...props }, ref) => (
  <DialogPrimitive.Overlay
    ref={ref}
    className={cn(
      "fixed inset-0 z-60 bg-[hsl(var(--overlay)/0.55)] dark:bg-[hsl(var(--overlay)/0.65)]",
      "data-[state=open]:animate-in data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0",
      className
    )}
    {...props}
  />
))
DialogOverlay.displayName = DialogPrimitive.Overlay.displayName

/** §5.14 widths: sm 420 · md 480 · lg 560 · xl 720. */
const dialogContentVariants = cva(
  cn(
    "fixed left-1/2 top-1/2 z-70 grid w-full translate-x-[-50%] translate-y-[-50%] gap-4",
    "rounded-xl border border-border bg-popover p-6 text-popover-foreground shadow-elev-2",
    "dark:border-border-strong",
    "duration-base data-[state=open]:animate-in data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0 data-[state=closed]:zoom-out-95 data-[state=open]:zoom-in-95"
  ),
  {
    variants: {
      size: {
        sm: "max-w-[420px]",
        md: "max-w-[480px]",
        lg: "max-w-[560px]",
        xl: "max-w-[720px]",
      },
    },
    defaultVariants: { size: "md" },
  }
)

const DialogContent = React.forwardRef<
  React.ElementRef<typeof DialogPrimitive.Content>,
  React.ComponentPropsWithoutRef<typeof DialogPrimitive.Content> &
    VariantProps<typeof dialogContentVariants>
>(({ className, children, size, ...props }, ref) => (
  <DialogPortal>
    <DialogOverlay />
    <DialogPrimitive.Content
      ref={ref}
      className={cn(dialogContentVariants({ size }), className)}
      {...props}
    >
      {children}
      <DialogPrimitive.Close
        className={cn(
          "absolute right-4 top-4 inline-flex h-8 w-8 items-center justify-center rounded-sm text-muted-foreground",
          "transition-colors duration-instant ease-out hover:bg-muted hover:text-foreground active:bg-surface-3",
          focusRing("popover"),
          "disabled:pointer-events-none"
        )}
      >
        <X className="h-4 w-4" />
        {/* 🔒 sr-only text, never aria-label — see the file header. */}
        <span className="sr-only">Close</span>
      </DialogPrimitive.Close>
    </DialogPrimitive.Content>
  </DialogPortal>
))
DialogContent.displayName = DialogPrimitive.Content.displayName

const DialogHeader = ({
  className,
  ...props
}: React.HTMLAttributes<HTMLDivElement>) => (
  <div
    className={cn("flex flex-col space-y-1.5 pr-8 text-left", className)}
    {...props}
  />
)
DialogHeader.displayName = "DialogHeader"

const DialogFooter = ({
  className,
  ...props
}: React.HTMLAttributes<HTMLDivElement>) => (
  <div
    className={cn(
      "flex flex-col-reverse gap-2 sm:flex-row sm:justify-end",
      className
    )}
    {...props}
  />
)
DialogFooter.displayName = "DialogFooter"

const DialogTitle = React.forwardRef<
  React.ElementRef<typeof DialogPrimitive.Title>,
  React.ComponentPropsWithoutRef<typeof DialogPrimitive.Title>
>(({ className, ...props }, ref) => (
  <DialogPrimitive.Title
    ref={ref}
    className={cn("text-title-lg text-foreground", className)}
    {...props}
  />
))
DialogTitle.displayName = DialogPrimitive.Title.displayName

const DialogDescription = React.forwardRef<
  React.ElementRef<typeof DialogPrimitive.Description>,
  React.ComponentPropsWithoutRef<typeof DialogPrimitive.Description>
>(({ className, ...props }, ref) => (
  <DialogPrimitive.Description
    ref={ref}
    className={cn("text-body text-muted-foreground", className)}
    {...props}
  />
))
DialogDescription.displayName = DialogPrimitive.Description.displayName

export {
  Dialog,
  DialogPortal,
  DialogOverlay,
  DialogTrigger,
  DialogClose,
  DialogContent,
  DialogHeader,
  DialogFooter,
  DialogTitle,
  DialogDescription,
  dialogContentVariants,
}
