import * as React from "react"
import { cva, type VariantProps } from "class-variance-authority"

import { cn } from "@/lib/utils"

/**
 * Alert — SUPERSEDED BY `ui/notice.tsx` (DESIGN_SYSTEM.md §5.17). Do not reach for this in
 * new code; reach for `Notice`, which has the six product tones, the action slot and the
 * structural Art. 50 guard.
 *
 * It stays, restyled onto the tokens, for one reason: six shipped files still import it
 * (`ModelSettingsModal`, `BuiltInModelManager`, `PermissionWarning`,
 * `EncryptionStatusBanner`, `BluetoothPlaybackWarning`, `TranscriptRecovery`), each of them
 * scheduled for its own package. Deleting it here would either break them or force this
 * package to rewrite six screens it has not read — the thing §0 calls a big-bang refactor.
 * Until then they at least inherit the token surface instead of `bg-background` + raw
 * amber/red literals.
 *
 * `variant="destructive"` is for an ERROR. It was abused as "an important box" across the
 * app, which is how the product ended up with red panels that reported nothing wrong; those
 * call sites become `Notice tone="warning"` / `"info"` in their own packages.
 *
 * NOTE ON `role="alert"`: this component is an assertive live region. 🔒 §8 forbids one on a
 * refusal, and it is the wrong container for anything that is merely important.
 */
const alertVariants = cva(
  "relative w-full rounded-md border p-3 text-body [&>svg+div]:translate-y-[-3px] [&>svg]:absolute [&>svg]:left-3 [&>svg]:top-3 [&>svg]:size-4 [&>svg~*]:pl-6",
  {
    variants: {
      variant: {
        default: "border-border bg-card text-card-foreground [&>svg]:text-muted-foreground",
        destructive:
          "border-destructive-border bg-destructive-surface text-destructive-ink [&>svg]:text-destructive-ink",
      },
    },
    defaultVariants: {
      variant: "default",
    },
  }
)

const Alert = React.forwardRef<
  HTMLDivElement,
  React.HTMLAttributes<HTMLDivElement> & VariantProps<typeof alertVariants>
>(({ className, variant, ...props }, ref) => (
  <div
    ref={ref}
    role="alert"
    className={cn(alertVariants({ variant }), className)}
    {...props}
  />
))
Alert.displayName = "Alert"

const AlertTitle = React.forwardRef<
  HTMLParagraphElement,
  React.HTMLAttributes<HTMLHeadingElement>
>(({ className, ...props }, ref) => (
  <h5
    ref={ref}
    className={cn("mb-1 text-body font-medium leading-none tracking-tight", className)}
    {...props}
  />
))
AlertTitle.displayName = "AlertTitle"

const AlertDescription = React.forwardRef<
  HTMLParagraphElement,
  React.HTMLAttributes<HTMLParagraphElement>
>(({ className, ...props }, ref) => (
  <div
    ref={ref}
    className={cn("text-caption [&_p]:leading-relaxed", className)}
    {...props}
  />
))
AlertDescription.displayName = "AlertDescription"

export { Alert, AlertTitle, AlertDescription }
