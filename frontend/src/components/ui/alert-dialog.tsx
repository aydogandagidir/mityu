"use client"

import * as React from "react"
import * as DialogPrimitive from "@radix-ui/react-dialog"

import { cn } from "@/lib/utils"
import { Button, type ButtonProps } from "@/components/ui/button"
import { dialogContentVariants } from "@/components/ui/dialog"

/**
 * AlertDialog — DESIGN_SYSTEM.md §5.14: the confirmation surface for destructive actions
 * (delete meeting, discard recovery, deactivate licence), replacing the hand-rolled
 * `bg-black bg-opacity-50` modals in `confirmation-modal.tsx`, `SettingsModal.tsx` (×6) and
 * `AnalyticsDataModal.tsx`.
 *
 * BUILT ON `@radix-ui/react-dialog`, WHICH IS A DIRECT DEPENDENCY. Radix's own AlertDialog
 * is only reachable here through the `radix-ui` umbrella, and ADR-C removes that second
 * import path rather than adding call sites to it; a new npm dependency is out of scope for
 * this package. The three things AlertDialog adds over Dialog are all props:
 *   • `role="alertdialog"` — Radix spreads `...contentProps` AFTER its own `role: "dialog"`,
 *     so the override is real, and the content keeps Radix's `aria-labelledby` /
 *     `aria-describedby` wiring to Title and Description.
 *   • it does not close on an outside click or on a pointer-down outside — a destructive
 *     confirmation is dismissed by a decision, not by a stray click. Escape still works,
 *     and `Cancel` is always present, so SC 2.1.2 is satisfied.
 *   • focus opens on CANCEL, not on the destructive action.
 * There is no auto-injected close X: the two buttons ARE the exits.
 */
const AlertDialog = DialogPrimitive.Root

const AlertDialogTrigger = DialogPrimitive.Trigger

const AlertDialogPortal = DialogPrimitive.Portal

const AlertDialogOverlay = React.forwardRef<
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
AlertDialogOverlay.displayName = "AlertDialogOverlay"

const CANCEL_ATTR = "data-alert-dialog-cancel"

const AlertDialogContent = React.forwardRef<
  React.ElementRef<typeof DialogPrimitive.Content>,
  React.ComponentPropsWithoutRef<typeof DialogPrimitive.Content>
>(({ className, onOpenAutoFocus, onPointerDownOutside, onInteractOutside, ...props }, ref) => (
  <AlertDialogPortal>
    <AlertDialogOverlay />
    <DialogPrimitive.Content
      ref={ref}
      role="alertdialog"
      className={cn(dialogContentVariants({ size: "sm" }), className)}
      onOpenAutoFocus={(event) => {
        onOpenAutoFocus?.(event)
        if (event.defaultPrevented) return
        const cancel = event.currentTarget instanceof HTMLElement
          ? event.currentTarget.querySelector<HTMLElement>(`[${CANCEL_ATTR}]`)
          : null
        if (cancel) {
          event.preventDefault()
          cancel.focus()
        }
      }}
      onPointerDownOutside={(event) => {
        onPointerDownOutside?.(event)
        event.preventDefault()
      }}
      onInteractOutside={(event) => {
        onInteractOutside?.(event)
        event.preventDefault()
      }}
      {...props}
    />
  </AlertDialogPortal>
))
AlertDialogContent.displayName = "AlertDialogContent"

const AlertDialogHeader = ({
  className,
  ...props
}: React.HTMLAttributes<HTMLDivElement>) => (
  <div className={cn("flex flex-col space-y-1.5 text-left", className)} {...props} />
)
AlertDialogHeader.displayName = "AlertDialogHeader"

const AlertDialogFooter = ({
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
AlertDialogFooter.displayName = "AlertDialogFooter"

const AlertDialogTitle = React.forwardRef<
  React.ElementRef<typeof DialogPrimitive.Title>,
  React.ComponentPropsWithoutRef<typeof DialogPrimitive.Title>
>(({ className, ...props }, ref) => (
  <DialogPrimitive.Title
    ref={ref}
    className={cn("text-title-lg text-foreground", className)}
    {...props}
  />
))
AlertDialogTitle.displayName = "AlertDialogTitle"

const AlertDialogDescription = React.forwardRef<
  React.ElementRef<typeof DialogPrimitive.Description>,
  React.ComponentPropsWithoutRef<typeof DialogPrimitive.Description>
>(({ className, ...props }, ref) => (
  <DialogPrimitive.Description
    ref={ref}
    className={cn("text-body text-muted-foreground", className)}
    {...props}
  />
))
AlertDialogDescription.displayName = "AlertDialogDescription"

/** The confirming action. `destructive` is the default because that is what this is for. */
const AlertDialogAction = React.forwardRef<HTMLButtonElement, ButtonProps>(
  ({ variant = "destructive", ...props }, ref) => (
    <DialogPrimitive.Close asChild>
      <Button ref={ref} variant={variant} {...props} />
    </DialogPrimitive.Close>
  )
)
AlertDialogAction.displayName = "AlertDialogAction"

const AlertDialogCancel = React.forwardRef<HTMLButtonElement, ButtonProps>(
  ({ variant = "outline", ...props }, ref) => (
    <DialogPrimitive.Close asChild>
      <Button ref={ref} variant={variant} {...{ [CANCEL_ATTR]: "" }} {...props} />
    </DialogPrimitive.Close>
  )
)
AlertDialogCancel.displayName = "AlertDialogCancel"

export {
  AlertDialog,
  AlertDialogPortal,
  AlertDialogOverlay,
  AlertDialogTrigger,
  AlertDialogContent,
  AlertDialogHeader,
  AlertDialogFooter,
  AlertDialogTitle,
  AlertDialogDescription,
  AlertDialogAction,
  AlertDialogCancel,
}
