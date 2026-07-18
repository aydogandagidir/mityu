# Roadmap (phased, go/no-go gated)

Local-first first, server-optional later. Each phase has an exit gate; do not silently skip or misreport it. ADR-0027 records a narrow publication-only exception for v1.0.4: A5 and C8 are deferred/not passed, field and accuracy claims stay prohibited, and downstream C8 dependencies remain locked.

## Phase 0 — Foundation & de-risk (before feature work)
- Fork the MIT base; rebrand to **Mityu** (working name); wire this Claude Code pack (CLAUDE.md, .claude/, docs/). Concrete rebrand edits:
  - `frontend/src-tauri/tauri.conf.json`: `productName` → `"Mityu"`; `identifier` → `"com.bluedev.mityu"`; window `title` → `"Mityu"`.
  - `frontend/package.json`: `"name"` → `"mityu"`. `frontend/src-tauri/Cargo.toml`: `name = "mityu"` (update the root workspace member path if the crate dir is renamed).
  - Replace app icons/assets and user-facing "meetily" strings. Keep the MIT `LICENSE` (Zackriya Solutions copyright) intact.
  - Note in DECISIONS.md that the name is provisional; run a TÜRKPATENT/EUIPO/USPTO + domain/app-store availability check before public launch.
- Decide in DECISIONS.md: authoritative audio module (`audio` vs `audio_v2`), server language (Rust/Axum vs FastAPI), audio-retention default.
- **Prove transcription quality on YOUR real audio** (meeting room + noisy field), whisper `large-v3` vs Parakeet, custom vocabulary for domain jargon. **Gate:** acceptable WER, or narrow scope. For v1.0.4 this remains explicit evidence debt (ADR-0027), not a quality claim.

## Phase 1 — Enterprise local-first MVP (single tenant, no server)
- Introduce the seams (WorkspaceContext/AuthContext, `workspace_id` on all entities, repository layer, dormant `sync/` module).
- Add: SQLCipher-encrypted local DB, local audit log, retention/redaction policy, org branding, structured source-linked summaries with HITL approval, export (PDF/Docx/Markdown).
- Converge canonical editor to BlockNote; keep legacy paths inert.
- **Gate:** works fully offline; a real pilot user gets value; DoD green. C8 is deferred/not performed for v1.0.4 only and remains the Phase 1 exit gate.

## Phase 2 — Enterprise self-host (optional server turns on)
- Build the NEW clean `server/` (authenticated, multi-tenant): OIDC (Keycloak/Authentik), RBAC, Postgres+RLS, audit, central policy, tenant-scoped sync API; enable the client `sync/`.
- Team features: shared workspaces, roles, admin console, SSO.
- **Gate:** negative cross-tenant test passes; app still works with server down; first team/enterprise customer live.

## Phase 3 — Managed multi-tenant SaaS
- Harden isolation & scale; per-tenant metering + billing; hosted IdP; EU-region deployment; self-serve onboarding.
- **Gate:** unit economics positive; isolation & audit verified at scale.

## Phase F — On-device AI agents (optional; only after the Phase 1 gate C8)
Backs the About-screen "Coming soon: a library of on-device AI agents — automating follow-ups, action tracking." NOT a new step on the linear path: a feature epic that turns on only after **C8**, because it consumes **C1** (source-linked summaries) + **C2** (action items). Local-first, **draft-only (HITL)**, **no autonomous external actions** (`CLAUDE.md` §0.5/§10). A dormant, tested seam is scaffolded now — `frontend/src-tauri/src/agents/` (off by default, ADR-0013, BACKLOG EPIC F).
- F0 ADR/design → F1 flag-gated framework + `agent_runs` migration → F2 follow-up drafter → F3 action-item tracker → F4 agents panel + transparency labels → F5 (later) opt-in scheduling.
- **Gate:** fully offline; every output a source-linked draft a human approves; no external side effects; `/security-review` + multitenancy-guardian clean.
- **Core is integration-free (by design):** Mityu captures system audio ("any meeting app … or a face-to-face conversation" — macOS + Windows; Linux is microphone-only, ADR-0022) and is **not a meeting bot by default** (`CLAUDE.md` §1). Meet/Zoom/Teams-related capabilities exist only as **opt-in, consent-gated integrations** — calendar metadata (light) and, in Phase 2+, a call-joining bot — per **ADR-0018** (BACKLOG EPIC G, post-C8).

## First 30 / 60 / 90 days
- **0–30:** Phase 0 + start Phase 1 seams. Real-audio transcription validation is the make-or-break task.
- **30–60:** Phase 1 core (encrypted local store, source-linked HITL summaries, export); onboard 1 pilot.
- **60–90:** Pilot hardening + consent/transparency + one export/integration; decide server language; scaffold (not deploy) `server/` skeleton with tenancy seam.
