# Design direction — read.ai as reference (adapted to Mityu's constitution)

> Owner directive (2026-07-09): build Mityu's UI/UX using **read.ai (Read AI)** as the reference.
> This doc is the analysis + plan. It governs the UI refresh already underway (design tokens +
> dark mode, Slices 0a–0c) and the screen redesigns that follow.

## The one rule that shapes everything below

**Reference read.ai's *design*, not its *architecture*.** read.ai is a **cloud meeting-bot**: it
joins Zoom/Meet/Teams, streams audio to its servers, and runs server-side analytics. Mityu is the
opposite by constitution — **local-first, on-device, "not a meeting bot," HITL, privacy-first**
(`CLAUDE.md` §0.1/§0.5/§0.6/§1). So we borrow read.ai's **information architecture, report
structure, metric presentation, and polished aesthetic**, and re-map anything that needs the
cloud/bot onto Mityu's local-first + opt-in Integrations model (EPIC G, ADR-0018).

Sources: read.ai/meeting-reports, read.ai/meetings, read.ai/assistant, read.ai/meeting-tools (fetched 2026-07-09).

## What read.ai does (observed) → our call

| read.ai capability | What it is | Mityu call | Why |
|---|---|---|---|
| **Meeting Report** (summary, key takeaways, decisions) | The hero artifact emailed after a call | **ADOPT (design)** | This is the north star. Mityu already has source-linked summaries (C1). Make the Report the centerpiece. |
| **Chapters & Topics** with jump-to navigation | Navigable meeting structure | **ADOPT** | Derive on-device from the transcript; each chapter links to its segment+timestamp. |
| **Action items** with accountability | Auto-extracted to-dos | **ADAPT** | Mityu has C2 action items. Keep — but **draft until approved**, each with a `source_chunk_id` link (HITL §0.5). Owners optional. |
| **Transcript** with speaker separation | Full transcript, diarized | **ADAPT** | Mityu captures **mixed system audio**, not per-participant streams (no bot). Diarization is lower-fidelity; ship transcript now, speaker labels as a later, clearly-"best-effort" feature. |
| **Playback + highlights reel** | Rewatch / 2-min recap synced to transcript | **ADOPT** | Mityu keeps `audio.mp4` + has `AudioPlayer`. Sync playback to the transcript timeline. Highlights = later. |
| **"For You" dashboard** (14-day summaries, trending topics, recent action items across meetings) | Personal command center | **ADOPT (adapt)** | Becomes Mityu's **Home** (today the sidebar is the only library). On-device aggregation across local meetings; search already exists (C3). |
| **File upload → report** | Upload audio/video → AI report | **ALREADY HAVE** | Mityu's Import Audio (beta). Align its output to the new Report layout. |
| **Talk-time metrics** | Per-participant talk time | **ADAPT (gated)** | Only meaningful with diarization. Compute **on-device** if/when feasible; frame plainly. Not a launch blocker. |
| **Sentiment / engagement / reactions** | Real-time emotional metrics | **ADAPT, cautious** | On-device only, opt-in, labeled "AI-generated · review required" (EU AI Act Art. 50). Never biometric identification. |
| **"Charisma" / "bias" scoring** | Pseudo-scientific speaker scoring | **REJECT** | Ethically fraught, EU-AI-Act-risky, and off-brand for a privacy-first tool. Deliberate divergence from read.ai. |
| **Cloud bot that joins calls** | Auto-join Zoom/Meet/Teams | **REJECT (core)** → opt-in later | Contradicts §1. Lives only as a future consent-gated Integration (EPIC G / ADR-0018 Tier 2), never the default. |
| **Auto-push to Asana/Jira/Notion/Slack** | One-click export to tools | **DEFER → opt-in** | Manual export/copy now (HITL "send is a user action"); integrations are EPIC G, per-workspace consent. |

## The redesign, screen by screen

North-star aesthetic: **card-dense, calm, data-legible** — like read.ai's report, on Mityu's bluedev
palette (`#1E56FF`), fully themed light/dark, every AI block carrying its HITL "draft → approve" state
and a source link. Sequenced so each screen is shippable and verifiable on its own.

**Phase A — Foundation (in progress).** Design tokens + dark mode (Slices 0a–0c done: tokens, next-themes,
shell + Settings/General migrated). Finish the token migration across the remaining screens so nothing is
half-dark: recording view, editor/notes, onboarding, the other Settings tabs. *Prerequisite for any redesign.*

**Phase B — The Meeting Report (the hero).** Redesign the notes/summary view (`app/notes`, `MeetingDetails`,
BlockNote editor + `AISummary`) into a read.ai-style **Report**: a header (title, date, duration), then
cards in order — **Summary → Key takeaways → Action items → Topics/Chapters (with a timeline) → Transcript**.
Every card is a source-linked draft with an approve control; "AI-generated · review required" labels are
non-hideable. Builds directly on C1 (source-linked summaries) + C2 (action items). This is where Mityu
starts to *feel* like read.ai while staying local-first + HITL.

**Phase C — Home / library (the "For You" analogue).** Turn the home route into a dashboard: recent meetings
as report cards, open action items aggregated across meetings, topics seen lately, and the existing search
(C3) promoted to the top. On-device aggregation only.

**Phase D — Playback synced to transcript.** Wire `AudioPlayer` to the transcript/Report timeline: click a
segment → seek; playhead highlights the active segment. Uses the local `audio.mp4` (mind the retention
decision, ADR-0005 / C6).

**Phase E — On-device speaker + talk-time (research, gated).** Investigate diarization on mixed system audio.
If viable on-device, add speaker labels + talk-time to the Report, framed as best-effort. Its own ADR before
any model ships (model licensing, §9). Never "charisma/bias."

**Cross-cutting:** brand tokens everywhere; motion (framer-motion, already a dep) for tab/underline/card
transitions; empty states with a clear next action; consistent iconography (lucide); AAA contrast in both
themes; every AI output draft-by-default + source-linked.

## Non-negotiables carried from the constitution

- No cloud dependency in capture→transcript→summary→store; app works fully offline.
- No AI item published without human approval + source-segment link.
- No biometric/"charisma" inference; sentiment (if any) is on-device, opt-in, labeled.
- Integrations (calendar, bot, tool-push) are opt-in, per-workspace, consent-gated — never default.

## Scope decision (owner, 2026-07-09)

**Follow read.ai closely — including its analytics/metrics surface — not just the report layout.** So the
metrics that were "ADAPT (gated/cautious)" above are promoted to first-class goals: talk-time, engagement,
sentiment, topics and chapters are **in scope**, presented prominently in the Report and dashboard. Two
changes to the plan follow: **Phase E (on-device speaker diarization + talk-time) moves up** — it's now a
priority enabler, not a maybe — and the Report/dashboard are designed with a metrics rail from the start.

**The four guardrails still hold (non-negotiable, constitution-level):** (1) fully on-device, no cloud
dependency; (2) HITL — every AI metric/summary is a draft, source-linked, human-approvable, Art.50-labeled;
(3) **no "charisma"/"bias" biometric scoring** (rejected regardless of read.ai); (4) **no default cloud bot /
audio streaming** — call-join stays a future opt-in Integration. "Follow read.ai for a large portion" is
satisfied within these limits: everything read.ai shows, Mityu can show too — computed locally, shown as
reviewable drafts.

## Status

Accepted as design direction (2026-07-09), scope = follow read.ai closely within the four guardrails. Phase A
(token/dark-mode migration) underway. Phase B (the Report) is the highest-value redesign; Phase E
(diarization/metrics research) promoted to run alongside B since metrics are now first-class.
