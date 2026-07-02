# BACKLOG — Ordered, executable tasks

The agent works this list **top-to-bottom**, respecting `depends-on`. Each task: an ID, the owning agent, the slash command to use, concrete acceptance criteria, and its gate. "Done" also requires the CLAUDE.md Definition of Done. Do not reorder to chase easy wins; dependencies exist for correctness.

Legend: **Agent** = `.claude/agents/` file · **Cmd** = `.claude/commands/`.

---

## EPIC A — Foundation (Phase 0)

### A1 · Wire the pack & orient
- Agent: (orchestrator) · Cmd: — · depends-on: none
- AC: BOOTSTRAP Step 0 done; contradictions between docs and repo resolved via ADRs.

### A2 · Dev environment reproducible
- Agent: qa-release-engineer · Cmd: — · depends-on: A1
- AC: SETUP.md "environment ready" checklist passes on macOS and Windows (state which were verified); offline summary via Ollama works.

### A3 · Rebrand to Mityu
- Agent: rust-tauri-core-engineer + frontend-nextjs-engineer · Cmd: /feature · depends-on: A2
- AC: productName/identifier/title, package.json, Cargo.toml → Mityu/com.bluedev.mityu/mityu; icons + strings replaced; MIT LICENSE intact; app launches branded.

### A4 · Lock ADR-0003/0004/0005
- Agent: sync-server-architect (0003), audio-pipeline-engineer (0004) · Cmd: — · depends-on: A2
- AC: server language chosen; authoritative audio module identified with evidence; retention default confirmed. All Accepted in DECISIONS.md.

### A5 · Phase 0 transcription validation ⛔ GATE
- Agent: audio-pipeline-engineer + qa-release-engineer · Cmd: /phase0-validate · depends-on: A2
- AC: PHASE0_VALIDATION.md report produced; WER + domain-vocab thresholds met; **human-reviewed go/no-go recorded**. If NO-GO → scope narrows to meeting-room; do not enter EPIC C field features.

---

## EPIC B — Tenant-aware seams (Phase 1, still single-tenant/local)

### B1 · WorkspaceContext / AuthContext seam
- Agent: rust-tauri-core-engineer · Cmd: /feature · depends-on: A3
- AC: `AuthContext { tenant_id, user_id, roles, request_id }` exists (docs/CONTRACTS.md); in local mode resolves to a single local user/workspace; no code reads "current user" any other way.

### B2 · `workspace_id` on all entities + repository layer
- Agent: db-migration-engineer + rust-tauri-core-engineer · Cmd: /db-migration then /prep-multitenant · depends-on: B1
- AC: forward-only migration adds `workspace_id` (+ sync fields) to meetings/transcripts/chunks/summaries/action_items; a tenant-scoped Repository is the ONLY storage access path; migration applies on empty + populated DB.

### B3 · Encrypted local store (SQLCipher)
- Agent: rust-tauri-core-engineer + security-privacy-auditor · Cmd: /feature then /security-review · depends-on: B2
- AC: sensitive local data encrypted at rest; key from OS-protected store; app still opens existing data (migration path documented).

### B4 · Dormant sync module skeleton
- Agent: rust-tauri-core-engineer · Cmd: /add-tauri-command · depends-on: B2
- AC: `sync/` client module with typed protocol messages (docs/CONTRACTS.md) but disabled; app fully works with it off.

---

## EPIC C — Core product value (Phase 1 MVP)

### C1 · Source-linked structured summaries + HITL
- Agent: rust-tauri-core-engineer + frontend-nextjs-engineer · Cmd: /feature · depends-on: A5, B2
- AC: summary uses the Block/Section schema; **every block/action item carries `source_chunk_id`**; UI renders drafts with Approve + visible source link; nothing publishes without approval.

### C2 · Action-item extraction
- Agent: rust-tauri-core-engineer · Cmd: /feature · depends-on: C1
- AC: action items (text, assignee?, due?, status, source_chunk_id) extracted as drafts; editable; approved items persisted.

### C3 · Search across meetings/transcripts
- Agent: rust-tauri-core-engineer + frontend-nextjs-engineer · Cmd: /feature · depends-on: B2
- AC: full-text search over transcripts/summaries within the workspace; tenant-scoped by construction.

### C4 · Export (PDF / DOCX / Markdown)
- Agent: frontend-nextjs-engineer · Cmd: /feature · depends-on: C1
- AC: a meeting's approved summary + action items export to PDF/DOCX/MD with source timestamps; works offline.

### C5 · Consent + transparency UI
- Agent: frontend-nextjs-engineer + security-privacy-auditor · Cmd: /feature then /security-review · depends-on: C1
- AC: visible "recording active" indicator; analytics opt-in; "AI-generated (review required)" labeling; these cannot be hidden. Backs EU AI Act Art. 50.

### C6 · Retention & redaction policy (local)
- Agent: rust-tauri-core-engineer + security-privacy-auditor · Cmd: /feature · depends-on: B3
- AC: configurable retention (default: delete audio after transcription); basic PII/keyword redaction rules applied before persistence/summary.

### C7 · Editor convergence to BlockNote
- Agent: frontend-nextjs-engineer · Cmd: /refactor via /feature · depends-on: C1
- AC: canonical editor = BlockNote; no new TipTap/Remirror usage; legacy paths inert.

