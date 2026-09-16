# CONVENTIONS — Coding standards (so agent output is consistent & reviewable)

## Rust (Tauri core + server if Rust)
- Errors: `anyhow::Result` at app boundaries; define typed errors (`thiserror`) for domain/authz (e.g. `AuthzError`, `SyncError`). Never `unwrap()`/`expect()` on fallible I/O in shipped paths.
- Logging: `tracing` with module targets and structured fields (`tenant_id`, `request_id` where available). **Never log secrets, transcripts, or PII.**
- Async: keep audio real-time paths off the async DB executor; document lock ordering; no blocking calls in async without `spawn_blocking`.
- Format/lint gate: `cargo fmt` + `cargo clippy --all-targets -D warnings` (no new warnings).
- Tauri commands: `#[tauri::command]`, typed args/returns via serde, serializable error type; register in `lib.rs`.

## TypeScript / Next.js
- Strict TS; `pnpm tsc --noEmit` clean; `pnpm run lint` clean.
- All backend access through `src/services/*` typed wrappers over `invoke()` — no raw `invoke` in components.
- User-facing errors are friendly; never surface raw Rust panics. Loading/streaming states for long ops.
- Canonical editor = BlockNote. Do not add TipTap/Remirror usage.

## SQL / migrations
- Forward-only, idempotent, monotonically named; never edit an applied migration.
- Every domain table: `id uuid`, `tenant_id`/`workspace_id`, `created_at`, `updated_at` (+ `updated_by`, `rev`, `deleted_at` if synced).
- Server tables ship their RLS policy in the same migration.
- Synced-table changes are additive; renames/drops are two-step (deprecate→migrate→drop) with a sync-compatibility note.

## Testing
- Rust: unit tests for non-trivial logic; integration tests for repositories (prove tenant scoping).
- Server: **mandatory** `cross_tenant_isolation_test` proving tenant A cannot read/modify tenant B (via API and against RLS).
- Frontend: component tests for review/consent/export; an e2e for record→approve→export where feasible.
- A bug fix ships with a regression test.

### Seeing a UI change, not just compiling it
A UI change is not done until it has been looked at. `tsc`, lint and a mockup all
pass on a screen that renders nothing.

```bash
python tools/ui/shoot.py --build design/report --expect "Action items"
```

It builds the static export, serves it over HTTP, screenshots each route and
**fails** if the PNG is suspiciously small or the rendered DOM is missing the
text you named. Two traps it exists to close, both of which produce a valid PNG
of nothing:
- Opening the export over `file://` gives an unstyled shell — the assets are
  referenced by absolute path, so `/` resolves to the filesystem root.
- Pages that call `invoke()` cannot render outside the Tauri runtime; they sit
  on a loading state forever. Screenshot the `/design/*` route with fixture data
  instead, which is what those routes are for.

Shots land in `target/ui-shots/` (ignored). This does not replace running the
real app — it catches "renders nothing" early, not "wrong in the app".

A route may carry a query string — `design/hitl?reject=1`, `design/tour?tour=3` — which the
tool splits off before appending `.html` and strips out of the PNG filename. `--expect` also
accepts a marker that begins with `-` (`--expect "--ai-surface"`), because half the markers
DESIGN_SYSTEM.md §11.2 requires *are* token names.

## Styling: semantic tokens, and where the lint cannot see

Colour, type, radius, elevation, motion and layering all come from the tokens in
`docs/DESIGN_SYSTEM.md` §4, declared in `frontend/src/app/globals.css` (on **both** `:root`
and `.dark` — a token declared in only one theme renders transparent in the other) and
mapped to utilities in `frontend/tailwind.config.js`. The working list of what actually
compiles is `frontend/src/app/design/tokens-reference.md`, rendered at `/design`.

Four ESLint guardrails in `frontend/.eslintrc.json` enforce the floor: no raw palette
utility or hex in `className`, no arbitrary `text-[Npx]`, no `ring-ring` without a
`ring-offset-*` sibling, no `outline-none` without a `focus-visible:` replacement.

**Their blind spot, stated plainly so nobody mistakes them for proof.** They are
`no-restricted-syntax` esquery selectors over the `className` attribute's **literal** value,
so they can only read a string literal. `frontend/src/` holds ~2342 literal
`className="…"` sites — but also ~85 `className={cn(…)}` and ~95 template-literal
`className` sites: about **180 compositions the rules cannot read at all**, and the
`ui/` primitives, where 13 files use `outline-none`, are exactly where `cn()` is densest.

Three consequences:

1. The guardrails are a cheap first pass over roughly 93 % of call sites, **not** a proof.
2. The `ring-ring` rule does **not** carry WCAG SC 2.4.11. What carries it is the arithmetic
   in §4.4.6 (the ring against every offset surface, worst case 5.32:1) and the
   `ringOffsetColor` map in `tailwind.config.js` that makes `ring-offset-card` compile at
   all. The rule only stops the most common way of forgetting the offset.
3. The compensating control is the **dead-class / built-CSS check** (§11.4 guardrail 6): it
   greps every emitted `className` token against the built stylesheet, so it sees classes
   however they were composed — including through `cn()` — and it is what catches a
   `ring-offset-card` that never compiled. Prefer it over trusting the selectors.

Compose focus styling from the shared exported constant rather than hand-writing it at a
`cn()` site, so the string those 180 compositions carry is one that was reviewed once. That
constant is **`frontend/src/components/ui/focus-ring.ts`**: `focusRing(surface)` for a
control, `focusRingWithin(surface)` for a composed field whose ring belongs to the group,
and `HIGHLIGHT_ITEM` for a Radix roving-focus menu item, where the `--accent` highlight is
the replacement for the stripped outline and a ring would double-draw. `surface` is chosen
mechanically from DESIGN_SYSTEM.md §5's table by **what the control sits on** — a filled
control takes its *parent* surface, never its own fill, because Tailwind draws the offset
outside the border box.

**`cn()` has to be taught every custom key you add to an existing Tailwind class group.**
`frontend/src/lib/utils.ts` registers the §4.5 type steps, `shadow-elev-*`, `duration-*` and
`max-w-measure` with `extendTailwindMerge`. Without that, `tailwind-merge` classifies
`text-label` as a *colour* (anything after `text-` that is not a t-shirt size is), and
`cn('text-label', 'text-muted-foreground')` silently returns only the colour — the type step
disappears with no error. A token added to `tailwind.config.js` without a matching entry
there works everywhere except inside `cn()`, which is the hardest place to notice it.

The last `overrides` entry in `.eslintrc.json` is a **shrinking quarantine** of files that
still carry pre-redesign classes. Delete your files from it when you migrate them; never
add one.

## Git / PR
- Branches: `feat/<slug>`, `fix/<slug>`, `chore/<slug>`, `refactor/<slug>`.
- Conventional commits: `feat: …`, `fix: …`, `refactor: …`, `docs: …`, `test: …`, `chore: …`.
- PR checklist = CLAUDE.md Definition of Done + `/tenant-check` (server) + `/security-review` (before release). CI must be green.
- One concern per PR. Never mix an audio-pipeline change with a schema change.

## Secrets & config
- LLM keys: OS keychain / Tauri secure store only. Never in SQLite plaintext, source, logs, analytics, or git.
- No hardcoded paths (use Tauri path APIs) or hardcoded ports as required infra.
