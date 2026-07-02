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

## Git / PR
- Branches: `feat/<slug>`, `fix/<slug>`, `chore/<slug>`, `refactor/<slug>`.
- Conventional commits: `feat: …`, `fix: …`, `refactor: …`, `docs: …`, `test: …`, `chore: …`.
- PR checklist = CLAUDE.md Definition of Done + `/tenant-check` (server) + `/security-review` (before release). CI must be green.
- One concern per PR. Never mix an audio-pipeline change with a schema change.

## Secrets & config
- LLM keys: OS keychain / Tauri secure store only. Never in SQLite plaintext, source, logs, analytics, or git.
- No hardcoded paths (use Tauri path APIs) or hardcoded ports as required infra.