### C8 · Phase 1 exit ⛔ GATE
- Agent: qa-release-engineer · Cmd: /release (dry-run) · depends-on: C1–C7
- AC: app works fully offline; a real pilot user completes record→approve→export; DoD green; multitenancy-guardian + security-privacy-auditor pass.

---

## EPIC D — Optional server (Phase 2, only after C8 + a team-customer need)

### D1 · Server skeleton (NEW, clean) — auth + tenancy from commit #1
- Agent: sync-server-architect · Cmd: /feature · depends-on: A4(0003), C8
- AC: `server/` per ADR-0003; OIDC authn; AuthContext derived per request; Postgres + RLS; health/version; NOT the legacy backend.

### D2 · Tenant model + RBAC + audit
- Agent: sync-server-architect + db-migration-engineer · Cmd: /db-migration then /tenant-check · depends-on: D1
- AC: tenants/users/memberships; roles owner/admin/member/viewer enforced on every sensitive route; append-only audit log; **negative cross-tenant test passes**.

### D3 · Tenant-scoped sync API + enable client sync
- Agent: sync-server-architect + rust-tauri-core-engineer · Cmd: /feature then /tenant-check · depends-on: D2, B4
- AC: sync protocol (rev/updated_by/soft-delete, LWW + audit on conflict); client SQLite ↔ server Postgres; app still works with server DOWN.

### D4 · Team features (shared workspaces, admin console, SSO)
- Agent: sync-server-architect + frontend-nextjs-engineer · Cmd: /feature · depends-on: D3
- AC: share a meeting to a workspace; admin console scoped to one tenant; enterprise SSO via OIDC.

### D5 · Phase 2 exit ⛔ GATE
- Agent: qa-release-engineer + security-privacy-auditor · Cmd: /release · depends-on: D1–D4
- AC: cross-tenant isolation verified; app runs with server down; /security-review clean; first team/enterprise customer live.

---

## EPIC E — Managed SaaS (Phase 3, only after D5 + unit-economics validation)
- E1 per-tenant metering + billing · E2 hosted IdP + EU-region deploy · E3 isolation/scale hardening · E4 self-serve onboarding.
- GATE E5: unit economics positive; isolation & audit verified at scale.

---

## EPIC F — On-device AI agents (Phase 1+, optional; only after gate C8)

Backs the About "Coming soon: a library of on-device AI agents." Local-first, **draft-only (HITL)**, source-linked, tenant-scoped, **no autonomous external actions** (ADR-0013). A dormant seam already exists (`frontend/src-tauri/src/agents/`, off by default); these tasks turn it on in sequence. Meeting-platform (Zoom/Meet/Teams) *API* integration is intentionally **not** here — the app captures system audio and is not a meeting bot (`CLAUDE.md` §1).

### F0 · ADR + agent boundaries ⛔ DESIGN GATE
- Agent: rust-tauri-core-engineer + security-privacy-auditor · Cmd: — · depends-on: C8
- AC: ADR-0013 confirmed at kickoff — agents local-first, draft-only (HITL), source-linked, tenant-scoped, no autonomous external actions; trigger = manual/on-demand first. Dormant `agents/` seam already merged; this formalizes scope before code.

### F1 · Agent framework (flag-gated, wired) + `agent_runs` store
- Agent: rust-tauri-core-engineer + db-migration-engineer · Cmd: /add-tauri-command then /db-migration · depends-on: F0
- AC: `AgentRunner` reachable via a flag-gated Tauri command; forward-only migration adds `agent_runs` (`workspace_id` + sync fields) via the tenant-scoped Repository; providers reuse the `summary/` provider layer; app works with agents OFF (default) and fully offline.

### F2 · Follow-up drafter agent
- Agent: rust-tauri-core-engineer + frontend-nextjs-engineer · Cmd: /feature · depends-on: F1, C1, C2
- AC: from an **approved** summary + action items, drafts a follow-up message as a DRAFT in the editor; user edits/approves; "send" is manual export/copy (**never** auto-send); every draft carries `source_chunk_id` links.

### F3 · Action-item tracker agent
- Agent: rust-tauri-core-engineer + frontend-nextjs-engineer · Cmd: /feature · depends-on: F2
- AC: aggregates open action items across meetings into a review list (status, due, source); no auto-notifications; tenant-scoped by construction.

### F4 · Agents panel (UI + transparency)
- Agent: frontend-nextjs-engineer + security-privacy-auditor · Cmd: /feature then /security-review · depends-on: F2
- AC: run-on-demand, draft review/approve, per-run audit; "AI-generated · review required" labels (EU AI Act Art. 50); these cannot be hidden.

### F5 · Opt-in scheduling / automation ⛔ GATE
- Agent: rust-tauri-core-engineer + security-privacy-auditor · Cmd: /feature then /security-review · depends-on: F4
- AC: optional scheduled runs; even then outputs are draft-by-default or require explicit per-action approval; fully offline; `/security-review` + multitenancy-guardian pass; **no autonomous irreversible action ships**.

---

## Cross-cutting (apply on every task)
- Run the PreToolUse/PostToolUse hooks (auto). Before any release: `/security-review` + `/tenant-check`.
- Add/adjust tests (server endpoints + non-trivial Rust logic). CI (.github/workflows/ci.yml) must be green.
- Update the relevant docs/ file and add an ADR when architecture/schema changes.
