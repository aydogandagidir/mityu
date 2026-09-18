import * as React from "react"

import { cn } from "@/lib/utils"

/**
 * Kbd — the keyboard-shortcut glyph for the §3.4 keyboard map: the shortcuts sheet, the
 * command palette's row hints, a tooltip that mentions its own accelerator.
 *
 * It sits on `--surface-2` with a hairline and `font-mono text-micro`, i.e. it reads as an
 * inert key rather than as a control: `<kbd>` is never focusable and never clickable here.
 * 11px `text-micro` is allowed because a key glyph is a symbol next to a sentence, not a
 * sentence — §4.5 reserves 12px as the floor for anything a person must READ.
 *
 * A combination is written as adjacent Kbds (`⌘` `K`), not as one "⌘K" string, so a screen
 * reader and a line break both treat the keys as separate tokens.
 */
const Kbd = React.forwardRef<HTMLElement, React.HTMLAttributes<HTMLElement>>(
  ({ className, ...props }, ref) => (
    <kbd
      ref={ref}
      className={cn(
        "inline-flex h-5 min-w-5 items-center justify-center rounded-xs border border-border",
        "bg-surface-2 px-1 font-mono text-micro text-muted-foreground",
        className
      )}
      {...props}
    />
  )
)
Kbd.displayName = "Kbd"

/** A group of keys with the right rhythm between them: `<KbdGroup><Kbd>⌘</Kbd><Kbd>K</Kbd>`. */
const KbdGroup = React.forwardRef<HTMLSpanElement, React.HTMLAttributes<HTMLSpanElement>>(
  ({ className, ...props }, ref) => (
    <span
      ref={ref}
      className={cn("inline-flex items-center gap-1 align-middle", className)}
      {...props}
    />
  )
)
KbdGroup.displayName = "KbdGroup"

export { Kbd, KbdGroup }
