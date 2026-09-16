# Mityu — Design System & UI Specification (v1, final)

> **Status:** source of truth for Phase 3 implementation. Supersedes the *visual* direction in
> `docs/DESIGN_READAI.md` (its information architecture and its REJECT guardrails are retained).
> Ground truth for everything this document may not break: `scratchpad/phase1-ui-map.md`
> §3 (state contract), §4 (MUST-PRESERVE register), §4.10 (test-pinned strings/roles),
> §5 (visual inventory), §8 (risks).
>
> **Stack unchanged.** Next.js 15 static export · Tailwind 3.4.19 · shadcn/ui (Radix) · lucide-react ·
> framer-motion · BlockNote · sonner · cmdk — inside Tauri 2, window 1100×700.
> **No new UI dependency is introduced.** `@heroicons/react` is removed; `tailwind-merge` is pinned *down*
> to `^2.6.0` to match Tailwind 3.4.
>
> **Notation.** 🔒 marks a frozen contract (a MUST-PRESERVE string, role, `aria-label`, event name, storage
> key, prop or route) at the line where it is restyled. A 🔒 item may be re-housed and re-coloured; its text
> and semantics may not change without an ADR.

---

## 0. Decision record

### 0.1 What won

Three complete directions were specified and judged by three independent lenses
(constitution & trust · UX quality and IA · implementability in this codebase + accessibility).

| Candidate | Trust | UX/IA | Implementability | **Total** | Rank |
|---|---|---|---|---|---|
| **A — Calm enterprise workspace** | 48 | 50.5 | **53** | **151.5** | **1 (base)** |
| B — The Evidence Ledger | **51** | **51** | 46 | 148 | 2 |
| C — Studio Console / Audio-Native | 45 | 43 | 41 | 129 | 3 |

