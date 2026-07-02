# SCAFFOLD — Target module layout for the code we add

The agent adds new code in these locations, adapting to the real repo. Goal: the seams live in obvious, single-responsibility places so tenant-safety and local-first are easy to keep and review. Do not scatter these concerns.

## Client (Tauri core) — `frontend/src-tauri/src/`
```
src/
  context.rs              # AuthContext / WorkspaceContext (CONTRACTS §1) — the ONLY identity source
  repository/             # tenant-scoped Repository trait + SQLite impls (CONTRACTS §2)
    mod.rs
    meeting_repo.rs
    transcript_repo.rs
    summary_repo.rs
    action_item_repo.rs
    settings_repo.rs
  llm/
    provider.rs           # LlmProvider trait (CONTRACTS §3)
    ollama.rs openai.rs anthropic.rs groq.rs openrouter.rs   # keys via secure store only
    summary.rs            # structured_summary orchestration → MeetingNotesDraft (source_chunk_id)
  policy.rs               # per-workspace policy: allowed_providers, retention, redaction (CONTRACTS §7 / SECURITY_PRIVACY)
  audit_local.rs          # local audit trail (append-only)
  sync/                   # DORMANT until Phase 2 (CONTRACTS §5)
    mod.rs protocol.rs client.rs
  migrations/             # forward-only SQLite migrations (DATA_MODEL) — RLS N/A locally
  (existing) audio*/ whisper_engine/ parakeet_engine/ database/ commands...
```
- New Tauri commands: typed, registered in `lib.rs`, with a typed TS wrapper in `frontend/src/services/`.
- `context.rs` + `repository/` land in EPIC B before feature work that persists data.

## Frontend — `frontend/src/`
```
src/
  services/               # typed invoke() wrappers (NO raw invoke in components)
    meetings.ts summaries.ts transcription.ts settings.ts sync.ts
  components/
    review/               # HITL: draft summary/action items + Approve + source-link chips
    consent/              # recording indicator, analytics opt-in, "AI-generated (review required)"
    export/               # PDF/DOCX/MD export
  (canonical editor = BlockNote; no new TipTap/Remirror)
```

## Optional server — `server/` (NEW, Phase 2; language per ADR-0003)
```
server/
  src/
    main.*                # bootstrap
    auth/                 # OIDC validation → AuthContext (CONTRACTS §1)
    middleware/           # sets app.tenant_id per request (RLS), request_id, audit
    repository/           # Postgres impls, tenant-scoped, mirror client entities
    routes/               # meetings, transcripts, summaries, action_items, sync, admin
    rbac.*                # Role checks (owner/admin/member/viewer)
    audit.*               # append-only AuditEvent (CONTRACTS §6)
    sync/                 # tenant-scoped sync endpoints (CONTRACTS §5)
  migrations/             # Postgres migrations; EACH table ships its RLS policy (MULTITENANCY)
  tests/
    cross_tenant_isolation_test.*   # MANDATORY negative test: A cannot see B
```
Never import or depend on the legacy `backend/`. It is reference-only.

## Evaluation — `eval/` (Phase 0)
```
eval/
  run_eval.py             # PHASE0_VALIDATION harness
  jargon.txt              # domain/part terms (TR+EN)
  quiet/ field/ multi/ jargon/   # <id>.wav + <id>.ref.txt
  report.md report.json   # output + verdict
```
