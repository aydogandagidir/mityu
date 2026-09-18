# Token reference — what WP2–WP16 may type

The authority is `docs/DESIGN_SYSTEM.md` §4; this is the working copy that sits next to the
fixture route, listing **the utilities that actually compile** after WP1. If a class is not
derivable from a row below, it is either a raw palette utility (ESLint guardrail 1) or a
class with no CSS behind it (the ADR-0037 failure class: `tsc`, lint and vitest all pass on
a class that emits nothing).

Render it: `/design`. Every value here is proved by `scratchpad/contrast-recheck.txt`, which
recomputes all of §4.4 from `src/app/globals.css` rather than from the document.

---

## 1. The migration rule

**New values landed under the old token names.** `--background`, `--card`,
`--muted-foreground` and the rest kept their names and changed their values, so the ~871
existing `bg-card` / `text-muted-foreground` call sites improved on the WP1 commit instead
of breaking. Two consequences to hold in mind while the component packages land:

* A file that still uses `bg-gray-50` has *not* been migrated — it renders the old grey next
  to the new surfaces. That is the intended mixed intermediate state; the fix is that file's
  own WP, not a token change.
* Nothing may be added to the ESLint quarantine list in `.eslintrc.json`. It only shrinks.

## 2. Surfaces and text

| Utility | Token | Use |
|---|---|---|
| `bg-background` | `--background` | app ground, rail, Meetings pane |
| `bg-card` | `--card` | content pane, cards, rows — the default surface |
| `bg-popover` | `--popover` | dropdown, select, tooltip |
| `bg-surface-2` | `--surface-2` | sunken well, code, track |
| `bg-surface-3` | `--surface-3` | pressed / selected well |
| `bg-muted` | `--muted` | row hover |
| `bg-accent` | `--accent` | active nav row, selected list row |
| `text-foreground` | `--foreground` | ink; AAA in both themes |
| `text-muted-foreground` | `--muted-foreground` | secondary text |
| `text-subtle-foreground` | `--subtle-foreground` | meta, timestamps |
| `text-primary-ink` | `--primary-ink` | brand **text** and icons — *not* `text-primary` |

`--primary` is a **fill**. Brand-coloured text uses `--primary-ink`, which is a hair darker
(light) / lighter (dark) so it clears 4.5:1 on `--surface-3` too.

## 3. Interaction states

Every filled control has a specified hover and pressed fill, and **there is no global
"hover = darker" rule** — inventing one is how the dark `--primary-hover` ended up at 3.82
under `Approve summary`.

| Family | idle | hover | active | label |
|---|---|---|---|---|
| ink (`default` button) | `bg-foreground` | `bg-foreground-hover` | `bg-foreground-active` | `text-background` |
| brand / verified | `bg-primary` · `bg-verified` | `bg-primary-hover` · `bg-verified-hover` | `bg-primary-active` · `bg-verified-active` | `text-primary-foreground` |
| destructive | `bg-destructive` | `bg-destructive-hover` | `bg-destructive-active` | `text-destructive-foreground` |
| record / stop | `bg-recording` | `bg-recording-hover` | `bg-recording-active` | **`text-recording-foreground`** |

`text-recording-foreground` is not optional: white on the dark `--recording` (#EA473E) is
3.84 and fails AA, so the dark label is a near-black red.

## 4. Registers (tint + ink + hairline)

`bg-x-surface` · `text-x-ink` · `border-x-border`, for `x` in **verified · success ·
warning · destructive · info · ai · recording** (recording borrows `border-destructive-border`).
Solid fills are `bg-x` with `text-x-foreground`.

The AI register is **violet** (256°) and warning is **amber** (36–38°): 220° apart, so
"this is a machine draft" can never be read as "something is wrong". Never paint a draft amber.

## 5. Boundaries and focus

| Utility | Token | Rule |
|---|---|---|
| `border-border` | `--border` | decorative hairline between two filled surfaces |
| `border-border-strong` | `--border-strong` | emphasised divider — **never a control's only boundary** |
| `border-input` | `--input` | the boundary that *identifies* a control (1.4.11) |
| `border-input-hover` | `--input-hover` | the same boundary on hover |
| `ring-ring` | `--ring` | focus, decoupled from `--primary` |

**The focus treatment is `focus-visible:ring-2 focus-visible:ring-ring
focus-visible:ring-offset-2 focus-visible:ring-offset-<surface>`.** The ring against the
fill it surrounds is 1.22:1; what carries SC 2.4.11 is the 2px offset in the colour of the
surface the control sits **on** — worst case 5.32:1. `ring-offset-*` accepts:
`background · card · popover · sidebar · muted · surface-2 · surface-3 · accent ·
verified-surface · success-surface · warning-surface · destructive-surface · info-surface ·
ai-surface · recording-surface`. Offsets are **surfaces, never fills** — a fill-coloured
offset paints a halo instead of a gap.

## 6. Type scale

`text-display` 24/32 · `text-title-lg` 20/28 · `text-title` 16/24 · `text-title-sm` 14/20 ·
`text-read` 15/24 · `text-body` 14/22 · `text-label` 13/18 · `text-caption` 12/16 ·
`text-micro` 11/16 · `text-eyebrow` 11/16 (pair with `uppercase`) · `font-mono` for anything
comparable. Weight and tracking travel with the step — do not re-declare them.

**12px (`text-caption`) is the floor for anything a person must read**, including every
consent, disclosure and compliance string. `text-micro` is an eyebrow, a count or an `mm:ss`
— never a sentence outside the 380px copilot window, and never compliance copy.

## 7. Geometry, elevation, motion, layering

* Radius `rounded-xs|sm|md|lg|xl|2xl` = 4/6/8/10/12/16px off `--radius`. `sm`/`md`/`lg` are
  byte-identical to today's 6/8/10, so existing corners move zero pixels.
* Elevation `shadow-elev-0|1|2|3`. **No component may use a raw Tailwind `shadow-*`.**
  Elevation is a last resort; a surface step is the default. In dark each level also steps
  the surface and adds a `--border-strong` hairline — that part is the component's job.
* Motion `duration-instant|fast|base|slow` with `ease-out` / `ease-in-out` / `ease-emphasis`.
  The global `prefers-reduced-motion` block in `globals.css` neutralises CSS animation and
  transition; framer-motion needs `useReducedMotion()` in each file that uses it, and the
  fallback must stay **semantic** (the recording dot goes solid while the text carries the
  state; it does not merely speed up).
* Layering `z-10` header · `z-30` rail + pane · `z-40` dock · `z-50` popover · `z-60/70`
  dialog · `z-80` toasts · `z-95/96` tour · `z-100` onboarding. Do not hand-write `z-[95]`.
* Shell metrics `--rail-w` 56 · `--pane-w` 280 · `--header-h` 56 · `--dock-h` 40 ·
  `--reviewbar-h` 48 · `--bottom-chrome` (published by the shell) · `--measure` 720.

## 8. Charts

`bg-chart-1` … `bg-chart-6` are **fills and strokes only**. Speaker names and talk-time
labels render in `--foreground` / `--muted-foreground`; no chart token ever carries small
text, and series are distinguished by order and label as well as by colour.