**A is the base.** It won on the axis that decides whether a redesign ships at all: it is the only
direction whose token arithmetic survived independent recomputation without a single error, whose radius
remap moves ~290 existing corners by zero pixels, and whose migration rule ("new token *values* under the
old token *names*") turns an 871-utility intermediate state into a partial upgrade rather than a
regression. It also had the best first-run/empty-state work and the only stop→report continuity story.

B beat it on the two things a compliance-facing product is actually sold on — review legibility and
evidence navigation — and C owned capture instrumentation. Both are grafted in below.

### 0.2 What was grafted in

| From | Graft | Where it lands |
|---|---|---|
| **B** | Brand blue means *a human decided*; the `default` button is **ink**, not blue | §2 P1, §4.2, §5.4 |
| **B** | **Approve** is always visible and **labelled**; Edit/Reject reveal but stay in the DOM and tab order | §5.10 |
| **B** | 12px floor for every compliance string; 11px is uppercase eyebrow / copilot-only | §4.5 |
| **B** | Review bar sticky at the **bottom** of the report column, with `role="progressbar"` | §5.10, §6.3 |
| **B** | In-session **consent receipt** ("Participants informed · 14:32") | §6.2 |
| **B** | Dead-code removal deferred to a late WP, *after* screens are proven | §9 WP15 |
| **B** | Screenshot CI job lands non-blocking, flips to blocking after one green week | §11 |
| **B** | `Meetings` is a **drawer/pane toggle, not a route** (fixes A's routeless rail item) | §3.2 |
| **B** | Skeletons carry `role="status" aria-live="polite"` + an sr-only sentence | §5.19 |
| **B** | Every empty state ships a next action — including `/actions` | §5.12 |
| **C** | `--ring` **decoupled from `--primary` at the token**, plus a mandatory ring offset | §4.2, §4.4 |
| **C** | Playhead as `role="slider"` with `aria-valuetext`, ±5s / ±30s / Home/End | §5.20, §8 |
| **C** | Level meters `aria-hidden` + a sibling `role="status"` announcing **presence changes only** | §5.21, §8 |
| **C** | `@media (pointer: coarse)` promotes every hover-revealed control to always-visible | §8 |
| **C** | Source chip **seeks the playhead** as well as scrolling the transcript | §5.9 |
| **C** | Capture state in the **window title and tray** (`● Mityu — recording`) | §3.5 |
| **C** | Fixed-bar consequences spelled out: page-body bottom inset + sonner `offset` | §4.11, §5.13 |
| **C** | Engine + sample rate named on screen ("48 kHz · Parakeet") as local-first *evidence* | §6.2 |
| **C** | On-disk storage size as local-first proof — in **Settings → Storage**, not as a Home tile | §6.5 |
| **C** | Add an RTL test for `/actions` (nonce guard, id dedupe, backend order) — zero coverage today | §9 WP11 |
| **C** | `useSidebar().isCollapsed` keeps its exact meaning for all 14 consumers | §3.2, §9 WP4 |
| **C** | Inline 🔒 notation marking frozen contracts at the point of use | this document |
| **C** | Dark elevation via an inset top highlight, not blur on near-black | §4.8 |

### 0.3 Hard-constraint violations resolved

1. **`role="note"` duplication (A and B both shipped it; it breaks CI).**
   Both candidates gave the per-card `AiLabel` chip `role="note"` with
   `aria-label="AI-generated content, human review required"` — the *same accessible name* as
   `ReviewRequiredBanner`. `DraftSummaryView.tsx:1231/1258` render `<SectionCard … aiLabel>` inside the same
   tree as `<ReviewRequiredBanner />`, and `DraftSummaryView.transparency.test.tsx:99,105` queries with the
   **singular** `screen.getByRole('note', {name: BANNER.name})`. Two matches throw.
   **Resolution:** `AiLabel` stays a plain `<span>` (as it is today at `report/primitives.tsx:15-22`) with
   **visible** text `AI-generated · review required` at **12px**. `ReviewRequiredBanner` remains the single
   `role="note"` marking per surface. §4.1 of the register is satisfied by visible text, not by a role.

2. **`/notes/[id]` retained (C).** A reachable route rendering fabricated meetings through
   `dangerouslySetInnerHTML`, inside a product whose thesis is a defensible record.
   **Resolution: deleted** (WP15), and the `Sidebar/index.tsx:561-562` routing heuristic is fixed so no id
   can route there.

3. **ADR-0034 caveat behind an info button (C).** The talk-time caveat is mandated disclosure.
   **Resolution:** `TalkTimePanel` stays in the report's **primary column** with its caveat rendered
   adjacent and verbatim. It is not demoted into the evidence drawer (A's flaw) or behind a control.

4. **A's `Meetings` rail item had no route and shifted the content pane 280px.**
   **Resolution:** the list pane is the **Meetings library only**, user-toggled and persisted, never
   route-dependent. Settings owns its own in-page section column. No pane appears or vanishes on navigation.

5. **A deleted `components/TranscriptView.tsx` in WP4, one WP before its consumer migrated — and it is a
   `vi.mock` target of `TranscriptPanel.diarization.test.tsx`.**
   **Resolution:** the deletion moves to WP15 and **must edit that suite's mock in the same commit**. This is
   the only sanctioned edit to a test file in the whole plan.

6. **Global Stop was promised on every route while `useRecordingStop` mounts only on `/`** (A and B both
   dodged this; C hoisted it carelessly). `useRecordingStop.ts:648-657` installs
   🔒 `window.handleRecordingStop` and **deletes it on unmount** — so today a dock on `/settings` has nothing
   to call. **Resolution:** WP6 is a standalone, ADR-gated work package that introduces
   `RecordingSessionProvider` as the **innermost** provider (preserving the 13-provider relative order
   byte-for-byte) and makes the unconditional 2s `router.push` fire **only when the user is on `/`**.
   It ships alone, behind the mailbox suite and the macOS+Windows smoke test.

7. **Blue-on-blue (B): the Approve control painted the same colour as approved content.**
   **Resolution (§2 P1):** filled brand blue = *the commitment not yet made*, at most **one per viewport**.
   Blue as a 2px rule / ink / tinted pill = *a commitment already made*. Per-row Approve is therefore
   `outline` + Check + the word "Approve"; the page-level `Approve summary` is the one filled blue button.

8. **`--ring` byte-identical to `--primary` (today's P0 #1).** Decoupled at the token (§4.2). Note the
   honest limit: `--ring` #0040FF against `--primary` #1F57FF is only **1.22:1**, so the *token change alone
   does not satisfy SC 2.4.11*. What satisfies it is the mandatory `ring-offset-2` in the adjacent surface
   colour (ring vs surface = **6.61:1** light, **6.41:1** dark; worst case across the twelve distinct
   offset surfaces **5.32:1**, §4.4.6). A bare `ring-ring` with no `ring-offset-*`
   is a **lint error** (§11.4) — and because a lint error needs a legal alternative, §4.7 declares the
   `ringOffsetColor` map that makes `ring-offset-card` / `-background` / `-popover` / … compile, and
   §5 gives the mechanical rule for picking one. Without that map the treatment emitted no CSS at all.

9. **Compliance labels at 10–11px.** Raised to **12px** everywhere (`AiLabel`, `AiMarking`, the Ask
   disclosure). 11px (`text-micro`) survives only as an uppercase eyebrow and inside the 380px copilot
   panel — **never** a sentence, never compliance.

### 0.4 What was rejected, and why

| Rejected | From | Why |
|---|---|---|
| **EvidenceConnector** (bezier drawn from a block to its segment) | B | Not buildable as specified: it tracks a target row inside `VirtualizedTranscriptView`, which virtualizes above `VIRTUALIZATION_THRESHOLD = 10` with `measureElement`, so the node is usually unmounted and every offset changes each scroll frame. B specified only the unmounted fallback. Replaced by the cheaper, equally legible pairing of a persistent evidence drawer + an **active** `SourceChip` state + a 2px segment rule (§5.9, §6.3). May return behind a flag once measured. |
| **Permanent 56px transport on every route** | C | 112px of fixed chrome on a 700px window, on `/settings` and `/actions` where it earns nothing. Replaced by a conditional 40px session dock that only exists while capture is active (§3.5). |
| **Report as a 52/48 split pane** | C | Does not resolve audit problem #8; `docs/DESIGN_READAI.md:45-50` prescribes one column. |
| **Timeline deck with a SPEAKERS lane** | C | A lane that ranks talk time by construction is exactly what ADR-0034 forbids. Chapters + action markers survive on the scrubber (§5.20); speakers do not become a timeline. |
| **Ochre `--ai`** | B | `#8C6312` sits on the warning hue; §8.7 requires AI notices to stay distinct from errors. A's violet (256°) is 220° from amber (36°). |
| **Removing the visible word "Source"** | B, and optional in A | §4.1 pins the control as carrying *visible* "Source" text. Final form: 🔒 `Source · 12:04`. |
| **"3 of 7 signed"** | B | "Signed" implies an e-signature the product does not provide. Final copy: `3 of 7 approved`. |
| **"+1h vs prev" / derived deltas** | C | Inferential framing; `docs/DESIGN_READAI.md` REJECT rows. |
| **Home stat tiles fed by data that does not exist** | C | `api_get_meetings` returns `{id,title}`; there is no disk-usage command in §3.5. A's degradation rule applies instead (§6.1). |
| **Hover-expanding rail (56→248px) on top of a 248px drawer** | B | Two mechanisms for one region; accidental reflow. |
| **DOM order inverted against visual order in the Report** | B | WCAG 1.3.2 meaningful sequence. Evidence precedes or follows in DOM exactly as it reads (§8). |
| **`richColors` flip-flop** | B | Resolved once: `richColors` **off**, tone carried by a 3px left bar (§5.13). |

---

## 1. Design thesis

Mityu's job is not to impress; it is to be trusted with a recording of someone's actual working life, and
then to be got out of the way. So the interface behaves like a well-made desk: a quiet neutral ground,
hairline structure instead of drop shadows, and one saturated brand-blue moment per screen reserved for the
single decision a human is being asked to make. Everything the model produced is visibly *provisional* —
marked in its own violet register that is neither "brand action" nor "error", carrying a visible
`Source · 12:04` chip that opens the transcript at that second and moves the playhead there, so approval is
a real decision rather than a rubber stamp. Blue is the audit trail: a blue rule down a block means a
person approved it. Capture is the one loud thing — a red dock with elapsed time, real level meters, the
engine doing the work named on screen, and a Stop that is reachable from every route. The result reads as
infrastructure a compliance officer would sign off on and a field engineer would leave open all day.

---

## 2. Principles

**P1 — One decision in blue; everything else is ink.**
`--primary` (bluedev `#1E56FF`) means *a human decided, or is deciding right now*. It appears **filled** on
exactly one control per viewport — the human-commit action (`Approve summary`,
🔒 `I confirm participants are informed — start recording`) — and as a **2px rule, outline or ink** on
content a person already approved. The neutral primary button (`Generate`, `Continue`, `Save`) is **ink**
(`bg-foreground text-background`).
*So we never:* ship two filled brand buttons in one viewport; paint a per-row Approve control the same
colour as content that is already approved; or use blue as decoration (today's `bg-blue-100` tiles,
`text-blue-700` labels and blue-inverted tooltips all go).

**P2 — Structure by hairline, not by shadow.**
Grouping comes from 1px `--border` and surface steps; elevation is reserved for things that genuinely float.
*So we never:* nest a shadowed card inside a shadowed card (today's recording pill is `shadow-lg` inside
`shadow-lg`), or default a card to `rounded-2xl` + `shadow-sm`.

**P3 — AI output is furniture that announces itself.**
Model output has a dedicated violet register, a status chip, a source chip and — once per surface — a
non-dismissable `role="note"` marking, in **every** state including loading, empty and error.
*So we do:* keep the Art. 50 note in all four `DraftSummaryView` branches and add the missing
`AiLabel` + `SourceChip` + review controls to Home's action items (the audit's highest-severity gap).
*So we never:* colour AI drafts with the warning/amber register (amber is reserved for things that are
*wrong*), render an AI claim without a jump-to-source control, or put a compliance string below 12px.

**P4 — Capture is always visible, always stoppable, and always honest.**
Recording state lives in the shell, in the window title and in the tray.
*So we do:* a 40px session dock with elapsed time, real per-device meters, the engine and sample rate, Pause
and Stop, on every route.
*So we never:* leave a route whose only stop affordance is a pill on another page, animate a level meter
from `Math.random()`, or claim a capability the platform does not have (no BlackHole, no silent Linux).

**P5 — Every surface is keyboard-complete before it is pretty.**
*So we do:* a `⌘K` palette over the existing `cmdk` dependency, one visible 2px focus treatment with an
offset, row actions reachable by Tab, `?` for a shortcut sheet, and
`@media (pointer: coarse)` promoting every hover-revealed control to always-visible.
*So we never:* hide an action behind `group-hover:opacity-100` only; convey state through `title=` alone;
or disable a control without an inline reason (a disabled button is not focusable, so its tooltip never
opens for a keyboard or AT user — it needs a sibling sentence or `aria-describedby`).

**P6 — Say what is true, in the user's words, once.**
One participant-notice concept, one notifications setting, one recordings-folder card, one model-settings
surface, one word for the object ("meeting" → its "report").
*So we never:* keep contradictory copy ("Nothing it offers is saved" beside pinned notes being written into
the summary), instruct users to install BlackHole, print a UTC worker stamp as if it were a clock time, or
regex-parse a date out of a meeting title.

**P7 — Density with air, at 1100×700 and at 4K.**
Compact rows (32–40px), 8px rhythm, 15/24 reading text. Fixed chrome is budgeted: ≤56px header + ≤40px dock.
Wide screens get a capped measure (720px / 72ch), never stretched cards.
*So we never:* drop below 12px for anything a person must read, or pad a settings card to `p-6` when `p-4`
reads calmer.

---

## 3. Information architecture & navigation model

### 3.1 Route / screen tree

```
/                       Home            idle:            Home (review queue + library digest)
                                        recording:       Live session   ← same route, presentational switch
                                        stopping/saving: Live session → Wrap-up pipeline
/meeting-details?id=&segment=&source=&jump=   Meeting Report (+ transcript evidence drawer)
/actions                Actions         Approved Action Center (read-only, ADR-0025)
/settings?section=…     Settings        general | appearance | recording | transcription | summary |
                                        privacy | learning | beta | license | about
/copilot                Copilot panel   🔒 BARE_ROUTE, frameless 380×520 second window
/design/*               Fixtures        page · primitives · shell · home · record · report · hitl ·
                                        speakers · actions · settings · dialogs · tour · copilot · learning
/notes/[id]             DELETED         legacy fake-data route (WP15)
```

🔒 **Route contract preserved exactly.** `/` stays the recording host because `src-tauri/src/tray.rs:115`
writes `sessionStorage.autoStartRecording` then `window.location.assign('/')`, consumed at
`useRecordingStart.ts:187-191`. `/settings` stays because `tray.rs:46` navigates to it. The Home↔Live switch
rule (`app/page.tsx:221-231`) is unchanged; what changes is that the recording state now *looks* like its
own screen instead of a dashboard with a pill glued to the bottom.

### 3.2 Shell: rail + Meetings pane + content

```
┌──────┬────────────────────┬──────────────────────────────────────────────┐
│ rail │ Meetings pane      │ content pane                                 │
│ 56px │ 280px (toggled)    │ fills; reading measure max 720px             │
└──────┴────────────────────┴──────────────────────────────────────────────┘
```

* **Rail — 56px, `bg-sidebar`, always visible, `z-30`.** Icon-only, 40×40 targets, Radix Tooltip on hover
  **and** focus (no `title=`).
  * top: brand mark (28px, 🔒 `/mityu-mark.svg`, alt 🔒 `Mityu`) — opens the **About / Help menu**
    (About, Check for updates, Shortcuts `?`, Report an issue). This kills today's three identical About
    entry points (Logo + Info ×2).
  * **Meetings** `Library` — toggles the Meetings pane (not a route; `aria-expanded` + `aria-controls`).
  * nav: **Home** `House` → `/` · **Actions** `ListChecks` → `/actions`, each with `aria-current="page"`,
    an active pill (`--sidebar-accent`) and a 2px left brand rule.
  * spacer
  * **Record control** (§3.5) — the single element carrying 🔒 `data-tour="record-button"`.
  * **Search** `Search` → opens the `⌘K` palette scoped to evidence search.
  * **Settings** `Settings2` → `/settings`.
  * version label `v{appVersion}` 🔒, `text-micro`, `--subtle-foreground`.
* **Meetings pane — 280px, `bg-background`, hairline right border.** Search field
  (🔒 placeholder `Search meeting evidence…`, 🔒 `aria-label="Search meeting evidence"`, 🔒 clear
  `aria-label="Clear meeting search"`, 🔒 ≥2-alphanumeric gate, 🔒 275ms debounce, 🔒 query never logged),
  date-grouped meeting rows, and a results header `Results for "q" · N` with a **Clear** control so the user
  can always get back (today the list swaps in place). All
  🔒 `SearchResultsList` roles and strings are preserved.
  * **Toggle:** rail item, `[`, or `⌘B`. **Persisted** to `localStorage['mityu.ui.meetingsPane']`.
    Default **open ≥1100px**, **closed <1000px** (today it is `useState(true)`, lost every launch).
  * 🔒 **`useSidebar().isCollapsed` keeps its exact meaning and polarity** (`false` = pane visible).
    `MainContent:11`, `app/page:42` → inline `4rem`/`16rem` at `:241`, and `StatusOverlays:271` all keep
    working; the inline width literals are replaced by `--rail-w` / `--pane-w`, not by a new flag.
    The `useSidebar()` return shape is unchanged — 14 files consume it and it throws outside its provider.
* **Content pane** owns its own scroll (🔒 `body { overflow:hidden; height:100% }` stays). One sticky
  `PageHeader` per screen, `z-10`. Settings renders its own 200px section column *inside* the content pane,
  so the Meetings pane never appears or disappears as a side effect of navigation.

### 3.3 The five journeys

1. **Home → Record.** Rail **Record** (or `⌘⇧R`, or the tray) → 🔒 `ensureRecordingConsent()` →
   licence gate → the `/` content pane becomes the **Live session**. The CTA **never** calls
   `recordingService` directly (🔒 it dispatches `start-recording-from-sidebar` on `/`, or sets
   `sessionStorage.autoStartRecording` + navigates, exactly as `SidebarProvider.tsx:151-169` does today).
2. **Record → Report.** Stop → the Live surface becomes a **Wrap-up** pipeline
   (Finalizing transcript → Saving meeting → Ready) driven by the already-written-but-never-rendered
   🔒 `RecordingStateContext.statusMessage`. 🔒 The success toast (`Recording saved successfully!` +
   `View Meeting`) is unchanged; the 2s `router.push('/meeting-details?id=…&source=recording')` now fires
   **only when the user is on `/`** (ADR required — see §0.3 item 6).
3. **Report → review.** The report opens on the Summary. Every block shows its status chip and
   `Source · 12:04` chip permanently, and a **labelled Approve** button; Edit/Reject reveal on
   hover/focus but stay in the DOM and tab order. A sticky bottom bar tracks `3 of 7 approved` and hosts
   **Approve summary**. A source chip opens the **evidence drawer** at that segment *and* seeks the
   playhead — the block you clicked stays on screen.
4. **Report → Actions.** Approving an action item surfaces it in **Actions**; each row keeps its
   🔒 `?id=&segment=&source=action-center&jump=` deep link.
5. **Anything → Settings.** Rail gear, tray, `⌘,`, or a deep link from an empty state
   (🔒 `Choose a model in Settings` → `/settings?section=summary`). Back is a breadcrumb, not
   `router.back()` next to a persistent rail.

**First run.** 🔒 Onboarding renders only when `get_onboarding_status.completed === false` inside Tauri;
step order 🔒 Welcome → Setup Overview → Downloads → Permissions (macOS only), 🔒 `totalSteps = isMac ? 4 : 3`.
The progress rail renders on **every** step. Completion calls the already-wired `onComplete` — no
`window.location.reload()`. Then the first-run tour: 🔒 `TOUR_ANCHORS`, 🔒 `WELCOME_COPY` and the three
🔒 `TOUR_STEPS` are verbatim, but **the automatic navigation to a sample meeting the user never recorded is
removed** and becomes an opt-in **"Try the sample report"** card on Home.

### 3.4 Keyboard map

| Shortcut | Action | Scope |
|---|---|---|
| `⌘/Ctrl+K` | Command palette (meetings, actions, settings sections, evidence search, commands) | global |
| `⌘/Ctrl+1…3` | Home / Actions / Settings | global |
| `⌘/Ctrl+,` | Settings | global |
| `⌘/Ctrl+⇧+R` | Start / stop recording (never bare `⌘R` — Tauri dev reload) | global |
| `Space` | Pause / resume capture **only while the session dock's Pause control itself has focus**; play/pause audio only while the scrubber has focus | **control-scoped — never document-level** |
| `⌘/Ctrl+B` or `[` | Toggle the Meetings pane | global |
| `⌘/Ctrl+F` or `/` | Focus evidence search / focus "Ask this meeting" on the Report | `⌘F` global · bare `/` only when not typing |
| `J` / `K` | Next / previous draft block | Report |
| `A` / `E` / `R` | Approve / Edit / Reject the focused block | Report, row focused |
| `S` | Open the focused block's source in the drawer | Report |
| `⌘/Ctrl+Enter` | Approve summary (only when the gate allows) | Report |
| `←` / `→`, `⇧←` / `⇧→`, `Home` / `End` | Playhead ∓5s / ∓30s / start / end | scrubber focused |
| `?` | Shortcuts sheet | only when not typing |
| `Esc` | Close overlay / cancel edit (🔒 existing semantics) | global |
| 🔒 `CommandOrControl+Shift+M / +A / +S` | Copilot panel / ask / capture — **Rust-registered, unchanged** | OS-global |

All additive. Every existing 🔒 `Enter`/`Escape` contract (reject reason, rename, search, license key, rule
editor, redaction term, language picker) is untouched.

**Activation scope — normative, not advisory.** Three rules, all enforced in `useShortcuts.ts` and all
in WP5's acceptance:

1. **`isTyping()` guard.** `J K A E R S ? /` and every other bare-character binding fire **only** when
   `document.activeElement` is not an `input`, `textarea`, `[contenteditable]` (this covers BlockNote)
   or an element with `role="textbox"`/`role="combobox"`, **and** no dialog, sheet or command palette
   is open. The earlier wording guarded only `J/K/A/E/R/S`, which left `?` marked *global*: typing a
   question mark into the reject-reason field, the Ask field, a rename field or the BlockNote editor
   would have opened the shortcuts sheet mid-sentence. `/` carries the same guard; `⌘/Ctrl+F` does
   not need it because a modifier chord cannot be typed as text.
2. **`J K A E R S` additionally require a focused row.** Unchanged from before: they are live only
   when a draft-block or action-item row has DOM focus.
3. **`Space` is never bound at document level.** It is the browser's default activation key, so it is
   left to the focused control: the session dock's **Pause** button (`<button>` → native activation)
   and the scrubber (`role="slider"`, §5.20). A document-level `keydown` handler for `Space` would
   pause a **live recording** whenever a user pressed it with a row, the dock or nothing in
   particular focused — that is CLAUDE.md §4's high-risk zone reached from a keyboard ticket, and it
   is forbidden. The dock's Pause control carries `aria-keyshortcuts="Space"` so the binding is still
   discoverable.

The production `contextmenu` suppression at `AppShell.tsx:124-130` is
removed for `input`, `textarea` and `[contenteditable]` (today it kills copy/paste in BlockNote and the
transcript on production builds).

### 3.5 Recording-active state (global)

While `isRecording`:

```
┌──────────────────────────────────────────────────────────────────────────────────────────┐
│ ● Recording · 12:34   🎙 MacBook Mic ▁▃▅▃▁  🔊 Display Audio ▁▁▂▁▁   [ ⏸ Pause ] [ ■ Stop ]│ 40px, z-40
└──────────────────────────────────────────────────────────────────────────────────────────┘
```

* The **session dock** renders at the bottom of the content pane on `/`, `/meeting-details`, `/actions` and
  `/settings`. `role="status" aria-live="polite"`, `--recording-surface` ground, `--recording` dot.
* 🔒 Copy is the existing contract: `Recording • mm:ss` / `Paused • mm:ss`, driven by backend
  `activeDuration`. Paused → `--warning` dot rendered as a **square** glyph plus the word — state is legible
  without colour and without motion.
* The rail Record control becomes a **Recording chip** with `aria-label="Recording, 12 minutes 34 seconds"`.
  The record / stop CTA itself is the `record` button variant and takes 🔒 `--recording-foreground`
  for its label — **not** `--primary-foreground`: white on the dark-theme `--recording` is 3.84:1 and
  fails AA (§4.4.4).
* 🔒 `RecordingStatusBar` keeps its component API and copy and remains the in-column marker inside the
  transcript view; the dock is the primary indicator. (The two never render in the same 100px — the dock is
  shell chrome, the bar is inside the scroller.)
* **Outside the window:** the window title and tray tooltip become `● Mityu — recording` /
  `❚❚ Mityu — paused`, so capture state is unambiguous when the app is behind something else.
* **Stop works from every route.** This requires `RecordingSessionProvider` (§0.3 item 6, WP6):
  `useRecordingStart` / `useRecordingStop` / `useModalState` / `useTranscriptRecovery` move from
  `app/page.tsx` into the **innermost** shell provider, so 🔒 `window.handleRecordingStop` survives
  navigation. 🔒 Stop is never disabled while recording.
* **Bottom-chrome budget.** At most **one** fixed bottom bar owns full height. When the dock and the
  Report's review bar coexist, the dock collapses to a 32px compact strip (dot · `12:34` · Stop) and the
  shell publishes `--bottom-chrome` (a CSS variable, 0 / 40 / 48 / 80px) that page bodies use for
  `padding-bottom: calc(var(--bottom-chrome) + 16px)` and the sonner `Toaster` uses for its `offset`.

---

## 4. Design tokens

### 4.1 Font

**Inter** via `next/font/google` (self-hosted at build; 🔒 no runtime network — the Tauri CSP allows no
external font or CSS host), weights **400 / 500 / 600**, `subsets: ['latin','latin-ext']`, exposed as
`--font-sans`.

Inter was drawn for 12–16px UI, has the x-height to hold up at the 13px label / 12px caption sizes this
density needs, ships true **tabular figures** (`'tnum' 1, 'cv05' 1`) which every timestamp, duration, stat
tile and `mm:ss` in this product depends on, and covers `latin-ext` (Turkish `ş ğ İ ı`). DM Sans's
geometric forms and proportional lining numerals are wrong for a compliance-facing transcript tool.
Monospace stays the **system stack** — declared so `font-mono` stops falling through, downloaded from
nowhere:

```ts
// frontend/src/app/layout.tsx
const sans = Inter({ subsets: ['latin','latin-ext'], weight: ['400','500','600'],
                     variable: '--font-sans', display: 'swap' });
```
```js
// tailwind.config.js
fontFamily: {
  sans: ['var(--font-sans)','ui-sans-serif','system-ui','-apple-system','Segoe UI','Roboto',
         'Helvetica Neue','Arial','sans-serif'],
  mono: ['ui-monospace','SFMono-Regular','SF Mono','Menlo','Consolas','monospace'],
},
```

### 4.2 CSS variables — `:root` (light)

🔒 Bare HSL triplets consumed as `hsl(var(--x))`, so every `/opacity` modifier in the codebase
(`bg-primary/20`, `ring-ring/50`, `bg-muted/60`) keeps working. Every token is defined on **both**
`:root` and `.dark` or it renders transparent in one theme.

```css
:root {
  /* ——— surfaces ——— */
  --background:            220 24% 97%;   /* #F6F7F9  app ground, rail, Meetings pane */
  --foreground:            218 22% 7%;    /* #0E1116  ink */
  --card:                  0 0% 100%;     /* #FFFFFF  content pane, cards, rows */
  --card-foreground:       218 22% 7%;
  --popover:               0 0% 100%;
  --popover-foreground:    218 22% 7%;
  --surface-2:             225 20% 95%;   /* #F0F1F5  sunken wells, code, tracks */
  --surface-3:             225 20% 92%;   /* #E7E9EF  pressed / selected well */

  /* ——— text ——— */
  --muted:                 225 20% 95%;
  --muted-foreground:      219 13% 40%;   /* #596273  secondary text */
  --subtle-foreground:     220 10% 42%;   /* #606876  meta / timestamps — clears 4.5:1 on
                                             card, background, surface-2/muted, surface-3
                                             and accent (min 4.63); the old 46% failed on
                                             surface-2 (4.34) and surface-3 (4.03) */

  /* ——— ink (the `default` button; NOT blue — see P1) ——— */
  --foreground-hover:      218 22% 16%;   /* #202632  ink fill, hover */
  --foreground-active:     218 22% 23%;   /* #2E3748  ink fill, pressed */

  /* ——— brand ——— */
  --primary:               225 100% 56%;  /* #1F57FF  bluedev #1E56FF, solid fills */
  --primary-foreground:    0 0% 100%;
  --primary-ink:           225 100% 54%;  /* #144FFF  brand TEXT + icons; a hair darker than
                                             the fill so it clears 4.5:1 on surface-3 too
                                             (4.82 vs 4.48 at 56%). The FILL --primary is
                                             unchanged: §10.10's #1E56FF promise is about the
                                             brand fill, not the text token. */
  --primary-hover:         225 79% 49%;   /* #1A4CE0  white label 6.66 */
  --primary-active:        225 79% 41%;   /* #163FBB  white label 8.53 */
  --secondary:             225 20% 95%;   --secondary-foreground: 218 22% 7%;
  --accent:                224 100% 96%;  /* #EBF0FF  active nav row, selected list row */
  --accent-foreground:     224 78% 41%;   /* #1742BA */

  /* ——— verified: "a human decided" (P1) ——— */
  --verified:              225 100% 56%;  --verified-foreground: 0 0% 100%;
  --verified-hover:        225 79% 49%;   --verified-active:     225 79% 41%;
  --verified-surface:      224 100% 96%;  --verified-ink:        224 78% 41%;
  --verified-border:       225 80% 85%;

  /* ——— structure & focus ——— */
  --border:                225 17% 91%;   /* #E4E6EC  hairline (decorative) */
  --border-strong:         220 14% 80%;   /* #C5CAD3  emphasised divider, scrollbar thumb —
                                             DECORATIVE ONLY, never a control's sole boundary */
  --input:                 220 12% 52%;   /* #768093  control boundary — ≥3:1 on card 3.98,
                                             background 3.71, surface-2/muted 3.52,
                                             surface-3 3.28, accent 3.49 (WCAG 1.4.11).
                                             The old 58% cleared 3:1 on --card ONLY. */
  --input-hover:           220 12% 44%;   /* #636C7E  input/outline hover boundary (min 4.35) */
  --ring:                  225 100% 50%;  /* #0040FF  focus — DECOUPLED from --primary */
  --overlay:               220 28% 10%;   /* #121721  scrim base, used at /0.55 */
  --scrollbar-thumb:       220 14% 80%;   --scrollbar-thumb-hover: 220 12% 64%;

  /* ——— status: x = solid fill · x-foreground = on the fill ·
                  x-surface = tint · x-ink = text/icon on tint or card ——— */
  --success:               156 79% 27%;  --success-foreground: 0 0% 100%;
  --success-surface:       152 45% 94%;  --success-ink:        156 82% 24%;
  --success-border:        152 44% 80%;

  --warning:               36 100% 27%;  --warning-foreground: 0 0% 100%;
  --warning-surface:       38 87% 94%;   --warning-ink:        36 100% 26%;
  --warning-border:        39 72% 79%;

  --destructive:           4 74% 49%;    --destructive-foreground: 0 0% 100%;
  --destructive-hover:     4 74% 43%;    --destructive-active: 4 74% 37%;
  --destructive-surface:   7 89% 96%;    --destructive-ink:    4 76% 38%;
  --destructive-border:    8 72% 86%;

  --info:                  205 86% 32%;  --info-foreground:    0 0% 100%;
  --info-surface:          207 69% 95%;  --info-ink:           205 86% 30%;
  --info-border:           205 66% 83%;

  /* ——— AI / draft register — violet: NOT amber, NOT brand blue ——— */
  --ai:                    256 58% 49%;  --ai-foreground:      0 0% 100%;
  --ai-surface:            253 88% 97%;  --ai-ink:             256 58% 49%;
  --ai-border:             256 73% 90%;

  /* ——— capture ——— */
  --recording:             4 74% 49%;    /* #D92D20 dot / solid */
  --recording-foreground:  0 0% 100%;    /* label ON the record/stop fill — 4.83 here.
                                            NEVER reuse --primary-foreground for this: in
                                            .dark that is white on #EA473E = 3.84 (FAIL). */
  --recording-hover:       4 74% 43%;    --recording-active: 4 74% 37%;
  --recording-ink:         4 76% 36%;    /* #A21F16 pill text */
  --recording-surface:     7 89% 96%;    /* #FEEEEC */
  --meter:                 225 100% 56%; --meter-hot: 36 100% 33%; --meter-clip: 4 74% 49%;
  --meter-track:           225 20% 92%;

  /* ——— shell ——— */
  --sidebar:               220 24% 97%;  --sidebar-foreground: 218 22% 7%;
  --sidebar-border:        225 17% 91%;  --sidebar-accent:     224 100% 96%;
  --sidebar-accent-foreground: 224 78% 41%;
  --sidebar-ring:          225 100% 50%;

  /* ——— descriptive series (speakers; identity, never ranking) ——— */
  --chart-1: 225 70% 52%;  --chart-2: 174 72% 30%;  --chart-3: 265 60% 55%;
  --chart-4: 35 85% 30%;   --chart-5: 340 65% 45%;  --chart-6: 198 80% 33%;

  /* ——— geometry & motion ——— */
  --radius: 0.5rem;       /* 8px base */
  --gutter: 1.5rem;       /* 24px; 16px below 1000px — the ONE responsive token, and globals.css
                             carries the `@media (max-width: 999.98px)` step that implements it */
  --rail-w: 3.5rem;       /* 56px */
  --pane-w: 17.5rem;      /* 280px */
  --header-h: 3.5rem;     /* 56px */
  --dock-h: 2.5rem;       /* 40px; 2rem when it shares the viewport with the review bar */
  --reviewbar-h: 3rem;    /* 48px */
  --bottom-chrome: 0px;   /* set by the shell: 0 / 40 / 48 / 80 */
  --measure: 45rem;       /* 720px reading measure */
  --dur-instant: 80ms;  --dur-fast: 140ms;  --dur-base: 200ms;  --dur-slow: 320ms;
  --ease-out: cubic-bezier(.2,.8,.2,1);
  --ease-in-out: cubic-bezier(.4,0,.2,1);
  --ease-emphasis: cubic-bezier(.16,1,.3,1);
}
```

### 4.3 CSS variables — `.dark` (near-black with elevated surfaces)

```css
.dark {
  --background:            223 24% 6%;    /* #0C0E13 */
  --foreground:            225 22% 93%;   /* #E9EBF1 */
  --card:                  222 22% 9%;    /* #12151C */
  --card-foreground:       225 22% 93%;
  --popover:               222 21% 12%;   /* #181C25 */
  --popover-foreground:    225 22% 93%;
  --surface-2:             222 21% 12%;
  --surface-3:             222 20% 16%;   /* #212631 */

  --muted:                 222 21% 13%;
  --muted-foreground:      219 15% 65%;   /* #98A2B3 */
  --subtle-foreground:     221 12% 58%;   /* #878FA1 — clears 4.5:1 on card, background,
                                             surface-2/popover, muted, surface-3 and accent
                                             (min 4.67); the old 53% failed on surface-2
                                             (4.42), muted (4.32) and surface-3 (3.93) */

  --foreground-hover:      225 22% 85%;   /* #D0D5E1  ink fill, hover */
  --foreground-active:     225 22% 78%;   /* #BBC1D3  ink fill, pressed */

  --primary:               225 100% 59%;  /* #2E62FF — white label passes AA at 4.88 */
  --primary-foreground:    0 0% 100%;
  --primary-ink:           225 100% 69%;  /* #6188FF  brand text/icon on dark — raised from
                                             67% so it clears 4.5:1 on surface-3 (4.67 vs
                                             4.32) as well as on card */
  --primary-hover:         225 100% 56%;  /* #1F57FF  white label 5.43 — the hover step goes
                                             DOWN in lightness in dark. Lightening the fill
                                             drops the white label below AA: the old
                                             225 100% 65% measured 3.82 and carried
                                             `Approve summary` and the frozen consent CTA.
                                             The usable dark band is 53–60% L (white ≥4.5 AND
                                             fill-vs-card ≥3:1); 56/54 sit inside it. This is
                                             also what today's `hover:bg-primary/90` does. */
  --primary-active:        225 100% 54%;  /* #144FFF  white label 5.85, fill vs card 3.12 */
  --secondary:             222 20% 16%;   --secondary-foreground: 225 22% 93%;
  --accent:                225 43% 16%;   /* #17203A */
  --accent-foreground:     224 100% 83%;  /* #A8BFFF */

  --verified:              225 100% 59%;  --verified-foreground: 0 0% 100%;
  --verified-hover:        225 100% 56%;  --verified-active:     225 100% 54%;
  --verified-surface:      225 43% 16%;   --verified-ink:        224 100% 83%;
  --verified-border:       225 45% 30%;

  --border:                223 20% 18%;   /* #252A37 */
  --border-strong:         222 18% 26%;   /* #363E4E — DECORATIVE ONLY */
  --input:                 222 14% 48%;   /* #69748C — ≥3:1 on card 3.89, background 4.12,
                                             surface-2/popover 3.64, muted 3.55,
                                             surface-3 3.23, accent 3.43. The old 43%
                                             cleared 3:1 on card/background/popover only. */
  --input-hover:           222 14% 58%;   /* #858EA3  input/outline hover boundary (min 4.61) */
  --ring:                  225 100% 72%;  /* #7094FF — DECOUPLED from --primary */
  --overlay:               223 40% 3%;    /* used at /0.65 */
  --scrollbar-thumb:       222 18% 26%;   --scrollbar-thumb-hover: 222 16% 36%;

  --success:               151 65% 58%;  --success-foreground: 152 70% 7%;
  --success-surface:       151 47% 10%;  --success-ink:        151 65% 62%;
  --success-border:        155 42% 19%;

  --warning:               38 92% 62%;   --warning-foreground:  36 85% 8%;
  --warning-surface:       38 65% 10%;   --warning-ink:         38 92% 66%;
  --warning-border:        40 61% 18%;

  --destructive:           358 62% 45%;  --destructive-foreground: 0 0% 100%;
  --destructive-hover:     358 62% 50%;  --destructive-active:  358 62% 54%;
  --destructive-surface:   355 38% 12%;  --destructive-ink:     3 100% 74%;
  --destructive-border:    353 41% 22%;

  --info:                  207 70% 38%;  --info-foreground:     0 0% 100%;
  --info-surface:          207 56% 12%;  --info-ink:            207 84% 68%;
  --info-border:           207 46% 22%;

  --ai:                    256 70% 58%;  --ai-foreground:       0 0% 100%;
  --ai-surface:            252 37% 14%;  --ai-ink:              256 100% 81%;
  --ai-border:             252 35% 25%;

  --recording:             3 80% 58%;    /* #EA473E — its own dark value, not the light one */
  --recording-foreground:  4 85% 10%;    /* #2F0704 — the record/stop LABEL in dark is a
                                            near-black red ink at 4.74, NOT white: white on
                                            #EA473E is 3.84 and fails AA. Same inversion the
                                            dark --success/--warning foregrounds already use. */
  --recording-hover:       3 80% 64%;    --recording-active:   3 80% 69%;
  --recording-ink:         3 100% 74%;   /* #FF817A */
  --recording-surface:     355 38% 12%;
  --meter:                 225 100% 67%; --meter-hot: 38 92% 62%; --meter-clip: 3 80% 58%;
  --meter-track:           222 20% 16%;

  --sidebar:               223 24% 6%;   --sidebar-foreground:  225 22% 93%;
  --sidebar-border:        223 20% 18%;  --sidebar-accent:      225 43% 16%;
  --sidebar-accent-foreground: 224 100% 83%;
  --sidebar-ring:          225 100% 72%;

  --chart-1: 225 100% 70%; --chart-2: 174 60% 50%; --chart-3: 265 85% 74%;
  --chart-4: 38 90% 60%;   --chart-5: 340 80% 68%; --chart-6: 198 85% 60%;
}
```

🔒 Definitions stay **class-scoped** (`.dark`), never `@media (prefers-color-scheme)` — `next-themes` writes
the class, `/design/page.tsx` wraps a subtree in `.dark` for side-by-side proof, and `tools/ui/shoot.py`
depends on class-based theming.

### 4.4 Contrast tables (computed, sRGB, WCAG 2.1)

**Method.** HSL → sRGB per CSS Color 4, rounded to 8-bit channels (so every number below is
reproducible from the hex in §4.2/§4.3), → relative luminance (4.5/12.92 · ((v+0.055)/1.055)^2.4)
→ (L1+0.05)/(L2+0.05). Thresholds: **4.5:1** for text under 24px/19px-bold, **3:1** for large text,
control boundaries and state indicators (1.4.11).

**Coverage rule (new).** A foreground token is tested against **every surface token the spec lets it
sit on**, not only `--card`. The declared surfaces are `--card`, `--background`, `--surface-2`
(= `--muted`), `--surface-3`, `--popover` and `--accent`. The earlier single-column discipline hid
three sub-threshold text pairs and four sub-threshold boundary pairs; §4.2/§4.3 were re-tuned until
the **minimum** column below clears the bar, and the minimum is what is bolded.

#### 4.4.1 Text on surfaces — light (`:root`), threshold 4.5

| Foreground | hex | `--card` | `--background` | `--surface-2` / `--muted` | `--surface-3` | `--popover` | `--accent` | **min** |
|---|---|---|---|---|---|---|---|---|
| `--foreground` | #0E1116 | 18.91 | 17.64 | 16.76 | 15.58 | 18.91 | 16.61 | **15.58** |
| `--muted-foreground` | #596273 | 6.14 | 5.73 | 5.44 | 5.06 | 6.14 | 5.39 | **5.06** |
| `--subtle-foreground` | #606876 | 5.62 | 5.24 | 4.98 | 4.63 | 5.62 | 4.93 | **4.63** |
| `--primary-ink` | #144FFF | 5.85 | 5.45 | 5.18 | 4.82 | 5.85 | 5.13 | **4.82** |
| `--verified-ink` / `--accent-foreground` | #1742BA | 8.32 | 7.76 | 7.37 | 6.85 | 8.32 | 7.30 | **6.85** |
| `--ai-ink` | #5B34C5 | 7.66 | 7.14 | 6.78 | 6.31 | 7.66 | 6.72 | **6.31** |
| `--recording-ink` | #A21F16 | 7.65 | 7.14 | 6.78 | 6.30 | 7.65 | 6.72 | **6.30** |
| `--success-ink` | #0B6F47 | 6.21 | 5.80 | 5.51 | 5.12 | 6.21 | 5.46 | **5.12** |
| `--warning-ink` | #855000 | 6.68 | 6.23 | 5.92 | 5.50 | 6.68 | 5.86 | **5.50** |
| `--destructive-ink` | #AB2117 | 7.09 | 6.62 | 6.28 | 5.84 | 7.09 | 6.23 | **5.84** |
| `--info-ink` | #0B578E | 7.59 | 7.08 | 6.72 | 6.25 | 7.59 | 6.66 | **6.25** |

#### 4.4.2 Text on surfaces — dark (`.dark`), threshold 4.5

| Foreground | hex | `--card` | `--background` | `--surface-2` / `--popover` | `--muted` | `--surface-3` | `--accent` | **min** |
|---|---|---|---|---|---|---|---|---|
| `--foreground` | #E9EBF1 | 15.32 | 16.20 | 14.30 | 13.98 | 12.71 | 13.51 | **12.71** |
| `--muted-foreground` | #98A2B3 | 7.09 | 7.50 | 6.62 | 6.47 | 5.88 | 6.25 | **5.88** |
| `--subtle-foreground` | #878FA1 | 5.63 | 5.95 | 5.26 | 5.14 | 4.67 | 4.96 | **4.67** |
| `--primary-ink` | #6188FF | 5.63 | 5.95 | 5.26 | 5.14 | 4.67 | 4.96 | **4.67** |
| `--verified-ink` / `--accent-foreground` | #A8BFFF | 10.06 | 10.63 | 9.39 | 9.17 | 8.34 | 8.87 | **8.34** |
| `--ai-ink` | #B89EFF | 8.15 | 8.62 | 7.61 | 7.44 | 6.76 | 7.19 | **6.76** |
| `--recording-ink` / `--destructive-ink` | #FF817A | 7.54 | 7.97 | 7.04 | 6.88 | 6.26 | 6.65 | **6.26** |
| `--success-ink` | #5FDDA0 | 10.73 | 11.34 | 10.02 | 9.79 | 8.90 | 9.46 | **8.90** |
| `--warning-ink` | #F8BE59 | 10.87 | 11.49 | 10.15 | 9.92 | 9.02 | 9.59 | **9.02** |
| `--info-ink` | #69B4F2 | 8.19 | 8.65 | 7.64 | 7.47 | 6.79 | 7.22 | **6.79** |

#### 4.4.3 Text on its own tint (the status / register pills and notices), threshold 4.5

| Pair | Light | Dark | Use |
|---|---|---|---|
| `--verified-ink` on `--verified-surface` | **7.30** | **8.87** | **Approved** pill |
| `--success-ink` on `--success-surface` | **5.63** | **9.50** | success notice |
| `--warning-ink` on `--warning-surface` | **6.07** | **9.70** | warning notice |
| `--destructive-ink` on `--destructive-surface` | **6.30** | **7.21** | error notice |
| `--info-ink` on `--info-surface` | **6.75** | **7.43** | info notice |
| `--ai-ink` on `--ai-surface` | **6.88** | **7.76** | AI **Draft** chip |
| `--recording-ink` on `--recording-surface` | **6.80** | **7.21** | `RecordingPill` text |
| `--warning` on `--recording-surface` | **5.62** | **9.76** | the **paused** square glyph inside the pill (§5.8) |

#### 4.4.4 Filled controls — label on idle / hover / active (threshold 4.5)

Every filled variant in §5.4 now has a specified hover and pressed fill, and each one is tested.
**There is no global "hover = darker" rule, and inventing one is how the old dark `--primary-hover`
broke.** The step direction is chosen per token by a single constraint — *move the fill far enough
from idle to be perceivable, without letting its own label fall under 4.5:1 or its own edge under
3:1 against the page* — and the direction that satisfies it is theme- and label-dependent: the ink
button moves toward the page in both themes; blue moves down in lightness in both; `destructive` and
`record` move down in light and **up** in dark, because in dark their labels are white and near-black
red respectively. The resulting ratio is tabulated for all twelve states rather than reasoned about.

| Control | Label token | Light: idle → hover → active | Dark: idle → hover → active |
|---|---|---|---|
| `default` (**ink**) | `--background` | 17.64 → **14.15** → **11.16** | 16.20 → **13.13** → **10.74** |
| `verified` / filled brand blue | `--primary-foreground` / `--verified-foreground` #FFF | 5.43 → **6.66** → **8.53** | 4.88 → **5.43** → **5.85** |
| `destructive` | `--destructive-foreground` #FFF | 4.83 → **5.96** → **7.44** | 6.02 → **5.09** → **4.58** |
| `record` | `--recording-foreground` | 4.83 → **5.96** → **7.44** | **4.74** → **5.60** → **6.55** |

Two corrections this table forced, both of which were live AA failures:

1. **Dark `--primary-hover` was `225 100% 65%` (#4D79FF): white on it is 3.82.** That is the hover
   state of the single most important control in the product — the filled `verified` button carrying
   `Approve summary` and the frozen string 🔒 `I confirm participants are informed — start recording`.
   It is now `225 100% 56%` (5.43). The usable dark band is **53–60% L**: below it the fill drops
   under 3:1 against `--card`, above it the white label drops under 4.5:1.
2. **The `record` variant painted `--primary-foreground` (white) on `--recording`.** In dark that is
   white on #EA473E = **3.84**. There is now a dedicated `--recording-foreground`: white in light
   (4.83), near-black red `4 85% 10%` in dark (4.74).

**The row that nearly was not monotonic, and the band that decides it.** Dark `destructive` was
first written 45% → 50% → **47%** (6.02 → 5.09 → 5.63): the pressed fill sat *between* idle and
hover, so a press moved the fill back *toward* idle and read as a partial un-hover rather than a
press. That was defended here as forced by the band, and it is not. The legal band for this token
is **45–54% L** — below 45% the fill drops under 3:1 against `--card` (idle is already only 3.04),
above 54% the white label drops under 4.5:1 (55% = 4.45). `--destructive-active` is therefore
`358 62% 54%`: the top of its own band, **4.58** on the white label and **3.99** against `--card`,
both passing, and the sequence 45 → 50 → 54 is monotonic like every other family. The ratio falls
across the ramp (6.02 → 5.09 → **4.58**) because in dark the label is white and the fill lightens;
what must not fall is AA, and it does not. Do not push past 54% — that is where 1.4.3 breaks.

**Disabled.** `disabled:opacity-50` is exempt from 1.4.3/1.4.11 (inactive controls), and the meaning
is carried by the mandatory visible sibling sentence / `aria-describedby` in §5 — never by the
colour and never by a tooltip.

#### 4.4.5 Non-text: control boundaries and fills (threshold 3.0)

`--surface-2` and `--muted` are byte-identical in light (`225 20% 95%`) but **not** in dark
(`222 21% 12%` vs `222 21% 13%`), so they get their own columns rather than a merged one — a merged
column would quietly report the better of the two.

| Token | hex | `--card` | `--background` | `--surface-2` | `--muted` | `--surface-3` | `--accent` | **min** |
|---|---|---|---|---|---|---|---|---|
| **light** `--input` | #768093 | 3.98 | 3.71 | 3.52 | 3.52 | 3.28 | 3.49 | **3.28** |
| **light** `--input-hover` | #636C7E | 5.28 | 4.93 | 4.68 | 4.68 | 4.35 | 4.64 | **4.35** |
| **dark** `--input` | #69748C | 3.89 | 4.12 | 3.64 | 3.55 | 3.23 | 3.43 | **3.23** |
| **dark** `--input-hover` | #858EA3 | 5.56 | 5.88 | 5.19 | 5.07 | 4.61 | 4.90 | **4.61** |

(Dark `--popover` = dark `--surface-2`, so the `--surface-2` column covers it; in light `--popover`
= `--card`.)

This is the fix for **seven** boundary pairs that were below 3:1 under the old `--input`
(light #8790A1: **2.998** on `--background`, **2.85** on `--surface-2`/`--muted`, **2.65** on
`--surface-3`, **2.82** on `--accent`; dark #5E687D: **2.98** on `--muted`, **2.71** on
`--surface-3`, **2.88** on `--accent`). The old value cleared 3:1 on `--card` only in light, and on
`--card`/`--background`/`--popover` only in dark. Three of the seven are reachable by design on
screens this document specifies: the per-row **Approve** button is `outline` with `hover:bg-muted`
(§5.4), the Meetings-pane search field sits on `bg-background` (§5.2), and §5.6 `Well`
(`bg-surface-2`) hosts inputs; the `--accent` pairs are reachable through any input or `outline`
control inside a selected / `aria-current` row (§5.15).

Filled surfaces against the page they sit on (1.4.11, ≥3 where the fill is the identifier):

| Fill | Light idle / hover / active vs `--card` | Dark idle / hover / active vs `--card` |
|---|---|---|
| `--primary` / `--verified` | 5.43 / 6.66 / 8.53 | 3.75 / 3.36 / 3.12 |
| `--destructive` | 4.83 / 5.96 / 7.44 | 3.04 / 3.59 / 3.24 |
| `--recording` | 4.83 / 5.96 / 7.44 | 4.76 / 5.62 / 6.56 |
| `--foreground` (ink) | 18.91 / 15.17 / 11.96 | 15.32 / 12.43 / 10.16 |
| `--ai` (draft 2px rule) | 7.66 | 3.24 |

Decorative, no SC applies: `--border` on `--card` **1.25**; `--border-strong` on `--card` **1.65**
light / **1.70** dark and on dark `--popover` **1.59**; `--verified-border` on `--verified-surface`
**1.44**; `--ai-border` on `--ai-surface` **1.28** light / **1.31** dark.

† `--border` / `--border-strong` separate two *filled* surfaces, which themselves identify the
regions — 1.4.11 does not apply, and **neither token may ever be a control's only boundary**. Any
boundary that *is* the only identifier of a control (inputs, selects, unfilled checkboxes, `outline`
buttons, in every state including hover) uses `--input` / `--input-hover`. This is why §5.5's input
hover is `border-input-hover` and **not** `border-border-strong`: the latter would have dropped a
focused-but-not-yet-focus-visible field's boundary to 1.65:1.

#### 4.4.6 The focus ring against every `ring-offset` surface (threshold 3.0)

The one thing that actually carries SC 2.4.11 is the 2px offset in the colour of the surface the
control sits on (§5 shared rules, §4.7 `ringOffsetColor`). Every declared offset surface is tested:

Two tokens share a row only where they carry the **same value in both themes** (`--background`/
`--sidebar`, `--accent`/`--verified-surface`, `--destructive-surface`/`--recording-surface`).
`--surface-2` and `--muted` do not, so they are listed separately.

| Offset surface | `--ring` light #0040FF | `--ring` dark #7094FF |
|---|---|---|
| `--background` / `--sidebar` | 6.17 | 6.77 |
| `--card` | 6.61 | 6.41 |
| `--popover` | 6.61 | 5.98 |
| `--surface-2` | 5.86 | 5.98 |
| `--muted` | 5.86 | 5.85 |
| `--surface-3` | **5.44** | **5.32** |
| `--accent` / `--verified-surface` | 5.80 | 5.65 |
| `--success-surface` | 5.99 | 5.68 |
| `--warning-surface` | 6.01 | 5.72 |
| `--destructive-surface` / `--recording-surface` | 5.87 | 6.13 |
| `--info-surface` | 5.88 | 5.82 |
| `--ai-surface` | 5.94 | 6.10 |

**Focus, stated honestly.** `--ring` #0040FF against `--primary` #1F57FF is **1.22:1** (1.71 in
dark) — decoupling the token alone does *not* satisfy SC 2.4.11. What satisfies it is the mandatory
2px offset above, whose worst case is **5.32:1**. The token is decoupled so that no component can
silently regress to today's 1.00:1, and **`ring-ring` without a `ring-offset-*` sibling is a lint
error** (§11.4 rule 3, with its stated blind spot).

#### 4.4.7 Descriptive series (charts), threshold 3.0

| Theme | `--chart-1…6` on `--card` |
|---|---|
| light | 5.84 / **4.56** / 5.59 / 5.97 / 5.81 / 5.61 |
| dark | 5.88 / **9.14** / 6.55 / 9.77 / 6.32 / 8.42 |

*(The light `chart-4` value previously read 5.42; recomputed from `35 85% 30%` = #8E570B on
#FFFFFF it is **5.97**. 5.42 is `chart-5` against `--background`. The series minimum is
`chart-2` at 4.56, now bolded. No accessibility consequence — every value clears 3:1 — but §0.1
picks candidate A on the strength of its arithmetic, so the arithmetic is corrected in place.)*

‡ Chart tokens are **fills and strokes only**. Speaker names and talk-time labels render in
`--foreground` / `--muted-foreground`; no chart token ever carries small text. Series are
distinguished by order and label as well as colour (ADR-0034 forbids leaderboard framing regardless).

**Register separation (§8.7 of the audit).** The AI register is violet (hue 256°) and the warning
register is amber (hue 36–38°) — 220° apart, and distinguishable in both themes without relying on
lightness. "This is a machine draft" can never be read as "something is wrong".

### 4.5 Type scale (Inter)

| Name | Size / line-height | Weight | Tracking | Usage |
|---|---|---|---|---|
| `text-display` | 24 / 32 | 600 | −0.02em | Onboarding step titles, empty-state heroes |
| `text-title-lg` | 20 / 28 | 600 | −0.015em | Page & report titles (`h1`) |
| `text-title` | 16 / 24 | 600 | −0.01em | Section cards, dialog titles, settings card `h2` |
| `text-title-sm` | 14 / 20 | 600 | −0.005em | Sub-sections, settings row titles (`h3`) |
| `text-read` | 15 / 24 | 400 | 0 | Report prose, transcript body, summary blocks |
| `text-body` | 14 / 22 | 400 | 0 | Default UI text, list rows, descriptions |
| `text-label` | 13 / 18 | 500 | 0 | Buttons, nav rows, form labels, toolbar |
| `text-caption` | 12 / 16 | 400 | +0.005em | Helper text, meta lines, tooltips — **and the floor for every compliance string** |
| `text-micro` | 11 / 16 | 500 | +0.01em | Counts and `mm:ss` chips, and the 380px copilot panel only |
| `text-eyebrow` | 11 / 16 | 600 | 0.08em, uppercase | Stat-tile labels, section eyebrows |
| `font-mono` | 12 / 16 | 400, `tnum` | 0 | Timestamps, license keys, keybinds, file paths |

Declared as real `theme.extend.fontSize` entries, so an undeclared step does not compile to a class —
structurally killing ADR-0037's phantom-class failure (`text-medium` ×2, `text-s` ×2, `background-blue-100`
and `shadow-xs` ×2 ship as silent no-ops today). This also removes all 79 arbitrary `text-[Npx]` usages.

**Hard rules.**
1. **12px is the floor for anything a person must read.** 🔒 `AiLabel` and 🔒 `AiMarking` move from 10px to
   `text-caption`; the Ask disclosure and every consent/legal string are `text-caption` or larger.
2. `text-micro` (11px) is an **uppercase eyebrow, a count, or a `mm:ss`** — never a sentence, never
   compliance. Its only sentence-level use is inside the 380px copilot window, which has no room and whose
   compliance strings are *still* `text-caption`.
3. Numerals that can be compared are always `font-mono` or `tabular-nums`.
4. Body text is AAA in both themes (18.91 / 15.32). Nothing load-bearing renders only in
   `--muted-foreground`.

### 4.6 Spacing scale

4px base: `0 · 2 · 4 · 6 · 8 · 12 · 16 · 20 · 24 · 32 · 40 · 48 · 64`.
Composition: inside a row `gap-2`; between rows `0` (hairline); inside a card `p-4`; between cards `gap-3`;
page gutter `var(--gutter)`; between page sections `gap-6`.
Control heights **28** (compact/inline) · **32** (default) · **36** (labelled row actions, incl. Approve) ·
**40** (primary + every rail target). Icon sizes 14 / 16 / 20.

### 4.7 Radius scale

`--radius: 0.5rem` (8px), mapped **all the way up** so a token change moves every corner. The step values
are chosen so that `sm`/`md`/`lg` reproduce today's 6 / 8 / 10px **exactly** — ~290 existing `rounded-*`
usages move zero pixels, while the ~95 orphaned `rounded` / `rounded-xl` / `rounded-2xl` usages become
token-driven for the first time:

```js
borderRadius: {
  xs:   'calc(var(--radius) - 4px)',  //  4  chips, checkbox, tiny badges
  sm:   'calc(var(--radius) - 2px)',  //  6  inputs, small buttons, row hover   (= today)
  md:   'var(--radius)',              //  8  buttons, cards, rows, toolbars      (= today)
  lg:   'calc(var(--radius) + 2px)',  // 10  section cards, stat tiles           (= today)
  xl:   'calc(var(--radius) + 4px)',  // 12  panels, dialogs, popovers, copilot
  '2xl':'calc(var(--radius) + 8px)',  // 16  onboarding hero, empty-state art only
}
```

**`ringOffsetColor` — the utilities the shared focus treatment actually needs.** Tailwind ships
exactly one offset colour by default (`ring-offset-white` / the `colors` palette), and this project
forbids raw palette utilities (§11.4 rule 1). Without the map below, `focus-visible:ring-offset-card`
**does not compile to any CSS**, every component in WP2–WP14 would emit a dead class, and §11.4
rule 3 (which makes bare `ring-ring` a lint error) would have no legal alternative. Declared in the
same `theme.extend` as `borderRadius`, in **WP1**:

```js
ringOffsetColor: {
  background:            'hsl(var(--background))',
  card:                  'hsl(var(--card))',
  popover:               'hsl(var(--popover))',
  sidebar:               'hsl(var(--sidebar))',
  muted:                 'hsl(var(--muted))',
  'surface-2':           'hsl(var(--surface-2))',
  'surface-3':           'hsl(var(--surface-3))',
  accent:                'hsl(var(--accent))',
  'verified-surface':    'hsl(var(--verified-surface))',
  'success-surface':     'hsl(var(--success-surface))',
  'warning-surface':     'hsl(var(--warning-surface))',
  'destructive-surface': 'hsl(var(--destructive-surface))',
  'info-surface':        'hsl(var(--info-surface))',
  'ai-surface':          'hsl(var(--ai-surface))',
  'recording-surface':   'hsl(var(--recording-surface))',
}
```

Fifteen entries resolving to **twelve distinct surface values** (some tokens are byte-identical in
both themes — `--sidebar` = `--background`, `--verified-surface` = `--accent`, `--recording-surface`
= `--destructive-surface`), all twelve certified against `--ring` at ≥5.32:1 in §4.4.6. The list is
**surfaces only, never fills**: Tailwind draws the offset as a box-shadow *outside* the border box,
so the offset must be the colour of whatever the control sits **on**, not the control's own fill. An
offset in the fill colour would paint a halo, not a gap. The same `theme.extend.colors` block gains
the new interaction and label tokens so `bg-foreground-hover`, `bg-primary-active`,
`bg-verified-hover`, `bg-destructive-hover`, `bg-recording-hover`, `text-recording-foreground` and
`border-input-hover` are real utilities too.

### 4.8 Elevation scale

Hairline-first: **elevation is a last resort, a surface step is the default.**

| Token | Light | Dark | Use |
|---|---|---|---|
| `--elev-0` | `none` + `1px solid hsl(var(--border))` | same | cards, rows, toolbars, panes — **the default** |
| `--elev-1` | `0 1px 2px hsl(var(--overlay)/.06), 0 1px 1px hsl(var(--overlay)/.04)` | `inset 0 1px 0 hsl(0 0% 100%/.04)` + surface step to `--popover` + `1px --border-strong` | dropdown, select, tooltip, popover |
| `--elev-2` | `0 8px 24px -8px hsl(var(--overlay)/.18), 0 1px 2px hsl(var(--overlay)/.08)` | `0 8px 24px -8px hsl(var(--overlay)/.6)` + inset highlight + `--border-strong` | dialog, sheet, command palette, toast, sticky bars |
| `--elev-3` | `0 16px 48px -12px hsl(var(--overlay)/.28)` | `0 16px 48px -12px hsl(var(--overlay)/.75)` + inset highlight | copilot window, coach-mark popover |

Black-alpha blur on `#0C0E13` reads as mud, so in dark each level *also* steps the surface
(`--card` → `--popover` → `--surface-3`), gains a `--border-strong` hairline, and gains a 1px inset top
highlight (`inset 0 1px 0 hsl(0 0% 100%/.04)`) which is what actually makes an edge read on ink.
No component may use a raw Tailwind `shadow-*`.

### 4.9 Motion scale

| Token | Duration | Easing | Applies to |
|---|---|---|---|
| `--dur-instant` | 80ms | `--ease-out` | hover/press colour, focus ring |
| `--dur-fast` | 140ms | `--ease-out` | chips, switches, tooltip, row reveal of HITL controls |
| `--dur-base` | 200ms | `--ease-out` | popover / dropdown / dialog in, tab indicator, toast |
| `--dur-slow` | 320ms | `--ease-emphasis` | evidence drawer, pane collapse, onboarding step |

**Reduced motion** (today: zero handling anywhere in `src/`, against 111 `animate-*`, 202 `transition-*`,
11 framer-motion files and a `vibrate` shake keyframe that is an SC 2.3.3 vestibular trigger):

```css
@media (prefers-reduced-motion: reduce) {
  *, *::before, *::after {
    animation-duration: 1ms !important; animation-iteration-count: 1 !important;
    transition-duration: 1ms !important; scroll-behavior: auto !important;
  }
}
```
plus `useReducedMotion()` from framer-motion in the 11 files that use it. **The fallbacks are semantic, not
merely faster:** the recording dot stops pulsing and becomes a solid dot *while the text carries the state*;
🔒 `mityu-tour-pulse` becomes a static 2px `--ring` outline; the typewriter transcript effect renders final
text immediately; skeletons stop shimmering and show flat `--surface-2`; the jump-to-source highlight does
not fade — it stays for 4s; `animate-spin` becomes a static three-quarter ring beside its existing
`role="status"` text; the `vibrate` keyframe is deleted outright.

### 4.10 z-index scale

| z | Layer |
|---|---|
| 0 | page content |
| 10 | sticky page header, report review bar |
| 20 | sticky pane toolbars, transcript segment highlight |
| 30 | app rail + Meetings pane |
| 40 | session dock (recording strip) |
| 60 | dialog + sheet overlay |
| 70 | dialog + sheet content, evidence drawer, **and every floating layer** — popover / dropdown / select / tooltip / command palette. DOM order is the tiebreaker (see below). |
| 80 | toasts (sonner) |
| 90 | full-screen drag-drop import overlay |
| 95 | 🔒 first-run tour coach-mark + spotlight (`CoachMarkTour` ships `z-[95]` today) |
| 96 | tour popover |
| 100 | onboarding full-screen shell (gates everything) |

**Why the floating layers are 70 and not 50 (ADR-0055).** Radix portals every one of them to
`document.body`, so a `Select`, `DropdownMenu`, `Popover` or `Tooltip` opened *from inside* a dialog
is a **sibling** of that dialog, not a descendant: at `z-50` it renders behind `z-70` dialog content
and the picker is unusable. Sharing the band and letting DOM order decide is correct for the common
case, because a layer opened from a dialog is appended after it. The known cost is the reverse case:
a **non-modal** floating layer that is already open when a dialog opens paints above the `z-60`
overlay until it closes. Radix's own dismiss behaviour closes it on the dialog's focus trap in
practice; a call site that keeps one open across a dialog boundary must close it itself.

Exposed as `theme.extend.zIndex` so no component hand-writes `z-[95]` again.

### 4.11 Fixed-chrome contract

Any component that fixes itself to an edge must declare its height into the shell, and the shell publishes
the sum:

* `--header-h` 56px (sticky, in-flow) · `--dock-h` 40px (32px when the review bar is present) ·
  `--reviewbar-h` 48px.
* `--bottom-chrome` = 0 / 40 / 48 / 80px, set by `AppShell`.
* Every page body ends with `padding-bottom: calc(var(--bottom-chrome) + 16px)`.
* The sonner `<Toaster>` takes `offset={bottomChrome + 16}` so a toast never sits under a bar.
* Total fixed chrome may not exceed **136px** (≈19%) of a 700px window; when both bottom bars are present
  the dock collapses first.

---

## 5. Component system

Every component is token-only. No raw palette utility, no `bg-white`, no `text-gray-*`, no `#hex` —
an ESLint rule (§11.4) enforces it, allow-listed for `/design/*`.

**Shared rules.** One focus treatment app-wide:

```
focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring
focus-visible:ring-offset-2 focus-visible:ring-offset-{surface}
```

**`{surface}` is not a placeholder for the implementer to invent** — it is one of the fifteen
`ringOffsetColor` entries declared in §4.7, chosen by **what the control sits on**, and the choice is
mechanical:

| The control is… | offset utility |
|---|---|
| inside a `Card`, a list row, a `Section` card, the page header, a toolbar | `ring-offset-card` |
| on the app ground: the rail, the Meetings pane, a page body, an `EmptyState` | `ring-offset-background` |
| inside a popover, dropdown, select, tooltip, dialog, sheet, command palette, toast | `ring-offset-popover` |
| inside the rail specifically (where `--sidebar` may diverge from `--background`) | `ring-offset-sidebar` |
| inside a `Well`, a segmented control, a progress/meter track, a skeleton | `ring-offset-surface-2` |
| inside a pressed or selected well | `ring-offset-surface-3` |
| inside a selected / `aria-current` row or a `verified` tint | `ring-offset-accent` / `ring-offset-verified-surface` |
| inside a `Notice` or a status pill | `ring-offset-{tone}-surface` (`info` / `warning` / `success` / `destructive` / `ai`) |
| inside the session dock's recording strip | `ring-offset-recording-surface` |

A **filled** control (`default`, `verified`, `destructive`, `record`) takes the offset of its
*parent* surface, never its own fill — the offset is drawn outside the border box, so matching the
fill would paint a halo instead of a gap. Every one of the twelve distinct surface values is
certified against `--ring` at ≥5.32:1 in §4.4.6, so the treatment is uniform and never needs a
per-component override.
This replaces today's five competing treatments and the 16 files that strip `outline-none` with no
replacement. Icon-only controls always carry an `aria-label` **and** a Radix Tooltip that opens on focus as
well as hover. **A disabled control always carries a visible sibling sentence or `aria-describedby`** — a
disabled button is not focusable, so a tooltip on it never opens for keyboard or AT users. This matters most
for 🔒 `Export approved summary` ("Approve the summary to export") and the five 🔒 `Approve summary` gate
reasons.

### 5.1 App rail (`components/shell/AppRail.tsx`)
* 56px column, `bg-sidebar`, `border-r border-sidebar-border`; brand-mark button (40×40); Meetings toggle;
  nav items (40×40, icon 20); spacer; Record control; Search; Settings; version.
* States: default / hover (`bg-sidebar-accent/60`) / active (`bg-sidebar-accent`, 2px left `--primary` rule,
  `aria-current="page"`) / focus-visible / recording (Record becomes the Recording chip).
* `<nav aria-label="Primary">`; Meetings toggle carries `aria-expanded` + `aria-controls`.
* 🔒 Exactly **one visible** `data-tour="record-button"` at any time (today the Sidebar renders it at `:466`
  collapsed **or** `:779` expanded, never both). `TourProvider.tsx:61-79` resolves it by selector.

### 5.2 Meetings pane (`components/shell/MeetingsPane.tsx`)
* 280px, `bg-background`, hairline right border; sticky 44px header; search field; scroll region; sticky
  group headers (`text-eyebrow`, `bg-background/95 backdrop-blur`).
* States: open / collapsed (width 0, content kept mounted until the transition ends) / searching /
  results / too-short / empty / error — 🔒 all `SearchResultsList` roles and strings preserved
  (`role=list aria-label="Meeting search results"`, `role=listitem`,
  `aria-label="Open source in ${title} at ${time}"`, `Transcript` chip,
  `Type at least 2 letters or numbers to search.`, `Searching local meeting evidence...`,
  `No matching meeting evidence found.`, and the error string).
* `<aside aria-label="Meetings">`; rows are links in a `role="list"`.

### 5.3 Page header (`components/shell/PageHeader.tsx`)
* Sticky, `h-14`, `bg-card/85 backdrop-blur`, `border-b border-border`; left: eyebrow + `h1`
  (`text-title-lg`), optionally inline-editable; right: at most 1 primary + 1 secondary + `⋯` overflow.
* States: at-top (no border) / scrolled (border + `--elev-1` in light, surface step in dark) / with-banner
  (the shared `SystemNotices` slot sits **above** it with one 8px gutter — ending today's three competing
  top gutters).
* Replaces the orphan `components/MainNav` (`h-0`, renders nothing) and gives the Report the title editor
  that `SummaryPanel.tsx:5` imports and never renders.

### 5.4 Buttons (`ui/button.tsx`)

Sizes: `xs` h-7 · `sm` h-8 · `default` h-9 · `row` h-9 with a visible label (row actions) · `lg` h-10 ·
`icon` 32×32 · `icon-lg` 40×40. Base: `rounded-md gap-2 [&_svg]:size-4`,
`transition-colors var(--dur-instant)`, the shared focus treatment,
`disabled:opacity-50 disabled:pointer-events-none`.

**Every variant specifies idle, hover, active and disabled.** A variant with only an idle fill forces
the implementer to invent the interaction colour of the product's primary and its most
compliance-critical controls, and an invented value cannot be checked against §4.4.

| Variant | Idle | Hover | Active (pressed) | Disabled | Use |
|---|---|---|---|---|---|
| `default` | `bg-foreground text-background` (**ink**) | `bg-foreground-hover` | `bg-foreground-active` | `opacity-50` + visible reason | neutral primary: Generate, Continue, Save, Retry |
| `verified` | `bg-verified text-verified-foreground` (**brand blue**) | `bg-verified-hover` | `bg-verified-active` | `opacity-50` + visible reason | the one human-commit action per viewport: `Approve summary`, 🔒 `I confirm participants are informed — start recording` |
| `outline` | `border border-input bg-card` | `bg-muted border-input-hover` | `bg-surface-3 border-input-hover` | `opacity-50` + visible reason | secondary, toolbar, form, **per-row `Approve`** |
| `ghost` | transparent | `bg-muted` | `bg-surface-3` | `opacity-50` + visible reason | row + toolbar icon actions |
| `link` | `text-primary-ink no-underline underline-offset-4` | `underline` | `underline decoration-2` | `opacity-50` | inline links — **the colour never moves**, so it stays ≥4.67:1 on every surface; only the decoration changes |
| `destructive` | `bg-destructive text-destructive-foreground` | `bg-destructive-hover` | `bg-destructive-active` | `opacity-50` + visible reason | Delete, Deactivate, Confirm reject |
| `record` | `bg-recording text-recording-foreground rounded-full` | `bg-recording-hover` | `bg-recording-active` | 🔒 **never disabled while recording** | the record / stop CTA only |

Every label/fill pair above, in both themes, is computed in **§4.4.4**; every fill-against-page pair
in **§4.4.5**; the boundary pairs for `outline` in **§4.4.5**. Three notes the implementer must not
"simplify" away:

* `record` takes **`text-recording-foreground`**, not `text-primary-foreground`. White on dark
  `--recording` #EA473E is **3.84:1** and fails AA; the dark value of `--recording-foreground` is a
  near-black red at 4.74.
* In **dark**, the blue hover and active steps go *down* in lightness, not up. The white label only
  clears 4.5:1 in a 53–60% L band, and the previous `--primary-hover` of `225 100% 65%` measured
  **3.82:1** underneath `Approve summary` and 🔒 `I confirm participants are informed — start
  recording`.
* `outline`'s hover keeps `bg-muted`, which is only safe because `--input` was re-tuned in §4.2/§4.3:
  at the old value the per-row **Approve** button's only boundary fell to **2.85:1** on hover.

**Removed:** `green`, `blue`, `red`, `gray` (raw palette; `green` and `gray` had hover === idle).
Migration: `variant="green"` → `outline` (row-level Approve) or `default`; `variant="red"` →
`destructive`; `variant="blue"` → `default`.

`ui/button.tsx` has 29 consumers. `buttonVariants` is `cva` + `VariantProps<typeof buttonVariants>`
(`ui/button.tsx:3,41-44`), so **deleting a variant key turns every remaining call site into a type
error** — `pnpm tsc --noEmit` fails the moment the key is gone unless the call sites move in the
same commit. There are **seven** legacy-variant call sites, and only two are in `LearningSettings`:

| File | Line | Variant | Migrates to |
|---|---|---|---|
| `components/LearningSettings.tsx` | 272 | `green` | `default` (Save edit) |
| `components/LearningSettings.tsx` | 291 | `green` | `outline` (Activate) |
| `components/AISummary/DraftSummaryView.tsx` | 419 | `green` | `outline` |
| `components/AISummary/DraftSummaryView.tsx` | 445 | `red` | `destructive` |
| `components/AISummary/DraftSummaryView.tsx` | 669 | `green` | `outline` |
| `components/AISummary/DraftSummaryView.tsx` | 695 | `red` | `destructive` |
| `components/AISummary/DraftSummaryView.tsx` | 1099 | `green` | `verified` (the one `Approve summary`) |

All seven migrate in **WP2**, in the commit that deletes the keys — `DraftSummaryView.tsx` is
therefore in WP2's blast radius as well as WP10's. WP10 later *restyles* those rows; it is not
allowed to be the package that unblocks `tsc`. The two `LearningSettings` sites are covered only by
copy assertions, so a silent visual break there is possible — check them by eye.

This is what keeps blue from flooding the UI: **the default button is ink; blue appears only where a human
signs.** It also makes P1 a type-level constraint rather than a review convention.

### 5.5 Inputs (`ui/input`, `textarea`, `select`, `switch`, `checkbox`)
* h-9 (textarea `min-h-20`), `rounded-sm`, `bg-card`, `border border-input`, `px-3`, `text-body`,
  placeholder `--subtle-foreground`.
* States: idle / hover (**`border-input-hover`**, never `border-border-strong` — that token is
  decorative at 1.65:1 and would drop the field's only boundary below 3:1 on hover, §4.4.5) /
  focus (`border-ring` + the ring treatment) / invalid (`aria-invalid`, `border-destructive`, message
  `role="alert"` in `--destructive-ink`, never colour-only) / disabled (`bg-muted`, boundary stays
  `border-input` — 3.52 light / 3.55 dark on `--muted`) / read-only (`bg-surface-2`, no border).
* A field inside a `Well` (`bg-surface-2`) or in the `bg-background` Meetings pane keeps
  `border-input`: §4.2/§4.3 re-tuned `--input` precisely so those two grounds clear 3:1 (3.52 / 3.28
  light, 3.64 / 3.23 dark). The placeholder is `--subtle-foreground`, which now clears 4.5:1 on all
  six declared surfaces (§4.4.1/§4.4.2) — at its previous value it measured 4.34 inside a Well.
* Switch 36×20, `bg-input` off / `bg-primary` on, 16px thumb. Checkbox 16×16 `rounded-xs` — **replaces the
  two raw `<input type="checkbox">`** in `RecordingConsentDialog.tsx:91-96` and `recordingNotification.tsx`.
* 🔒 The locked API-key pattern (transparent overlay + `animate-vibrate`) becomes a `readOnly` field with a
  `Lock` adornment, the sentence 🔒 `Stored securely — enter to replace` and a visible **Replace key**
  button. Keys are still never echoed back.

### 5.6 Surfaces (`Card`, `Section`, `Well`)
* `Card` — `bg-card border border-border rounded-lg`; header `px-4 h-11` (`text-title` + count chip + right
  actions); body `p-4`; footer `px-4 py-3 border-t`. No shadow.
* `Section` — headerless grouping: `text-eyebrow` label + `space-y-3`.
* `Well` — `bg-surface-2 rounded-sm p-3` for inert content (code, paths, quoted original text).
  **Maximum nesting depth 2** (card → well); the Beta tab's three-level nesting collapses.
* Replaces the ~20 literal `"bg-card rounded-lg border border-border p-6 shadow-sm"` strings in Settings and
  the `rounded-2xl` `SectionCard` in `report/primitives.tsx` (which 🔒 keeps its props
  `icon, title, count, accent, aiLabel, actions, children`).

### 5.7 Tabs / segmented control (`ui/tabs`)
* Underline tabs for page level (2px `--primary` indicator, `text-label`, 36px, `border-b` track);
  segmented control (`bg-surface-2 p-0.5`, active pill `bg-card` + `--elev-1`) for Theme-style switches.
* The indicator is a CSS `::after` driven by `data-state` + a custom property — **not** a framer-motion
  `layoutId` positioned from `offsetLeft` in `useLayoutEffect` (today's underline misaligns on resize and
  font load, `app/settings/page.tsx:57-65`).
* 🔒 **The `variant` DEFAULT is `plain` — no track, no height, no `::after`.** A shape is a migration the
  owning package performs, not something the primitive performs on every consumer at once: while
  `/settings` still owns its framer-motion bar, an `underline` default paints **two** 2px primary rules
  on it and collapses a `py-4` trigger to `h-9`, and the call site cannot opt out because
  `data-[state=active]:after:bg-primary` sits in a modifier group nothing at the call site can merge
  away. WP12 passes `variant="underline"` in the same change that deletes the framer-motion bar.

### 5.8 Status pill / badge (`ui/badge.tsx`, `report/StatusPill.tsx`)

Single primitive, `h-5 px-1.5 rounded-xs text-caption`, optional 6px leading dot. **Never colour-only:**
every pill carries its word *and* an icon.

| Tone | Surface / ink | Where |
|---|---|---|
| `neutral` | `--surface-2` / `--muted-foreground` | counts, "Legacy" |
| `ai` (**Draft**) | `--ai-surface` / `--ai-ink`, dashed `--ai-border`, `FileEdit` | unreviewed AI block or action item |
| `verified` (**Approved**) | `--verified-surface` / `--verified-ink`, `Check` | approved block, approved action |
| `edited` (**Edited**) | transparent / `--verified-ink`, `--verified-border`, `Pencil` | human-edited block |
| `destructive` (**Rejected**) | `--destructive-surface` / `--destructive-ink`, `X` | rejected block (🔒 text also `line-through`) |
| `info` | `--info-surface` / `--info-ink` | "Beta", "Recommended" |
| `warning` | `--warning-surface` / `--warning-ink` | things that are **wrong** — never AI status |

**`AiLabel`** — the per-card Art. 50 chip. 🔒 It is a plain `<span>` with **no `role`** and **no
`aria-label`** (see §0.3 item 1): `Sparkles` 12px + visible text 🔒 `AI-generated · review required`,
`--ai-surface` / `--ai-ink`, `border border-ai-border`, **`text-caption` (12px, up from 10px)**. It renders
`children` as text only, so it structurally cannot contain a control.

**`RecordingPill`** — `--recording-surface` ground, `--recording-ink` label,
🔒 `Recording • 12:34` / 🔒 `Paused • 12:34` in tabular numerals. **The glyph differs by state and is
not a colour swap:**

| State | Glyph | Token | Motion |
|---|---|---|---|
| recording | ● **round dot** | `--recording` (4.83 light / 4.76 dark vs `--card`) | pulses |
| paused | ■ **square** | `--warning` (5.62 light / 9.76 dark vs `--recording-surface`, §4.4.3) | static |

This matches §3.5, §6.2 and the MUST-PRESERVE register (`RecordingStatusBar.tsx:33-45`: "pulsing red
dot + `Recording • MM:SS`, **orange** dot + `Paused • MM:SS`") and it is load-bearing under
`prefers-reduced-motion`, where §4.9 removes the pulse: with the pulse gone, **shape and hue are the
only two differentiators left**, so a red dot for both states would erase the distinction entirely.
Building `RecordingPill` with one red dot for both states is a defect, not a simplification.

### 5.9 Source-link chip (`SourceChip`)
The single most important small component in the product: the link from a claim to its proof.
* **Anatomy** — `<button>`, h-6, `px-2`, `rounded-xs`, `border border-input`, `bg-card`, `Clock` 12px
  in `currentColor`, then 🔒 the **visible word `Source`** in **`text-primary-ink`** followed by
  `· 12:04` in `font-mono text-micro tabular-nums` in **`text-subtle-foreground`**.
  §4.1 pins "visible 'Source' text"; the timestamp is added, never substituted.
* **Colour contract** — the label is `--primary-ink` (≥4.82 light / ≥4.67 dark on every declared
  surface, §4.4.1/§4.4.2) and the timestamp is `--subtle-foreground` (≥4.63 / ≥4.67). The boundary is
  `--input`, not `--border`: the chip is a control, and `--border` at 1.25:1 may never be a control's
  only boundary (§4.4.5). A chip is **never** distinguished by colour alone — the word `Source` and
  the `Clock` icon are always present.
* **Behaviour** — click opens the evidence drawer at 🔒 `onJumpToSource(source_chunk_id)` **and** seeks the
  playhead via 🔒 `PlaybackBarHandle.seekTo(sec)` when `audio_start_time` is known. In a product whose
  thesis is a defensible record, verifying a claim by *hearing* it is strictly stronger than reading it.
* **States** — idle (`bg-card`, `text-primary-ink`, `border-input`) /
  hover (`bg-accent`, `border-primary-ink`, label stays `--primary-ink` at **5.13** on `--accent`
  light / **4.96** dark, timestamp `--subtle-foreground` at 4.93 / 4.96) /
  focus-visible (the shared treatment with `ring-offset-card`, or `ring-offset-accent` when the chip
  sits in a selected row) /
  pressed (`bg-surface-3`; label 4.82 / 4.67) /
  **active** (the drawer currently shows this segment → `bg-accent` + 2px `--primary` left rule +
  `aria-expanded="true"`; the rule, not a colour change, is what marks it) /
  **unresolved** (the item has **no `source_chunk_id`** — `aria-disabled` + a visible sibling
  sentence *"This item has no transcript link"*; export stays 🔒 fail-closed). 🔒 **A MISSING
  TIMESTAMP IS NOT AN UNRESOLVED ITEM.** `DraftBlock` / `DraftActionItem` declare `source_chunk_id`
  as the REQUIRED evidence anchor and carry no time field at all, so a chip keyed on the clock
  string would be dead for every real block and would claim, falsely, that an item with a perfectly
  good link has none. The UTC `sourceTimestamp` worker fallback is still **never printed as if it
  were a clock time** — the chip reads plain `Source`, and it still clicks. `aria-disabled` rather
  than the `disabled` attribute, because a `disabled` button is not focusable and the
  `aria-describedby` carrying the explanation would be unreachable for exactly the users it is for.
  `aria-controls` is emitted only when the caller names a drawer that is actually mounted — a
  dangling IDREF is an axe `aria-valid-attr-value` violation.
* **A11y** — 🔒 `aria-label="Jump to source transcript segment"` (blocks) ·
  🔒 `Open the transcript at {timestamp}` with visible text 🔒 `Source · {timestamp}` (Ask claims) ·
  🔒 `Open source in {meeting} at {time}` (Actions). Plus `aria-expanded` + `aria-controls="transcript-drawer"`.

### 5.10 HITL block controls (`ReviewControls`) and the review bar
* **Row anatomy (left → right).** A 3px status rule down the full row height — `--ai` **dashed** for draft,
  `--verified` **solid** for approved, `--destructive` for rejected — then block text at `text-read`, then a
  meta row (StatusPill · SourceChip · `Original` toggle when edited), then the action cluster.
  Draft-vs-approved reads from across the room, by rule weight *and* chip *and* word.
* **Action cluster.** 🔒 `Approve` is **always visible and labelled**: a 36px `outline` button with `Check`
  + the word "Approve" (it is `outline`, not filled blue, because there are many per screen and blue is
  reserved for the one commit — §0.3 item 7). `Edit` (`Pencil`) and `Reject` (`X`) are icon buttons revealed
  on `hover` / `focus-within` / row keyboard focus, and are **always in the DOM and tab order**
  (`opacity-0 focus-within:opacity-100 group-hover:opacity-100`, never `display:none`, never
  `pointer-events-none`). Under `@media (pointer: coarse)` all three are always visible.
* 🔒 **Accessible names unchanged:** `Approve block`, `Edit block`, `Reject block`, `Confirm reject`,
  `Cancel reject`, `Reason for rejecting this block (optional)`, `Approve action item`, `Edit action item`,
  `Reject action item`, `Reason for rejecting this action item (optional)`, `Show originally generated text`.
* **States.** draft / approved (rule turns `--verified`, the Approve control is replaced by a static
  `Approved` pill) / editing / rejecting / **locked** (🔒 `areBlockReviewControlsLocked(summaryStatus,
  isApprovingSummary, isBlockMutationPending)` → all disabled with **one shared visible explanation line**,
  not a tooltip) / pending (spinner on the pressed control only, `aria-busy` on the row).
* 🔒 Reject opens an inline field beneath the block, placeholder
  `Why is this wrong? (optional — press Enter to reject)`; the reason is **optional and never gates**;
  Enter submits blank, Escape cancels.
* 🔒 Optimistic mutation with **revert on `false`** (`useHitlAction`) + error toast is unchanged.
* **Review bar** (`report/ReviewBar.tsx`) — sticky at the **bottom** of the report column (today it is at the
  top, far from the last block): `3 of 7 approved` as `role="progressbar"` with
  `aria-valuenow/valuemin/valuemax/aria-label="Blocks approved"`, 🔒 `Export ▾`
  (`aria-label="Export approved summary"`, disabled with the visible sentence 🔒 *Approve the summary to
  export*), and 🔒 `Approve summary` → `Summary approved` as the one `verified` button, with all five gate
  reasons rendered as a **visible sentence** beside it, not only as a tooltip.
  The word is **"approved"**, never "signed" — the product provides no e-signature.

### 5.11 Stat tile (`StatTile`)
`bg-card border border-border rounded-lg p-3`; `text-eyebrow` label + 14px icon; value `text-title-lg
tabular-nums`; optional `text-caption` sub-line. Rendered as a `<dl>`.
States: value / empty (`—`) / loading (skeleton) / **not applicable (tile omitted, never rendered as `0`)**.
🔒 Descriptive metrics only: Duration · Words (+wpm) · Segments · Action items · **Approved `3 / 7`**
(review progress above the fold) · Speakers when diarization is done.
**No sentiment, engagement, charisma, meeting score or period-over-period delta exists in this system.**

### 5.12 Empty state (`EmptyState`)
Centred, max 420px: 40×40 `--surface-2` tile with a 20px lucide icon (never an emoji), `text-title`
headline, `text-body` `--muted-foreground` sentence, **one primary action** and at most one link.
Variants **first-run** / **filtered** / **blocked**. Every empty state ships a next action — including
🔒 `/actions`, which offers none today. The 🔒 "Choose a model in Settings" deep link is shown **once**, not
twice as today.

### 5.13 Toast (sonner)
* **One position for everything: 🔒 `bottom-center`.** The three current positions (global bottom-center,
  downloads top-right, recording notice bottom-right) collapse into one stack.
* `offset={var(--bottom-chrome) + 16}` so toasts always clear the session dock and the review bar.
* Card: `bg-popover border border-border rounded-md --elev-2 p-3`, `text-body` title + `text-caption`
  description, optional `Button size="sm" variant="outline"`, close `X`. **`richColors` off** — tone is a
  3px left bar in `--success` / `--warning` / `--destructive` / `--info`, so toasts match the token system.
  **This is a deliberate deviation from a MUST-PRESERVE entry and it owes an ADR** (ADR-K, §11.7):
  `phase1-ui-map.md` §4.8 records the shipped config as `position="bottom-center" richColors
  closeButton`. `position` and `closeButton` are preserved exactly; `richColors` is dropped because it
  paints sonner's own Tailwind palette, which no §4.4 row covers and which §11.4 rule 1 forbids
  everywhere else. Removing it is the only way toast tone can be verified against the token system —
  but it is a visible change to shipped behaviour, so it is recorded, not assumed.
* 🔒 `closeButton` stays explicitly configured: it is the only dismiss affordance for the infinite-duration
  download toasts. Download toasts keep 🔒 their dedupe id `download-${modelName}` and their durations, lose
  their double chrome, and wire the already-declared, currently-unused `onDismiss`.

### 5.14 Dialog / Sheet / AlertDialog
* Overlay `bg-[hsl(var(--overlay)/0.55)]` light, `/0.65` dark — replaces `bg-black/80` and the hand-rolled
  `bg-black bg-opacity-50` overlays in `confirmation-modal.tsx`, `SettingsModal.tsx` (×6) and
  `AnalyticsDataModal.tsx`.
* Content `bg-popover rounded-xl border --elev-2`; widths `sm 420 / md 480 / lg 560 / xl 720`.
* 🔒 **The auto-injected close keeps `<span class="sr-only">Close</span>` and no `aria-label`.** This is
  load-bearing: `AskPanel.test.tsx` fails on **any** descendant whose `aria-label` contains "close" or
  "dismiss". `AskPanel` is therefore **never** wrapped in a Dialog or Sheet, and its collapsible trigger is
  a **text trigger** ("Ask this meeting"), never an icon labelled Close.
* **AlertDialog** (new) for destructive confirmations: focus trap, Escape, `destructive` primary.
* **Sheet** powers the evidence drawer and the shortcuts sheet.

### 5.15 List rows

| Row | Height | Left | Middle | Right |
|---|---|---|---|---|
| **Meeting row** | 56 | 32px status square (`--surface-2`; `--ai-surface` when a draft waits) | title `text-body` + `text-caption` meta | review chip (`Draft` / `5 of 7 approved` / `Approved`) + `⋯` overflow |
| **Action item row** | auto, min 56 | 2px status rule | action text `text-read`, then meeting link · `SourceChip` · optional Assignee / Due chips · `AiLabel` while draft | `ReviewControls` (draft) or nothing (🔒 `/actions` is read-only) |
| **Transcript segment row** | auto | 🔒 `[12:04]` `font-mono text-micro tabular-nums`, a `<button aria-label="Play from this segment">` when `onSeekToTime` is present | text `text-read` + SpeakerChips + ConfidenceIndicator (honouring 🔒 `showConfidenceIndicator`) | — |

All rows: full-width hit area, `rounded-sm` hover `bg-muted/60`, hairline separators, inset focus ring,
actions reachable by Tab (never hover-only). 🔒 Transcript rows keep `id="segment-${id}"`, `data-index`, the
virtualizer `measureElement` ref and `VIRTUALIZATION_THRESHOLD = 10`; the jump highlight becomes a 2px
`--primary` left rule + a `bg-accent` fade over `--dur-slow` instead of `bg-yellow-100 ring-yellow-300`.

### 5.16 Progress & download (`Progress`, `DownloadCard`)
One `Progress`: 6px track `bg-surface-2`, fill `bg-primary` (`bg-destructive` on error), `rounded-full`,
🔒 `role="progressbar"` + `aria-valuenow/min/max` + `aria-label` — replacing five hand-rolled bars
(h-1.5 / h-2 / h-3, gradient-gray, blue-600) across `DownloadProgressStep`, `UpdateDialog`,
`ImportAudioDialog`, `DownloadToastContent`, `ModelDownloadProgress`, `RetranscribeDialog`.
Indeterminate = a 1.4s sweep; under reduced motion a static striped track plus the text state.
`DownloadCard`: name + size + status Badge + `142 MB / 670 MB · 4.2 MB/s · ~2 min left` in
`text-caption tabular-nums`; **Retry / Cancel as real props**, not a card-title string comparison.

### 5.17 Banner / Notice (`ui/notice.tsx`)
Replaces the six hand-rolled amber boxes and the abused `Alert variant="destructive"`.
* `rounded-md border p-3`, 16px icon, `text-body` title + `text-caption` body, optional actions;
  tones `info` / `warning` / `success` / `destructive` / `ai` / `consent`.
* **`Notice tone="ai" as="note"`** renders `<div role="note" aria-label="…">` and **accepts no `action` prop
  and no children that are controls** (children are filtered through a text-only renderer). This makes the
  Art. 50 non-dismissability guard structural instead of a code-review promise. It is the **single**
  `role="note"` marking per surface — 🔒 `ReviewRequiredBanner`
  (`aria-label="AI-generated content, human review required"`, rendered in all four `DraftSummaryView`
  states), 🔒 the `AskPanel` disclosure
  (`aria-label="You are interacting with an AI assistant; AI-generated content, human review required"`),
  🔒 `CopilotInsights`' `AiMarking` (`aria-label="AI-generated content notice"`), and 🔒 the legacy-summary
  banner (`aria-label="Legacy AI-generated summary is unverified"`).
* Other instances, copy verbatim: 🔒 `EncryptionStatusBanner` (`warning`, non-dismissable,
  `aria-live="polite"`), 🔒 `TrialBanner` (`warning`, `role="status"`), 🔒 the Action-Center provenance strip
  (`info`, inside `<section aria-label="Action provenance">`), the consent-responsibility box (`consent`).
* System notices render in **one shared `SystemNotices` slot** at the top of the content pane with a single
  8px gutter.

### 5.18 Tooltip
`bg-popover text-popover-foreground border --elev-1 rounded-sm px-2 py-1 text-caption`, 6px offset, 300ms
open delay, 0ms within a group, **opens on focus as well as hover**. **No longer brand-blue inverted**
(today every hover flashes `bg-primary`, including over the red record button). One `TooltipProvider` in
`AppShell`; the nested providers in `Sidebar` and `DraftSummaryView` are removed. A tooltip never carries
information available nowhere else — and never labels a *disabled* control (§5 shared rules).

### 5.19 Skeleton
`bg-surface-2 rounded-sm` with a 1.4s shimmer; flat under reduced motion. Composed shapes: meeting list (8
rows), report (header + 2 section cards + 6 transcript lines), settings section (3 cards), transcript spine.
Every skeleton region carries `role="status" aria-live="polite"` with an sr-only sentence
(e.g. *Loading meeting report…*). Replaces the five loading patterns and the full-screen spinner at
`meeting-details/page.tsx:342-363`.

### 5.20 Playback scrubber + chapters (`report/PlaybackBar.tsx`, `TopicsTimeline.tsx`)
* One shared ruler: a proportional chapter band sitting **directly above** the scrubber it maps to, with
  action-item diamonds at their `audio_start_time`. 🔒 Chapter labels stay verbatim first-words
  (deterministic, on-device — no Art. 50 marking needed), 🔒 the <2-chapter / <120s gate is unchanged, and
  🔒 `onJumpToSegment(segmentId, startSec)` is unchanged.
* **Playhead:** `role="slider"`, `aria-label="Playback position"`, `aria-valuemin=0`,
  `aria-valuemax={durationSec}`, `aria-valuenow`, and
  `aria-valuetext="12 minutes 4 seconds of 1 hour 24 minutes"`. Keyboard: `←`/`→` ±5s, `⇧←`/`⇧→` ±30s,
  `Home`/`End`, `Space` play/pause — `Space` is handled **on the slider element itself**, never on
  `document` (§3.4 rule 3), so it can never reach the capture pause path.
* 🔒 `PlaybackBar` still renders `null` (never an error) outside Tauri, without a folder, or on load failure.
* A **"Follow playhead"** chip re-arms transcript auto-scroll after the user has scrolled away.

### 5.21 Level meter (`AudioLevelMeter`)
12 segments, `--meter` → `--meter-hot` at ≥ −6 dBFS → `--meter-clip` at 0, track `--meter-track`, fed by the
real 🔒 `audio-levels {timestamp, levels[]}` event and 🔒 `start/stop_audio_level_monitoring`.
Updates by `transform: scaleY()` at ~30fps — **never random**. The component exists today and is only
reachable behind a commented-out "Test Mic"; it is wired into the live device strip.
**A11y:** the meter is `aria-hidden="true"` (decorative) with a **sibling `role="status"` that announces only
signal-*presence* changes** ("Microphone signal detected" / "No microphone signal for 10 seconds"), never a
continuous stream.

---

## 6. Screens

Wireframes at ~1100×700, the default window. Rail 56 · Meetings pane 280 · content 764.

### 6.1 Home — `/` (idle)

```
┌────┬──────────────────────────────────────────────────────────────────────────────────┐
│ ▣  │  Home                                                          [ ● Record ]      │ 56
│    ├──────────────────────────────────────────────────────────────────────────────────┤
│ ▤  │  ⚠ Local data is currently stored unencrypted.  Learn more                       │ 40 (conditional)
│ ⌂▌ ├──────────────────────────────────────────────────────────────────────────────────┤
│ ☑  │   NEEDS REVIEW                                                          3        │
│    │  ┌────────────────────────────────────────────────────────────────────────────┐  │
│    │  │ ◫  Q3 planning with Acme          12 Sep · 48m     [Draft] 5 of 7 approved │  │ 56
│    │  ├────────────────────────────────────────────────────────────────────────────┤  │
│    │  │ ◫  Site visit — Nordwind          11 Sep · 22m     [Draft] not started     │  │
│    │  └────────────────────────────────────────────────────────────────────────────┘  │
│    │                                                                                  │
│    │   OPEN ACTION ITEMS                                            8      View all → │
│    │  ┌────────────────────────────────────────────────────────────────────────────┐  │
│    │  │▌ Send the revised pricing deck to Acme by Friday                           │  │
│    │  │  ✦ AI-generated · review required   Q3 planning · Source · 12:04           │  │ 68
│ ●  │  │                                              [✓ Approve]  ✎  ✕            │  │
│ ⌕  │  ├────────────────────────────────────────────────────────────────────────────┤  │
│ ⚙  │  │▌ Book the security review with the platform team                           │  │
│    │  │  [Approved] · Weekly sync · Source · 04:31                                 │  │
│v1.0│  └────────────────────────────────────────────────────────────────────────────┘  │
└────┴──────────────────────────────────────────────────────────────────────────────────┘
```

**Regions.** Header: page title + the *only* primary button on the screen. The shared `SystemNotices` slot
directly beneath. Then three stacked sections, content max-width 960 centred, gutter 24.
1. **Needs review** — the queue that makes Mityu a review tool rather than a recorder.
2. **Open action items** — fed by 🔒 `summaryDraftService.getOpenActionItems(8)`, now rendered with
   **`AiLabel` + `SourceChip` + inline Approve/Edit/Reject** for draft items. This closes the audit's
   highest-severity finding: today these AI-extracted items (which include `status:'draft'`) appear with no
   Art. 50 marking and no source link, conveying status only through a 2px dot with a `title`.
3. **Recent** — date-grouped meeting rows.

**Data honesty.** 🔒 `api_get_meetings` returns only `{id,title}`. Until it carries `created_at` and
`duration`, the group header is **"All"** and the meta line **omits the date** — the dashboard must never
regex-parse a date out of a meeting title again (`HomeDashboard.tsx:21-32`). The same rule applies to any
tile: a metric with no backing command is **not drawn**. (On-disk storage size therefore lives in
Settings → Storage, next to the erasure disclosure, until a real command exists.)

**States.** *Empty (first run):* one `EmptyState` — "Nothing recorded yet" → *Record your first meeting*,
plus a **"Try the sample report"** card (this replaces the tour's automatic router hijack).
*Loading:* list skeletons with the section headers already visible.
*Error (action items):* the section renders an inline `Notice tone="destructive"` with **Retry**, and the
rest of Home still renders. A HITL queue never fails silently — today the failure is invisible.

### 6.2 Live session — `/` (recording)

```
┌────┬──────────────────────────────────────────────────────────────────────────────────┐
│ ▣  │  Q3 planning with Acme  ✎                          ● Recording · 12:34           │ 56
│    ├──────────────────────────────────────────────────────────────────────────────────┤
│ ▤  │  🎙 MacBook Mic ▁▃▅▃▁   🔊 Display Audio ▁▁▂▁▁   48 kHz · Parakeet   Lang: Auto  │ 44
│ ⌂  ├──────────────────────────────────────────────────────────────────────────────────┤
│ ☑  │  ✓ Participants informed · 14:32 — Consent acknowledged on this device           │ 28
│    ├──────────────────────────────────────────────────────────────────────────────────┤
│    │   12:02  Okay, let's lock the Q3 launch. I'd rather slip a week than ship the     │
│    │          old onboarding.                                                         │
│    │   12:20  Design can't finish until the final copy lands — Friday is the real      │
│    │          deadline for that.                                                      │
│    │   ⟳ Listening…                                                                   │
│ ▣  │                                                                                  │
│ ⌕  ├──────────────────────────────────────────────────────────────────────────────────┤
│ ⚙  │  ● Recording · 12:34                                     [ ⏸ Pause ]  [ ■ Stop ] │ 40 (dock, z-40)
└────┴──────────────────────────────────────────────────────────────────────────────────┘
```

**Regions.** Header: the **editable meeting title** (the one place it can be named before it is saved) plus
the recording pill. Device strip: mic and system chips each with a **real** `AudioLevelMeter`, plus
**"48 kHz · Parakeet"** — naming the engine and sample rate doing the work on *this* machine turns the
local-first claim into visible evidence. Consent receipt: a one-line confirmation that the gate was passed
on this device. Transcript stream at the 720px measure. Dock: elapsed + Pause + Stop.

🔒 Device naming stays **Microphone** / **System Audio**, never input/output.

**States.** *Starting:* dock shows "Initializing recording…" with Stop disabled (🔒 Stop is never disabled
once recording is live). *Paused:* square `--warning` glyph + 🔒 `Paused • 12:34`, meters grey, Resume
replaces Pause. *No speech yet:* 🔒 `Listening for speech...` / `Speak to see live transcription`.
*Device error:* an inline `Notice tone="destructive"` under the device strip keeping the existing titles
(Microphone Not Available / System Audio Not Available / Permission Required / Recording Failed), each with
a real next action, and **copy rewritten to the actual mechanisms** — ScreenCaptureKit / Core Audio tap
(macOS) and WASAPI loopback (Windows). **BlackHole and "Audio MIDI Setup" are removed everywhere**
(`RecordingControls.tsx:117`, `PermissionWarning.tsx:126-128`, `TranscriptPanel.tsx:93`) — CLAUDE.md §4 says
no virtual device is required. On Linux a `Notice tone="warning"` states that system-audio capture is
unavailable (ADR-0022) instead of silently hiding the warning.

**Wrap-up (stopping → saving → completed).** The same surface, transcript dimmed, dock replaced by:

```
  ✓ Finalizing transcript      ⟳ Saving meeting        ○ Ready
  Processing 3 remaining chunks…                  [ Open report ]  [ Record another ]
```

Driven by 🔒 `RecordingStateContext.statusMessage`, which is written in four places today and rendered
nowhere. The step becomes determinate when `chunks_in_queue` is known. This makes the up-to-64s
(60s poll + 4s grace + 0.5s) wait legible instead of a black box. 🔒 The success toast
(`Recording saved successfully!` · `N transcript segments saved.` · `View Meeting`) is unchanged; the
auto-navigate now fires only from `/` (§0.3 item 6, ADR required).

**vs today.** The `fixed bottom-12` pill inside a doubled card, the `fixed bottom-4` status overlays and the
inline `marginLeft: '4rem'|'16rem'` mirrors are gone; elapsed time moves out of the scrolling region into
the chrome; the title is visible and editable; the `Math.random()` waveform (`app/page.tsx:175-189`) is
deleted; 🔒 `showConfidenceIndicator` from `ConfigContext` is honoured instead of hardcoded `true`.

### 6.3 Meeting Report — `/meeting-details?id=…`

```
┌────┬─────────────────────┬────────────────────────────────────────────────────────────┐
│ ▣  │ Meetings        ⌕   │  ‹ MEETING REPORT                                          │ 56
│    │ ┌─────────────────┐ │  Q3 planning with Acme  ✎   12 Sep · 48m · Draft           │
│ ▤▌ │ │⌕ Search evidence│ │                             [Transcript] [Export ▾]        │
│ ⌂  │ └─────────────────┘ ├────────────────────────────────────────────────────────────┤
│ ☑  │  TODAY              │  ✦ AI-generated · review required                          │ role=note
│    │ ▸ Q3 planning  ◫    │    Every line below is a draft linked to its transcript     │
│    │   Weekly sync       │    source. Nothing is final until you approve it.           │
│    │  THIS WEEK          ├────────────────────────────────────────────────────────────┤
│    │   Site visit   ◫    │  ⏱ DURATION  ▤ WORDS   # SEGMENTS  ☑ ACTIONS  ✓ APPROVED   │
│    │   Vendor call       │  48m         9,312     146         3          3 / 7        │
│    │  EARLIER            ├────────────────────────────────────────────────────────────┤
│    │   Kickoff           │  ▐Pricing▐Security review▐Data residency▐Next steps▐       │
│ ●  │                     │  ▶ ──────●────────────────────────────  12:04 / 48:12      │
│ ⌕  │                     ├────────────────────────────────────────────────────────────┤
│ ⚙  │                     │  ┌ ✦ Summary ───────── AI-generated · review required ───┐ │
│    │                     │  │▌ The team aligned on Q3 pricing…                      │ │
│v1.0│                     │  │  [Draft]  Source · 08:12        [✓ Approve]  ✎  ✕     │ │
└────┴─────────────────────┴────────────────────────────────────────────────────────────┘
       sticky review bar:  │  3 of 7 approved  ▬▬▬▬▬░░░░░       [ ✓ Approve summary ]   │ 48
```

**Region by region.**
1. **Sticky header** — inline-editable `h1` wired to the already-existing 🔒
   `useMeetingData.handleTitleChange` / `handleSaveMeetingTitle`; meta `date · duration · status`;
   `Transcript` (opens the drawer), 🔒 `Export ▾` (disabled until approved, with the visible reason),
   `⋯` (Rename, Open recording folder, Regenerate, Delete).
2. **The single Art. 50 note** — `Notice tone="ai" as="note"`, full width, non-dismissable, in **every**
   branch including loading, empty and error. The Ask disclosure lives inside the Ask card far below, so
   **two amber banners never stack again** (today `AskPanel.tsx:60-81` then `DraftSummaryView.tsx:139-156`).
3. **Stat tiles** — Duration / Words / Segments / Action items / **Approved 3 / 7**. `actionItemCount` is
   finally passed from `draftResponse.action_items` (the tile never renders today). The duplicated
   "segments" in the meta line is removed.
4. **Chapters + playback** (§5.20) — proportional band directly above the scrubber, keyboard-operable
   playhead.
5. **Section cards** — Summary → Key points / Decisions → Action items, one column at the 720px measure.
   Each card carries `AiLabel`; each row is a `BlockRow` (§5.10).
6. **Speakers & talk time** — **in the primary column, not the drawer.** 🔒 ADR-0034 copy verbatim
   (`not a measure of contribution`, `share of the speech`, `more than 100%`, `best-effort estimate`,
   `has not been measured`, `no way to rename`, `biometric`), 🔒 speakers in order of **first speech**,
   never by duration, 🔒 no textbox and no name/rename/assign control, `--chart-1…6` swatches. The caveat is
   broken into three scannable sentences — **same words, same test assertions** — and is never placed behind
   an info button.
7. **Ask this meeting** — a collapsible card near the bottom whose trigger is the **text** "Ask this
   meeting". 🔒 Its `role="note"` disclosure renders unconditionally above the input; 🔒 no descendant
   anywhere in the panel may carry an `aria-label` containing "close" or "dismiss"; 🔒 outcomes render as
   answers, not errors (`leaves it unanswered` with **no** `role="alert"`; `discarded rather than shown`
   with the dropped text never rendered).
8. **Transcript** — not inline. The header's `Transcript` button and every `SourceChip` open the right
   **evidence drawer** (400px, `z-70`, Sheet visuals but **`modal={false}`**: focus is **not** trapped, the
   report stays interactive, `Esc` closes it and returns focus to the chip that opened it). 🔒 Both panes
   stay mounted so BlockNote content, in-flight edit/reject fields, optimistic draft state and transcript
   scroll survive; 🔒 jump-to-source relies on `VirtualizedTranscriptView` staying mounted to retry after
   `onRequestSegment` pagination.
   🔒 **Tour reveal:** when `activeAnchor === TOUR_ANCHORS.transcriptPanel` the drawer **opens**, mirroring
   `page-content.tsx:93-103` — the anchored element must be *visible*, not merely present, or step 1
   degrades to the centred fallback.
9. **Review bar** — §5.10, sticky at the bottom of the column. 🔒
   `data-tour="summary-approve-block"` stays on the **first** draft block only.

**States.** *Loading:* report skeleton, not a full-screen spinner. *No summary:* `EmptyState` inside the
Summary card — "No summary yet / Drafts stay on this device until you review and approve them" +
**Generate summary** (`default`, ink), with the model deep-link shown once. *Generating:* indeterminate
`Progress` + `Stop`. *Error:* `Notice tone="destructive"` with Retry (`window.alert` → toast).
*Legacy summary:* 🔒 `role="note" aria-label="Legacy AI-generated summary is unverified"`, text
`Legacy AI-generated summary · unverified`, action `Regenerate with sources`, editing disabled.
*Approved:* status chip flips to **Approved**, controls lock, Export enables.
*Narrow (<900px):* the drawer becomes a full-width sheet; both panes stay mounted.

**vs today.** A transcript-primary split pane with a 640px-capped collapsible summary becomes a report
document with an evidence drawer; the "context for the AI" textarea moves out of the transcript footer into
the **Generate ▾** popover with template, language and model; icon-only toolbars gain labels; BlockNote's
hardcoded `theme="light"` follows the app theme; the report stops 🔒 writing model config to the DB as a
side effect of being viewed (`meeting-details/page.tsx:98-115`); `/design/report`'s four private copies of
`StatTile`/`AiLabel`/`SourceChip`/`SectionCard` are deleted in favour of the real primitives.

### 6.4 Actions — `/actions`

```
┌────┬──────────────────────────────────────────────────────────────────────────────────┐
│ ▣  │  Actions                                       ⌕ Filter actions…    18 approved  │ 56
│ ▤  ├──────────────────────────────────────────────────────────────────────────────────┤
│ ⌂  │  🛡 AI-extracted · human approved                                                 │ info
│ ☑▌ │     This view is read-only. Each action links to its transcript source so you     │
│    │     can verify it; approval is separate from future work-progress tracking.       │
│    ├──────────────────────────────────────────────────────────────────────────────────┤
│    │   Q3 PLANNING WITH ACME · 12 Sep 2026                                             │
│    │  ┌────────────────────────────────────────────────────────────────────────────┐  │
│    │  │▌ Send the revised pricing deck to Acme by Friday                           │  │
│ ●  │  │  👤 Ada  📅 Fri                              ◷ Source · 12:04           ↗  │  │
│ ⌕  │  └────────────────────────────────────────────────────────────────────────────┘  │
│ ⚙  │   WEEKLY SYNC · 10 Sep 2026                                                       │
│    │  ┌────────────────────────────────────────────────────────────────────────────┐  │
│v1.0│  │▌ Draft the JD for the two engineering roles   ◷ Source · 04:31          ↗  │  │
└────┴────────────────────────────────  [ Load more ]  ────────────────────────────────┘
```

**Regions.** Header with a client-side filter field (non-mutating — allowed under 🔒 ADR-0025, which forbids
any work-state / complete / overdue affordance). The provenance strip becomes `Notice tone="info"` inside
🔒 `<section aria-label="Action provenance">` with its copy verbatim. Rows group under sticky meeting
headers, which removes the redundant per-card "Approved" pill from an approved-only list. Action text leads
at `text-read`; assignee / due / time are the secondary line.

**States.** 🔒 *Loading* `role="status"` `Loading approved actions…` + skeleton rows. 🔒 *First-load error*
`role="alert"` `Unable to load Action Center` + `Approved actions could not be loaded from local storage.`
+ Retry. 🔒 *Empty* `role="status"` `No approved actions yet` + the explanation **+ a next action**
("Open the sample report"). *Load-more error:* an inline Notice, and **`Load more` stays visible** (today it
hides behind the error). 🔒 `<ul aria-label="Approved actions">`, backend order, id-dedupe and the
🔒 `ACTION_CENTER_PAGE_SIZE = 100` pagination contract are unchanged.
🔒 **No analytics may be introduced on this route.**

**vs today.** 72 raw grey/blue utilities and **zero** `dark:` variants — a light island inside a dark app —
become fully tokenized.

### 6.5 Settings — `/settings?section=…`

Ten sections, each `?section=` addressable so every 🔒 "Choose a model in Settings" hand-off becomes a real
deep link: `general` · `appearance` · `recording` · `transcription` · `summary` · `privacy` · `learning` ·
`beta` · `license` · `about`. The section list is a 200px column **inside** the content pane, so the
Meetings pane never appears or disappears as a side effect of navigating here.

**Consolidations.** One Notifications row with two sub-options backed by both stores; one recordings-folder
card; one analytics switch (About links to it instead of mounting a second copy); one model surface —
`ModelSettingsModal` (1387 lines, ~40 `useState`) decomposes into `ProviderPicker` + per-provider panels
sharing one `ApiKeyField`, rendered inline in Settings and as a thin dialog from the report toolbar via its
existing 🔒 `layout` prop. 🔒 The `ModelConfig` type its five importers use is unchanged.

**Storage (`general`).** The erasure disclosure stays verbatim, and gains the concrete local-first proof:
**on-disk size (audio + database)** rendered beside it — but only once a real Tauri command backs it.
Until then the row is omitted, never faked.

**Appearance.** 🔒 `ThemeToggle` keeps `role="radiogroup" aria-label="Theme"` with `role="radio"
aria-checked` System/Light/Dark and its mounted guard. It additionally calls
`getCurrentWindow().setTheme(...)` (guarded by `isTauri()`, with the matching
`core:window:allow-set-theme` capability) so the **native titlebar follows the app at runtime**, not just at
launch.

**States.** *Loading* section skeleton. *Saving* — optimistic autosave with a 1.2s `Saved` marker in the
card header; the one explicit Save (model config) gets a persistent unsaved-changes footer instead of a
silently disabled button. *Error* → `Notice tone="warning"` + Retry.
*Dead ends removed:* Beta's empty `featureOrder` loop; the unreachable cloud-provider API-key branch.

🔒 **Kept exactly:** every switch label and `aria-label` (Learning ×3, Copilot ×3, Analytics), the two-step
analytics opt-**out** with its full disclosure, the analytics default-**OFF** and the ~60
`Analytics.track*` identifiers (relocating a control moves its `trackButtonClick` identifier — §4.7 of the
register lists every site), the Beta DOM order (`Beta Features` **before** the copilot switch), the Modes
editor contract, the Redaction guarantees, the Learning headline that states *what happened, never why*,
and every Enter/Escape handler.

### 6.6 Onboarding — full-screen, `z-100`

Four steps (three on non-macOS). The progress rail renders on **every** step (today Welcome and Permissions
hide it, and the 4th step has no icon); step icons are data-driven for 3 or 4 steps; a persisted
🔒 `current_step === 4` on non-macOS falls back to 3 instead of a blank screen.

**States.** *Downloading* — Continue **keeps its label**: "Continue — transcription still downloading",
disabled with a visible reason (today it becomes a bare spinner). *Parakeet ready* — Continue enables.
*Error* — per-card `Notice tone="destructive"` + Retry wired through a real `onRetry` **prop**, not a
card-title string match. *Permissions* — **per-row** pending state (today one `isPending` flag disables both
rows) and an inline themed message instead of `alert()`. *Completion* — calls the already-wired
`onComplete`; **no `window.location.reload()`** (which today masks missing refetches in SidebarProvider,
ConfigProvider, LicensingProvider and EncryptionStatusBanner — those refetches are made explicit in WP13).

🔒 Preserved: the local-first Welcome claims ("Mityu transcribes and summarizes on this computer. Nothing is
uploaded."), the skip link `I'll do this later`, the hint `Recording won't work without permissions. You can
grant them later in settings.`, permissions set **only** after explicit user action, the
`save_onboarding_status_cmd` payload shape, `PARAKEET_MODEL`, the `isCompletingRef` race guard, and the rule
that the tour, the rail, the trial banner and the encryption banner never mount during onboarding.

### 6.7 Copilot window — 380×520, frameless

Width 360 → **380** (Rust `MIN_PANEL_WIDTH` updated). 🔒 `data-tauri-drag-region` stays on the header;
🔒 the close button keeps `aria-label="Close the copilot panel"`; 🔒 the transcript `<li>` textContent stays
**exactly** the line (no speaker prefix, no timestamp); 🔒 `AiMarking` renders in every InsightState phase
with `role="note" aria-label="AI-generated content notice"` and no prop can suppress it; 🔒 the disclosure
region, its acknowledgment button and its `localStorage['mityu.copilot.aiDisclosureAcknowledged']` key are
unchanged, **including that a storage failure means it is shown again**; 🔒 the four action labels and their
order; 🔒 the protection headline verbatim from the backend; 🔒 no button matching `/start|record/i`.

Changes: every `text-[10px]` rises — compliance strings to `text-caption` (12px), chrome to `text-micro`;
the pin target goes from ~20px to 28px; `ProtectionChip` becomes a focusable Popover so `protection.detail`
reaches keyboard and touch; the insights region becomes collapsible but never removable; the footer copy is
reconciled (it no longer claims "Nothing it offers is saved" beside pinned notes being written into the
summary); `app/copilot/page.tsx`'s raw `invoke('is_recording_paused')` moves behind `copilotService`.

### 6.8 Key dialogs

**Recording consent** (480px) — 🔒 title `Before you record`; 🔒 the description paragraph; a
`Notice tone="consent"` carrying 🔒 `You are responsible for participant consent` + the jurisdiction
paragraph; a real `Checkbox` 🔒 `Don't show this again on this device` (🔒 reset on every open); footer
🔒 `Cancel` + 🔒 `I confirm participants are informed — start recording` as **`variant="verified"`** —
red is the *indicator* of recording, not the semantics of this action, and today's `bg-red-600` reads as
destructive. 🔒 X / Esc / outside-click all call `onCancel`; a dismissal must never start a recording.
Adds one `text-caption` line: "You can change this in Settings → Privacy & consent."

**Delete meeting** — `AlertDialog`, 480px. Leads with the meeting's name in **one sentence**:
*Delete "Q3 planning with Acme"? This removes the meeting, its transcript, recording and search index from
this device. It cannot be undone.* The full 🔒 ADR-0026 disclosure (Mityu-managed DB/search/recording/
recovery-cache removal, unknown files retained, SSD wear-levelling, copy-on-write filesystems, snapshots,
backups, exports and WebView storage) moves **verbatim** into a `<details>` labelled **"What exactly is
removed"**. 🔒 `aria-labelledby="delete-confirmation-title"`, `aria-busy`, `Cancel`/`Delete` → `Deleting…`,
and the 🔒 purge of IndexedDB + `sessionStorage indexeddb_current_meeting_id` **before** `api_delete_meeting`
are unchanged.

**Import audio** (560px) — a real drop zone, a file card with a title field and an **Advanced** disclosure
(language, model). 🔒 Gated on `betaFeatures.importAndRetranscribe`; 🔒 the licence gate
(`isLicenseRequiredError` → close + `openActivateDialog({paywall:true})`); 🔒 cannot close while processing;
🔒 Parakeet forces `auto`; 🔒 the format list; 🔒 the drop overlay stays `pointer-events-none`.
The error state **keeps the chosen file visible** and "Try again" retries the same file.

**Recovery** (720px) — 🔒 `Recover Interrupted Meetings`, the `Interrupted Meetings` list, the first-10-
segments preview, `Cancel`/`Delete`/`Recover`, once per session via `recovery_dialog_shown`, skipped while
recording/stopping/processing/saving. `confirm()`/`alert()` → `AlertDialog` + toast.

**Update** — 🔒 cannot be closed while downloading; 🔒 the three states and three button labels; 🔒 the
post-install toast → `relaunch()`.

**Model download** — no dialog: the shared toast card (deduped, dismissible) plus a `DownloadCard` in
Settings.

---

## 7. Motion & micro-interactions

| # | Interaction | Spec |
|---|---|---|
| 1 | Button / row hover, focus ring | `background-color`, `border-color`, `box-shadow` `var(--dur-instant)` `var(--ease-out)` |
| 2 | HITL Edit/Reject reveal on row hover/focus | `opacity` + 2px `translateX` `var(--dur-fast)`; **instant when focus-driven** — keyboard users get no delay |
| 3 | Status chip change (Draft → Approved) | chip cross-fades `var(--dur-fast)`; the row's status rule turns `--ai`→`--verified`; an `aria-live="polite"` message announces it; **no scale, no bounce, no confetti** |
| 4 | Tab / section indicator | 2px bar translates + resizes `var(--dur-base)` via CSS custom properties (not `offsetLeft` measurement) |
| 5 | Popover / dropdown / select / tooltip | `opacity 0→1`, `scale .98→1`, `translateY 2px→0` `var(--dur-base)` — tailwindcss-animate `data-[state]` variants |
| 6 | Dialog | overlay `opacity`; content `opacity` + `scale .97→1` + `translateY 4px→0` `var(--dur-base)` |
| 7 | Evidence drawer | `translateX 100%→0` `var(--dur-slow)` `var(--ease-emphasis)` |
| 8 | Meetings-pane collapse | `width` + content `opacity` `var(--dur-slow)`; content unmounts only after the transition ends |
| 9 | Jump-to-source landing | target row `bg-accent` in at `var(--dur-fast)`, out over 1.6s, plus a persistent 2px `--primary` left rule while it is the drawer's current segment; `scrollIntoView({behavior:'smooth'})` → `'auto'` under reduced motion; **under reduced motion the highlight does not fade — it holds for 4s** |
| 10 | Recording dot | 1.6s pulse `opacity .55↔1`; reduced motion → solid dot **and the text carries the state** |
| 11 | Level meters | `transform: scaleY()` at ~30fps from real `audio-levels` payloads, 90ms linear — fast enough to feel live, slow enough not to strobe; **never random** |
| 12 | Playhead | position transitions 90ms while playing, **instant while dragging** |
| 13 | Transcript segment arrival | `opacity 0→1` + `translateY 4px→0` `var(--dur-fast)`, only for the newest segment, only when the list is at the bottom |
| 14 | Progress fill | `width` `var(--dur-base)`; indeterminate = 1.4s sweep, static striped track under reduced motion |
| 15 | Toast | enter `translateY 8px→0` + fade `var(--dur-base)`; exit `var(--dur-fast)` |
| 16 | Skeleton shimmer | 1.4s sweep; flat `--surface-2` under reduced motion |
| 17 | Tour spotlight + coach mark | spotlight `box-shadow` grows `var(--dur-slow)`; dim colour `hsl(var(--overlay)/0.6)` replacing the `rgba(2,6,23,.6)` literal; 🔒 `mityu-tour-pulse` kept, token-driven, **static** under reduced motion |
| 18 | Route change | `opacity 0→1` only. **No y-translate** — it fights the sticky header and the fixed bottom chrome |
| 19 | Theme switch | 🔒 `disableTransitionOnChange` stays — instant swap, no colour crossfade |
| 20 | Approve-summary success | the review bar's progress fills to 100% `var(--dur-base)`, then the bar collapses `var(--dur-slow)`; no celebration |

The `vibrate` keyframe is **deleted** (SC 2.3.3 vestibular trigger).

---

## 8. Accessibility

**Landmarks.** `<nav aria-label="Primary">` (rail) · `<aside aria-label="Meetings">` (pane) · `<main>` with
exactly one `<h1>` per screen · `<aside aria-label="Transcript">` (evidence drawer, non-modal) ·
`<footer role="status" aria-label="Recording status">` (session dock) · toasts in sonner's live region.
`/copilot` has its own single `<main>`.

**Focus order.** Skip link ("Skip to content") first · rail · Meetings pane search · pane rows · page header
title · header actions · system notices · content sections **in reading order** · review bar · session dock.
**DOM order matches visual order everywhere** (SC 1.3.2) — on the Report the drawer is an aside adjacent to
the content it annotates, and a "Skip to transcript" link is provided rather than inverting the source
order. Dialogs and sheets trap focus (Radix) and return it to the trigger. The evidence drawer is
`modal={false}`: it does **not** trap focus, `Esc` closes it and focus returns to the `SourceChip` that
opened it.

**ARIA on custom controls.**
* Rail items — `aria-current="page"`; the Meetings toggle — `aria-expanded` + `aria-controls`; the recording
  chip — `role="status"`, `aria-label="Recording, 12 minutes 34 seconds"`.
* `SourceChip` — `aria-expanded` + `aria-controls="transcript-drawer"`, 🔒 existing `aria-label`s verbatim.
* Art. 50 markings — 🔒 exactly the three frozen `role="note"` names, one per surface, each with no
  descendant `button`, `[role=button]`, `input`, `aria-hidden` or `hidden`. The per-card `AiLabel` is a
  plain `<span>` (§0.3 item 1).
* Review controls — 🔒 the eleven pinned names; `aria-pressed` on toggles (Original, Pin); `aria-busy` on a
  pending row; state changes announced `aria-live="polite"`.
* Playhead — `role="slider"` with `aria-valuetext` (§5.20). Chapter/marker lanes — `role="list"` /
  `role="listitem"`, each a `<button aria-label="Chapter: Security review, starts at 36:12">`.
* Level meters — `aria-hidden` + a sibling `role="status"` announcing **presence changes only** (§5.21).
* Progress — 🔒 `role="progressbar"` + `aria-valuenow/min/max` + `aria-label` everywhere (today: nowhere).
* Model cards — `role="radiogroup"` / `role="radio"` with `aria-checked` (today `div onClick`, unreachable
  by keyboard, with hover-only delete).
* Onboarding step dots — `<button aria-label="Go to step 2: Setup" aria-current>`.
* Stat tiles — `<dl>`. `ConfidenceIndicator` keeps 🔒 `aria-label="Transcription confidence: N%"` and loses
  `role="status"` (it is not a live region).
* 🔒 Refusals are **never** `role="alert"`.
* **Status is never conveyed by colour alone**: every chip carries a word and an icon, every dot has an
  adjacent label, rejected text is also `line-through`, paused is also a square glyph, series carry labels.
* No `data-testid` anywhere (the suites query by role + accessible name); no `title=` as the only label.

**Contrast.** §4.4. Body AAA in both themes; every status ink ≥4.5 on its surface; every meaningful non-text
boundary ≥3:1; focus ring 6.61 / 6.41 against its offset.

**Hit targets.** 40×40 for rail items and the record control; 36px for labelled row actions (Approve);
32×32 for toolbar/row icon buttons with 8px spacing; 28×28 only inside the 380px copilot panel. Full-row
click areas for list items. Today's 24×24 `p-1` sidebar edit/delete buttons — adjacent with `gap-1`, so the
SC 2.5.8 spacing exception does not rescue them — are raised to 32.

**Pointer coarse.** `@media (pointer: coarse) { .group [data-reveal] { opacity: 1 } }` promotes **every**
hover-revealed control to always visible — one rule, the whole defect class (HITL cluster, meeting-row
rename/delete, transcript actions) closed for touch and pen.

**Reduced motion.** §4.9, with semantic fallbacks so no state is communicated by animation alone.

**Zoom.** The layout survives 200% browser zoom at 1100px (the evidence drawer becomes a full-width sheet).

**Keyboard.** §3.4. Tab reaches every row action (nothing hover-only); Esc cancels an inline edit without
losing the draft; arrow keys move within Radix groups; `?` is the discoverability surface. No document-level
`keydown` handler is registered by any editor — the legacy `AISummary` global `Cmd+Z`/`Cmd+C` hijack is
removed with that dead branch.

**Native chrome.** `tauri.conf.json` drops `"theme": "Light"` (today a light titlebar over a near-black app)
and gains `minWidth: 900, minHeight: 600`. `ThemeToggle` additionally calls `setTheme` at runtime (§6.5).

---

## 9. Implementation map

Sixteen work packages, foundation → shell → screens → fixtures/docs, each sized for one engineer in one
sitting and each independently verifiable. The machine-readable plan, with per-package files, dependencies,
acceptance checks, fixture routes and `--expect` markers, is
**`scratchpad/phase3-work-packages.json`** — that file is authoritative for sequencing.

| WP | Title | Risk |
|---|---|---|
| WP1 | Token foundation: globals.css, tailwind.config.js, Inter, tailwind-merge pin | high (blast radius) |
| WP2 | shadcn primitive layer: button/input/tabs/tooltip/dialog + v3 generation unification | medium |
| WP3 | Product primitives: Notice, Badge/StatusPill, AiLabel, SourceChip, Card, StatTile, EmptyState, Progress, Skeleton | medium |
| WP4 | Shell layout: AppRail, MeetingsPane, PageHeader, SystemNotices, CSS-var widths | high |
| WP5 | Keyboard model: ⌘K palette, shortcuts sheet, context-menu fix | low |
| WP6 | RecordingSessionProvider + SessionDock (global Stop) | **highest — ships alone** |
| WP7 | Home: review queue + compliant action items | medium |
| WP8 | Live session, real meters, wrap-up pipeline, honest device copy | high (audio-adjacent) |
| WP9 | Report shell: document layout, stat tiles, scrubber, evidence drawer | high |
| WP10 | HITL controls + review bar (DraftSummaryView) | high |
| WP11 | Actions page + its first automated test | low |
| WP12 | Settings: sections, `?section=`, one model surface | high |
| WP13 | Onboarding + dialogs + toasts | medium |
| WP14 | Copilot panel | low-medium |
| WP15 | Cleanup & dead-code removal (**after** screens are proven) | medium |
| WP16 | Verification tooling, CI screenshot gate, ADRs | low |

**Migration rule that makes WP1 survivable.** New token *values* land under the **old token names**
(`--background`, `--card`, `--primary`, `--muted-foreground`, …), so every one of the ~871 existing
`bg-card` / `text-muted-foreground` call sites **improves** rather than breaks during the intermediate
state. Only additive names (`--ai*`, `--verified*`, `--recording*`, `--surface-*`, `--border-strong`,
`--subtle-foreground`, `--meter*`) are new.

**Sequencing rules.**
1. Deletions happen in **WP15**, after the screens that replace them are proven. The one exception is
   `frontend/src/components/MainNav/index.tsx` (orphan, `h-0`, zero importers), removed in **WP4** —
   which is why that exact path is listed in **WP4's `files[]`** and named in WP4's acceptance.
   Ownership is singular: WP15's acceptance no longer says "if not already gone". The `files[]`
   arrays in `scratchpad/phase3-work-packages.json` are the blast-radius declaration, so a path that
   appears only in acceptance prose is owned by nobody — WP15's full deletion inventory is therefore
   enumerated in its `files[]`, not only in its acceptance text.
2. **WP15 is the only package permitted to edit a test file**, and only
   `TranscriptPanel.diarization.test.tsx`'s `vi.mock('@/components/TranscriptView')`, in the same commit
   that deletes the file.
3. WP6 lands **alone**, with an ADR, the `recordingCompletionMailbox` / `recordingService` /
   `storageService` suites as the gate, and the CLAUDE.md §4 manual smoke test (record → transcript appears)
   on one macOS and one Windows path.
4. Nothing may add a mount-time `invoke`/`listen` to a browser-renderable component (`DraftSummaryView`,
   `AskPanel`, `SpeakerTurns`, `report/*`, `EmptyStateSummary`, the `BlockNoteSummaryView` structured path)
   or the fixtures go blank. **Never** stub `window.__TAURI_INTERNALS__`.
5. **Rust and Tauri configuration carry the CLAUDE.md §5.1–5.2 gates.** Four packages touch them —
   WP4 (`src-tauri/tauri.conf.json`), WP6 (`src-tauri/src/tray.rs`), WP12 (`tauri.conf.json`
   capabilities) and WP14 (`src-tauri/src/copilot/window.rs`) — so each of those four runs
   `cargo fmt --check`, `cargo build` and `cargo clippy --all-targets -- -D warnings` (diffed against
   `scratchpad/baseline-clippy.log`), plus a `tauri.conf.json` schema validation in WP4 and WP12.
   A frontend-only gate set (`tsc` / `lint` / `vitest` / `shoot.py`) would let a `tray.rs` edit or a
   malformed `tauri.conf.json` land with no compile evidence at all, and a bad capability array fails
   only at runtime.
6. **A package's `fixture_route` must be created by a package.** Every route in §11.1 names an owning
   WP, and that WP's `files[]` or `new_files[]` contains the route's `page.tsx`. A route named only in
   an acceptance command is an unpassable gate — `design/record` was exactly that, gating WP6, the one
   package the plan says **ships alone**.

---

## 10. Deliberately kept from today

1. **The whole HITL apparatus, string for string.** The non-dismissable `role="note"` Art. 50 markings
   (summary, Ask, copilot, legacy), the eleven review-control accessible names, the optional-reason reject
   flow, the whole-summary approval gate with its five tooltip reasons, `areBlockReviewControlsLocked`, the
   optimistic-with-revert semantics, and the approved-only export with its excluded-items disclosure and
   fail-closed `ExportApprovalError`. This is the moat and it is already correct; the redesign re-houses it.
2. **Source linkage as a system.** `source_chunk_id` → `?segment=&jump=` deep links, the re-scroll nonce,
   `onRequestSegment` paging, chapter click → jump + seek, Ask claim → source, Action Center → Source.
   The chip changes; the plumbing does not.
3. **The consent and trust chain.** `ensureRecordingConsent()` before every start path, the one-time consent
   ticket → `start_recording_with_devices_and_meeting`, the `stop_recording` ownership boolean, the
   completion-token mailbox claimed **before** save, `recording-consent.json` and `computeConsentGate`'s
   fail-open behaviour, the re-armable Settings toggle, the encryption banner's render condition and copy,
   analytics default-OFF with its two-step opt-out and full data inventory.
4. **Honest AI states.** "No evidence" and "refused" rendered as answers rather than errors; dropped-claim
   counts disclosed and dropped text never shown; diarization failure shown with a retry; the dashboard
   refusing to fabricate a metric it has no data for.
5. **The ADR-0034 speaker ethics boundary** — anonymous speakers in order of first speech, the full caveat
   verbatim and adjacent to the numbers, no rename or assign control anywhere, no ranking colours, no
   sorting by duration.
6. **The route and event contracts.** `/`, `/settings`, `/meeting-details?id=`, `/actions`, `/copilot` as a
   BARE_ROUTE; `sessionStorage.autoStartRecording`; `start-recording-from-sidebar` and
   `check-updates-from-tray`; `window.handleRecordingStop`; every Tauri `listen()`/`emit()` name;
   `model-config-updated`. Rust evaluates JS against these names and renaming produces no compile error.
7. **The tour.** `TOUR_ANCHORS`, `tourAnchorSelector`, `mityu-tour-pulse`, the verbatim `WELCOME_COPY` and
   three `TOUR_STEPS`, the coach-mark's "target hidden → centred, Back/Next still work" contract, the
   `getRoot` prop and `/design/tour`'s URL API. Only the automatic sample-meeting navigation is removed.
8. **The `/design/*` fixture pattern.** Rendering real components with injected data and no
   `__TAURI_INTERNALS__` stub is the only way this app can be seen outside Tauri. The redesign **expands**
   it (primitives, shell, home, record, actions, settings, dialogs) and makes it CI-enforced.
9. **The semantic-token architecture itself.** HSL triplets consumed as `hsl(var(--x))`, `.dark` as a class,
   shadcn names. The values change completely; the mechanism is right and every `/opacity` modifier depends
   on it.
10. **`#1E56FF`.** The bluedev brand colour is unchanged. What changes is its *meaning and dose*: from a
    flood of blue tiles, chips, tooltips and labels down to one filled action per viewport plus the rule that
    marks what a human approved — which is what makes it read as a decision rather than as decoration.
11. **One editor, one toaster, one icon set.** BlockNote, sonner, lucide, framer-motion for the few things
    CSS cannot do. No new UI dependency; `tailwind-merge` is *down*graded to match Tailwind 3.4;
    `@heroicons/react` is removed (one import, `AISummary/index.tsx:7`).
12. **The copy that is already good** — the Action Center provenance note, the Ask panel's refusals, the
    consent dialog's local-processing claims, the recovery toasts, the storage/erasure disclosure, the
    licensing "your data is never locked" promise, the Learning headline that states what happened and never
    why. Where copy changes it is because it was **wrong** (BlackHole, "Nothing it offers is saved", a UTC
    stamp printed as a clock time) or duplicated — never because it was unfashionable.

---

## 11. Verification plan

CI today runs `pnpm run lint` · `pnpm tsc --noEmit` · `pnpm test` · the landing test. **No build, no
screenshot, no a11y step — nothing in the pipeline proves rendering.** The product routes cannot render in a
browser at all (they `invoke()` on mount and sit on a loading state forever), and stubbing
`window.__TAURI_INTERNALS__` to force them produces a white page. So the verification surface *is* the
`/design/*` fixture set, and this plan makes it enforceable.

### 11.1 Fixture routes that must exist

Fixtures render **real** components with injected data (the pattern `LearningSettings`' `service` prop
already establishes). They add **no** mount-time `invoke`/`listen`.

**Every route below is created or edited by the WP in its "Owning WP" column, and that WP's `files[]` /
`new_files[]` in `scratchpad/phase3-work-packages.json` contains the route's `page.tsx`.** Verified
against disk: `frontend/src/app/design/` currently holds only `copilot`, `hitl`, `learning`,
`page.tsx`, `report`, `speakers` and `tour`, so every row marked **new** is a file some package must
actually add. `design/record` previously had no creator at all while two packages gated on shooting
it — `shoot.py` would have requested `/design/record.html`, got a 404 and failed the 25 000-byte
check, blocking WP6, the package that ships alone.

| Route | Status | Renders | Owning WP |
|---|---|---|---|
| `design` | extend | full token matrix, all 11 type steps, radius/elevation/motion/z scales, light **and** `.dark` side by side | WP1 |
| `design/primitives` | **new** | every §5 component × every state × both themes | WP3 |
| `design/shell` | **new** | rail + Meetings pane + header + system notices at 900 / 1100 / 1440 px | WP4 |
| `design/record` | **new** | live session: idle · recording · paused · wrap-up · device-error · Linux notice; dock states | **WP6 creates** (`app/design/record/page.tsx` in WP6 `new_files`: idle / recording / paused / dock states), **WP8 extends** (wrap-up, device-error, Linux notice frames; the file is in WP8 `files[]`) |
| `design/home` | **new** | Home: populated · empty (first-run) · loading · action-items error | WP7 |
| `design/report` | rebuild | the report document on the **real** primitives (its four private copies deleted) | WP9 |
| `design/hitl` | keep | block rows: draft · approved · edited · rejected · locked · pending; `?reject=1` opens the reject field | WP10 |
| `design/speakers` | keep | `SpeakerChips` + `TalkTimePanel`, all five states | WP9 |
| `design/actions` | **new** | Action Center: populated · loading · first-load error · empty · load-more error | WP11 |
| `design/settings` | **new** | each of the ten sections; save/error states; the model surface | WP12 |
| `design/dialogs` | **new** | consent · delete · import · recovery · update · analytics, every state | WP13 |
| `design/tour` | keep | coach marks, `?tour=N` URL API | WP16 (`app/design/tour/page.tsx` is in WP16 `files[]`) |
| `design/copilot` | keep | 8 frames at **380**px (was 360) | WP14 |
| `design/learning` | keep | learned-rules card via its injected `service` | WP12 (`app/design/learning/page.tsx` is in WP12 `files[]`) |

### 11.2 `tools/ui/shoot.py --expect` markers

Invocation: `python3 tools/ui/shoot.py --build <route> --expect "<text>" [--expect …] [--theme dark]`.
The tool fails if the PNG is `< 25 000` bytes **or** any `--expect` string is missing from `--dump-dom`
(case-insensitive; the dump includes attributes, so an `aria-label` is a valid marker).

| Route | `--expect` markers |
|---|---|
| `design` | `Type scale` · `--ai-surface` · `--verified` · `Focus ring` |
| `design/primitives` | `AI-generated · review required` · `Approve` · `Source ·` · `Jump to source transcript segment` |
| `design/shell` | `Home` · `Actions` · `Record` · `Search meeting evidence` |
| `design/home` | `Needs review` · `AI-generated · review required` · `Jump to source transcript segment` |
| `design/record` | `Recording` · `Paused` · `Listening for speech` · `Finalizing transcript` · `48 kHz` |
| `design/report` | `Action items` *(documented)* · `AI-generated content, human review required` · `Source ·` · `Approve summary` |
| `design/hitl` | `Approve block` · `Reject block` · `Approve action item` |
| `design/hitl?reject=1` | `Reason for rejecting this block (optional)` · `Confirm reject` *(plus the existing `document.querySelector('button[aria-label="Reject block"]')` driver)* |
| `design/speakers` | `Speaker` *(documented)* · `talk time` *(documented)* · `not a measure of contribution` · `best-effort estimate` · `biometric` |
| `design/actions` | `AI-extracted · human approved` · `Approved actions` · `Load more` |
| `design/settings` | `Recording consent` · `Redact sensitive content` · `Usage analytics is off by default` |
| `design/dialogs` | `Before you record` · `I confirm participants are informed — start recording` · `What exactly is removed` |
| `design/copilot` | `never records on its own` · `AI-generated content notice` · `Best effort on macOS` |
| `design/tour` | `Take a 30-second tour` · `Nothing is final until you approve it` |
| `design/learning` | `Learn from my corrections` · `Summaries still need your approval either way` |

**Query-string routes need `shoot.py` support, and it lands in WP1.** `tools/ui/shoot.py:219` builds
`url = f"http://127.0.0.1:{port}/{route.lstrip('/')}.html"` — it appends `.html` to the raw route and
has no query handling, so `design/hitl?reject=1` becomes `/design/hitl?reject=1.html`, which the
static server cannot resolve. `:220` then builds the PNG name as `route.replace("/", "-") + ".png"`,
which leaves the `?` (and any `=`) in the filename. The two rows above that use a URL API
(`design/hitl?reject=1`, `design/tour?tour=N`) are called load-bearing by §5.10 and §10.7, so the
tool must be able to shoot them **before** the packages that depend on them: `shoot.py` is therefore
in **WP1's `files[]`**, where `:219-220` become

```python
path, _, query = route.partition("?")
url = f"http://127.0.0.1:{port}/{path.lstrip('/')}.html" + (f"?{query}" if query else "")
png = os.path.join(args.out_dir, re.sub(r"[^A-Za-z0-9._-]", "-", route) + ".png")
```

`shoot.py` imports `argparse http.server os shutil socket socketserver subprocess sys threading`
and **not `re`** — WP1 adds the `import re`, or the first query-route shot dies on a `NameError`
rather than on a missing marker. The module docstring's "Routes are given without the `.html`
suffix" line gains the query form, so the USAGE block does not contradict the code.

WP1 is the first package and already runs `shoot.py`, so every later package — WP10's `?reject=1`
gate and WP16's all-routes CI job — can rely on it. WP16 still owns `--theme dark`.

**`--theme dark` is a new flag (WP16).** Theme is a **class on `<html>`**, so `--force-prefers-color-scheme`
does nothing — the flag must inject `class="dark"` before the screenshot. Every route above is shot in both
themes.

### 11.3 Vitest suites that must stay green

`pnpm test` = `vitest run`, include `['src/**/*.test.ts','src/**/*.test.tsx']`. **Zero `data-testid` exist in
the codebase** — the suites query by role, accessible name and exact copy, so markup and copy *are* the
contract. Toolchain is pinned: Node 20.19.4, pnpm 10.33.0, jsdom ^29.

**DOM-asserting (9) — these constrain markup and copy:**

| Suite | Constrains | WPs that must re-run it |
|---|---|---|
| `AISummary/DraftSummaryView.transparency.test.tsx` | the **singular** `getByRole('note', {name:/AI-generated content, human review required/i})` in all four states; zero controls in the banner subtree; renders without Tauri | WP3, WP9, WP10 |
| `AskThisMeeting/AskPanel.test.tsx` | both `role="note"` names; **no descendant `aria-label` containing "close"/"dismiss"**; `Question about this meeting`; Enter submits; noEvidence has **no** `role="alert"`; dropped text never rendered; `Open the transcript at …` count | WP2, WP3, WP9 |
| `copilot/CopilotInsights.test.tsx` | `AiMarking` in **every** phase; disclosure region + acknowledgment + `localStorage` key incl. the throw path; refusal copy; the four action labels; pin ids | WP14 |
| `copilot/CopilotPanel.test.tsx` | `appendLine` key shape; **no** `/start\|record/i` button; `Best effort on macOS` verbatim; body never `/undetectable\|invisible/i`; `<li>` textContent equality | WP14 |
| `report/SpeakerTurns.test.tsx` | all five states; the full ADR-0034 caveat; **order of first speech**; no textbox, no rename/assign button; empty `innerHTML` for `[]` | WP1, WP3, WP9 |
| `MeetingDetails/TranscriptPanel.diarization.test.tsx` | diarization failure copy + `Try again`; no invoke while recording; **mock paths** `@tauri-apps/api/core`, `@/components/VirtualizedTranscriptView`, `@/components/TranscriptView`, **relative** `./TranscriptButtonGroup` | WP8, WP9, **WP15 (may edit)** |
| `BetaSettings.test.tsx` | switch name `Enable the live copilot panel`; `Meeting modes` label; **`Beta Features` precedes the switch in DOM order** | WP12 |
| `copilot/ModesSettings.test.tsx` | built-ins get no Edit/Remove; `Name`/`Purpose`/`Voice` only; Save gating; one `Use this`, one `Active`; refusal in `role="alert"` | WP12 |
| `hooks/useDiarization.test.ts` | availability mapping and `run()` ordering | WP9 |

**Logic suites (16) — pin IPC payloads, copy and state machines.** All must stay green and **none may be
edited**: `draftSummaryReviewState` · `trialBannerLogic` · `useExportOperations` · `recordingConsent` ·
`exportMarkdown` · `exportModel` · `exportDocx` · `speakerTurns` · `checkout` · `recordingService` ·
`recordingCompletionMailbox` · `summaryDraftService` · `storageService` · `indexedDBService` ·
`licensingService` · `types/licensing`.
`trialBannerLogic.ts` and `draftSummaryReviewState.ts` must stay **pure** (node env) — moving the logic into
a component untests it.

**Three orphaned suites** (`tests/lib/blocknote-markdown`, `summary-language-preferences`,
`onboarding-summary-model`) are outside the vitest glob and **protect nothing today**. WP16 either brings
them into the glob or deletes them with an ADR note — it must not silently rely on them.

**New coverage added by this plan:** WP11 adds the first automated test for `/actions` (nonce guard, id
dedupe, backend ordering, `Load more` error path) — a 307-line page with zero coverage today.

### 11.4 Build-time guardrails (WP1 + WP16)

**Mechanism first.** The plan adds **no npm dependency**, so these are not a custom ESLint plugin
package. `frontend/.eslintrc.json` is six lines today (`extends: next/core-web-vitals` plus one
rule) and stays eslintrc — flat-config migration is out of scope and `next lint` must keep working.
Rules 1–4 are expressed as **`no-restricted-syntax` esquery selectors over the `className`
attribute's literal value**, added in WP1:

```jsonc
"no-restricted-syntax": ["error",
  { "selector": "JSXAttribute[name.name='className'] > Literal[value=/\\b(bg|text|border|ring|from|to|via|divide|placeholder|fill|stroke)-(gray|slate|zinc|neutral|stone|red|orange|amber|yellow|lime|green|emerald|teal|cyan|sky|blue|indigo|violet|purple|fuchsia|pink|rose)-\\d{2,3}\\b|\\b(bg|text)-(white|black)\\b|#[0-9a-fA-F]{3,8}/]",
    "message": "Rule 1: raw palette utility or hex literal in className — use a semantic token (§4.2)." },
  { "selector": "JSXAttribute[name.name='className'] > Literal[value=/text-\\[[\\d.]+(px|rem)\\]/]",
    "message": "Rule 2: arbitrary type size — the §4.5 scale is real utilities now." },
  { "selector": "JSXAttribute[name.name='className'] > Literal[value=/\\bring-ring\\b(?![\\s\\S]*ring-offset-)/]",
    "message": "Rule 3: ring-ring without a ring-offset-* sibling (§4.4.6, §5 shared rules)." },
  { "selector": "JSXAttribute[name.name='className'] > Literal[value=/\\boutline-none\\b(?![\\s\\S]*focus-visible:)/]",
    "message": "Rule 4: outline-none without a focus-visible: replacement." }
]
```

plus an `overrides` entry turning rule 1 off for `src/app/design/**`. If a selector proves awkward in
eslintrc, the fallback is a **repo-local** `eslint-plugin-local/` resolved by relative path — still no
npm dependency. What is *not* acceptable is leaving the mechanism to the implementer.

**The blind spot, stated plainly.** A literal-value selector can only see a string literal. In
`frontend/src/` there are **2 342** literal `className="…"` sites, but also **85** `className={cn(…)}`
and **95** template-literal `className={\`…\`}` sites — **180 compositions these rules cannot read**,
and the `ui/` primitives (where **13 files under `src/components/ui/` use `outline-none`**, six of
them with no replacement) are exactly where `cn()` composition is densest. So:

* Rules 1–4 are a **cheap first pass over the 93 % of call sites that are literals**, not a proof.
* **Rule 3 does not "carry SC 2.4.11".** What carries it is the `--ring`/offset arithmetic in §4.4.6
  and the `ringOffsetColor` map in §4.7; rule 3 only stops the most common way of forgetting it, and
  it is blind to every `cn()`-composed primitive.
* The compensating control is **guardrail 6 below (the dead-class / built-CSS check)**, which reads
  the *emitted* CSS and therefore sees classes however they were composed, plus WP2's explicit
  acceptance line naming the six outline-stripping primitives by file.
* WP2 additionally centralises the focus treatment in one exported constant, so the `cn()` sites
  compose a reviewed string instead of hand-writing one.

The four rules themselves:

1. **ESLint: no raw palette utilities in `className`** — `(bg|text|border|ring|from|to|via|divide|
   placeholder|fill|stroke)-(gray|slate|zinc|neutral|stone|red|orange|amber|yellow|lime|green|emerald|teal|
   cyan|sky|blue|indigo|violet|purple|fuchsia|pink|rose)-\d{2,3}`, plus `bg-white`, `bg-black`,
   `text-white`, `text-black` and `#hex` literals. Allow-listed for `src/app/design/**`.
   Baseline: `scratchpad/baseline-lint.log` — **zero new warnings** is the acceptance bar for every WP.
2. **ESLint: no arbitrary type sizes** — `text-\[\d+px\]` / `text-\[[\d.]+rem\]` are errors (the scale is
   real utilities now).
3. **ESLint: `ring-ring` requires a `ring-offset-` sibling** in the same `className` — with the
   blind spot above. The legal alternative it points at must exist: see §4.7's `ringOffsetColor`.
4. **ESLint: `outline-none` requires a `focus-visible:` sibling** in the same `className` (16 files strip
   the outline with no replacement today, including six `ui/` primitives).
5. **Tailwind `content` globs** extended with `./src/lib/**`, `./src/hooks/**`, `./src/contexts/**` —
   `lib/recordingNotification.tsx` renders JSX with Tailwind classes and is not scanned today, so its
   classes may emit **no CSS** in the static export while `tsc`, lint and vitest all pass (ADR-0037's exact
   failure class).
6. **Dead-class check** — grep every `className` token emitted by the build against the built CSS; a token
   with no rule is a failure. This is the only mechanism that catches #5, **and the only one that sees
   through `cn()`**. It is what would catch a `ring-offset-card` that never compiled.
7. **`tailwind-merge` pinned to `^2.6.0`.** The installed 3.6.0 is built for Tailwind v4 class groups while
   the project is on 3.4.19, so `cn()` can silently resolve the new `fontSize` / `shadow` / `ring` groups
   wrongly. `pnpm-lock.yaml` also resolves 2.6.1 transitively — de-duplicate.

### 11.5 CI job

WP16 adds a `ui-visual` job: `next build` → install Chromium → run `tools/ui/shoot.py` over **all 14**
`/design/*` routes with the §11.2 markers, in **both** themes, plus the two query-string routes
(`design/hitl?reject=1`, `design/tour?tour=3`) using the `?`-splitting landed in WP1, plus the
dead-class check.

**A second CI job, `rust-gates`,** runs `cargo fmt --check`, `cargo build` and
`cargo clippy --all-targets -- -D warnings` on any PR touching `frontend/src-tauri/**`, and validates
`tauri.conf.json` against the Tauri 2.6.2 schema. CLAUDE.md §5.1–5.2 already requires this; today's
pipeline (lint · `tsc` · `pnpm test` · the landing test) has no Rust step at all, so WP4, WP6, WP12
and WP14 would otherwise change Rust or Tauri configuration with zero compile evidence.

**Rollout:** the job lands **non-blocking** first and flips to **blocking after one green week**. It must
not gate the redesign on a screenshot gate that has never run.

Artefacts: `scratchpad/shots-after/*` diffed against the existing `scratchpad/shots-before/*`.

### 11.6 Manual gates (not automatable here)

1. **CLAUDE.md §4 audio smoke test** — record → transcript appears, on at least one **macOS** and one
   **Windows** path — required before merging WP6 and WP8. Linux system audio is broken (ADR-0022).
2. **Right-click QA on a production build** — the context-menu suppression is production-only, so the
   copy/paste fix in WP5 is invisible in dev.
3. **Tray paths** — Settings, record toggle, `check-updates-from-tray`, and the drag-drop import listener,
   after WP4 and WP6.
4. **First run on a clean profile** — onboarding → `onComplete` (no reload) → tour → record button pulses,
   after WP13.
5. **Tour anchor** — exactly one visible `data-tour="record-button"`, and the transcript anchor visible when
   its step is active, verified via `/design/tour?tour=3` and on the real route.
6. **Keyboard scope** — with a live recording running, press `Space` with (a) a draft row focused,
   (b) the dock focused but not its Pause button, (c) nothing focused: capture must keep running in
   all three. Then press `?` inside the reject-reason field, the Ask field and BlockNote: the
   shortcuts sheet must not open and the character must be typed. This is the manual half of §3.4's
   activation-scope rules; the automated half is WP5's acceptance.

### 11.7 ADRs this redesign owes `docs/DECISIONS.md`

Per CLAUDE.md §5.7, architecture-level decisions need an ADR before or with the WP that lands them.
Twelve, not ten — ADR-K and ADR-L cover the two places where this document knowingly changes shipped
behaviour rather than only re-housing it:

| ADR | Subject | WP |
|---|---|---|
| A | Semantic token expansion: status / `--ai` violet / `--verified` / `--recording` families; `--ring` decoupled from `--primary`; radius, elevation, motion and z scales | WP1 |
| B | Typeface change DM Sans → Inter, and the 12px compliance floor | WP1 |
| C | `tailwind-merge` downgrade to `^2.6.0` and the single shadcn generation (v3 forwardRef) | WP1, WP2 |
| D | Blue means "a human decided": ink `default` button, `verified` variant, one filled blue action per viewport | WP2 |
| E | IA: rail + Meetings **pane** (not a route), `?section=` addressable Settings, `/notes/[id]` removal and the `Sidebar` routing-heuristic fix | WP4, WP15 |
| F | `RecordingSessionProvider`: hoisting the four recording hooks to shell scope, and **auto-navigate after stop only from `/`** | **WP6** |
| G | The Report becomes one document with a non-modal evidence drawer (supersedes the split pane; `docs/DESIGN_READAI.md` visual direction marked superseded, guardrails retained) | WP9 |
| H | Reduced-motion policy and the deletion of the `vibrate` keyframe | WP1 |
| I | Dead-code removal inventory (19 modules incl. the legacy `AISummary` editor trio, `SettingsModal`, `DatabaseImport/*` whose Rust commands are not registered, and the duplicate `metadata.ts`/`.tsx`) | WP15 |
| J | Verification: `/design/*` as the contract surface, the `shoot.py` `--theme` flag and query-string support, the lint guardrails **with their `cn()` blind spot**, the `rust-gates` job, and the orphaned-suite disposition | WP1, WP16 |
| K | **sonner `richColors` off** — a deliberate deviation from MUST-PRESERVE §4.8, which records `position="bottom-center" richColors closeButton` as shipped. `position` and `closeButton` are preserved; `richColors` is replaced by a 3px tone bar in `--success`/`--warning`/`--destructive`/`--info` because it paints sonner's own palette, which no §4.4 row covers and §11.4 rule 1 forbids everywhere else (§5.13) | WP13 |
| L | **Interaction and label tokens**: `--foreground-hover/-active`, `--primary-active`, `--verified-hover/-active`, `--destructive-hover/-active`, `--recording-foreground/-hover/-active`, `--input-hover`, and the `ringOffsetColor` map — plus the two AA failures they fix (dark `--primary-hover` white label at 3.82 under `Approve summary`; white on dark `--recording` at 3.84 on the record CTA) and the re-tuned `--input` / `--subtle-foreground` / `--primary-ink` values. Folded into ADR-A's token record but called out separately because it changes shipped colour, not just names | WP1 |
