import * as React from "react"
import { type LucideIcon } from "lucide-react"

import { cn } from "@/lib/utils"

/**
 * EmptyState — DESIGN_SYSTEM.md §5.12. Centred, max 420px: a 40×40 tile with a 20px lucide
 * icon (NEVER an emoji — an emoji renders differently on every platform and carries a tone
 * this product does not have), `text-title` headline, `text-body` sentence, ONE primary
 * action and at most one link.
 *
 * EVERY EMPTY STATE SHIPS A NEXT ACTION. An empty screen that only says "nothing here" is a
 * dead end; 🔒 `/actions` is exactly that today. The action is passed in as a node so this
 * component never learns about routing or services.
 *
 * THREE VARIANTS, which differ in what the tile means:
 *   `first-run`  nothing has happened yet        → neutral tile
 *   `filtered`   a query or filter excluded everything → neutral tile, and the action is
 *                normally "Clear filters"
 *   `blocked`    the user CANNOT proceed (no model, no permission) → warning tile, because
 *                this one is a thing that is wrong (§5.8) and needs to read differently
 *                from an ordinary blank slate.
 */
export type EmptyStateVariant = "first-run" | "filtered" | "blocked"

const TILE: Record<EmptyStateVariant, string> = {
  "first-run": "bg-surface-2 text-muted-foreground",
  filtered: "bg-surface-2 text-muted-foreground",
  blocked: "bg-warning-surface text-warning-ink",
}

export interface EmptyStateProps {
  icon: LucideIcon
  title: string
  description?: React.ReactNode
  /** Exactly one primary action. */
  action?: React.ReactNode
  /** At most one link (a `Button variant="link"` or an anchor). */
  link?: React.ReactNode
  variant?: EmptyStateVariant
  className?: string
}

export function EmptyState({
  icon: Icon,
  title,
  description,
  action,
  link,
  variant = "first-run",
  className,
}: EmptyStateProps) {
  return (
    <div
      className={cn(
        "mx-auto flex max-w-[420px] flex-col items-center gap-3 px-4 py-10 text-center",
        className
      )}
    >
      <span className={cn("grid size-10 shrink-0 place-items-center rounded-md", TILE[variant])}>
        <Icon className="size-5" aria-hidden />
      </span>
      <div className="space-y-1">
        <h3 className="text-title text-foreground">{title}</h3>
        {description ? (
          <p className="text-body text-muted-foreground">{description}</p>
        ) : null}
      </div>
      {action ? <div className="pt-1">{action}</div> : null}
      {link}
    </div>
  )
}
EmptyState.displayName = "EmptyState"
