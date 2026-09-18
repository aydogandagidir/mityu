/**
 * The one focus treatment — DESIGN_SYSTEM.md §5 (shared rules) and §4.4.6.
 *
 * WHY A CONSTANT. The audit counted five competing treatments across `src/`
 * (`focus:ring-blue-500` alone outnumbered `focus-visible:ring-ring`), 16 files that strip
 * the outline with no replacement at all, and `ring-1` on `ui/button.tsx` — 1px, which
 * fails SC 2.4.13's 2px perimeter. ~85 `cn()` sites compose their classes, and a composed
 * string is invisible to the ESLint guardrails (§11.4, and the blind spot ADR-0052 names).
 * So the treatment is exported once, reviewed once, and imported — never retyped.
 *
 * WHAT CARRIES SC 2.4.11. Not `--ring` on its own: against `--primary` it is 1.22:1
 * (1.71 in dark). It is the 2px OFFSET, drawn in the colour of the surface the control sits
 * on, whose worst case across the twelve distinct `ringOffsetColor` values is 5.32:1
 * (§4.4.6). Tailwind paints the offset OUTSIDE the border box, so it must be the surface
 * UNDER the control, never the control's own fill — a fill-coloured offset paints a halo
 * instead of a gap. A filled control (`default`, `verified`, `destructive`, `record`)
 * therefore takes its PARENT surface's offset.
 *
 * CHOOSING `surface` is mechanical (§5's table), not a judgement call:
 *   card             inside a Card, a list row, a Section card, the page header, a toolbar
 *   background       on the app ground: the rail, the Meetings pane, a page body, an EmptyState,
 *                    and a floating panel (popover / dialog / sheet), which sits on the page
 *   popover          a control INSIDE a popover, dropdown, select, tooltip, dialog, sheet,
 *                    command palette or toast
 *   sidebar          inside the rail, where --sidebar may diverge from --background
 *   surface-2        inside a Well, a segmented control, a progress/meter track, a skeleton
 *   surface-3        inside a pressed or selected well
 *   accent           inside a selected / aria-current row
 *   {tone}-surface   inside a Notice or a status pill (verified/success/warning/destructive/info/ai)
 *   recording-surface inside the session dock's recording strip
 *
 * Every class below is written out in full because Tailwind extracts class names from
 * source TEXT: a template-built `focus-visible:ring-offset-${surface}` emits no CSS.
 */

export type FocusSurface =
  | 'background'
  | 'card'
  | 'popover'
  | 'sidebar'
  | 'muted'
  | 'surface-2'
  | 'surface-3'
  | 'accent'
  | 'verified-surface'
  | 'success-surface'
  | 'warning-surface'
  | 'destructive-surface'
  | 'info-surface'
  | 'ai-surface'
  | 'recording-surface';

/**
 * `focus-visible`, never `focus`: `focus:` fires on mouse click too (103 sites shipped that
 * mistake). `outline-none` is only ever written together with the ring that replaces it.
 */
const RING =
  'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2';

const RING_OFFSET: Record<FocusSurface, string> = {
  background: 'focus-visible:ring-offset-background',
  card: 'focus-visible:ring-offset-card',
  popover: 'focus-visible:ring-offset-popover',
  sidebar: 'focus-visible:ring-offset-sidebar',
  muted: 'focus-visible:ring-offset-muted',
  'surface-2': 'focus-visible:ring-offset-surface-2',
  'surface-3': 'focus-visible:ring-offset-surface-3',
  accent: 'focus-visible:ring-offset-accent',
  'verified-surface': 'focus-visible:ring-offset-verified-surface',
  'success-surface': 'focus-visible:ring-offset-success-surface',
  'warning-surface': 'focus-visible:ring-offset-warning-surface',
  'destructive-surface': 'focus-visible:ring-offset-destructive-surface',
  'info-surface': 'focus-visible:ring-offset-info-surface',
  'ai-surface': 'focus-visible:ring-offset-ai-surface',
  'recording-surface': 'focus-visible:ring-offset-recording-surface',
};

/** The shared treatment for a focusable control. `card` is the commonest ground. */
export function focusRing(surface: FocusSurface = 'card'): string {
  return `${RING} ${RING_OFFSET[surface]}`;
}

/**
 * The same ring, drawn on the WRAPPER of a composed field (`InputGroup`), which reads as one
 * control even though it is a `<div>` around an `<input>` and some adornments.
 *
 * Two deliberate narrowings. `has-[…]`, not `focus-within`: `focus-within` fires on a mouse
 * click, the same defect as `focus:`. And it keys on the CONTROL specifically
 * (`[data-slot=input-group-control]`), not on any focusable descendant — a clear or reveal
 * BUTTON inside the group draws its own ring, and an unscoped `has-[:focus-visible]` would
 * light the group up at the same time, nesting two rings and telling the user nothing about
 * which of the two controls actually has focus.
 *
 * The surfaces are the ones a composed field can sit on; widen the union if a real one appears.
 */
export type FieldGroupSurface = Extract<
  FocusSurface,
  'background' | 'card' | 'popover' | 'surface-2'
>;

const RING_FIELD =
  'has-[[data-slot=input-group-control]:focus-visible]:outline-none has-[[data-slot=input-group-control]:focus-visible]:ring-2 has-[[data-slot=input-group-control]:focus-visible]:ring-ring has-[[data-slot=input-group-control]:focus-visible]:ring-offset-2';

const RING_FIELD_OFFSET: Record<FieldGroupSurface, string> = {
  background:
    'has-[[data-slot=input-group-control]:focus-visible]:ring-offset-background',
  card: 'has-[[data-slot=input-group-control]:focus-visible]:ring-offset-card',
  popover:
    'has-[[data-slot=input-group-control]:focus-visible]:ring-offset-popover',
  'surface-2':
    'has-[[data-slot=input-group-control]:focus-visible]:ring-offset-surface-2',
};

export function focusRingWithin(surface: FieldGroupSurface = 'card'): string {
  return `${RING_FIELD} ${RING_FIELD_OFFSET[surface]}`;
}

/**
 * Radix roving-focus items (menu / select / command) are the one place a ring is WRONG:
 * exactly one item is focusable at a time and the highlight already tracks it, so a ring
 * would double-draw on every arrow press. The highlight IS the replacement for the stripped
 * outline — which is why `outline-none` never appears in those files without this constant
 * next to it.
 */
export const HIGHLIGHT_ITEM =
  'outline-none focus:bg-accent focus:text-accent-foreground data-[highlighted]:bg-accent data-[highlighted]:text-accent-foreground';
