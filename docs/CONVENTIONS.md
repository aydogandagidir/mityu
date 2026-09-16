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

### Starting the app, not just building it
Some defects are invisible to every check above: a plugin declared for the wrong
platform links nothing and registers nothing, and that is not a compile error,
not a type error, and not a failing test. v1.2.1 shipped exactly that — no log
file at all on Windows (ADR-0047). The only way to find it is to start the
program and look.

```bash
cargo build -p mityu
tools/ci/verify-startup.sh          # Linux needs xvfb-run
```

It launches the binary, waits for the app's own `Application setup complete`
line, and asserts **exactly one** log file with real content in it. That last
assertion is not fussiness: `tauri_plugin_log::Builder::target()` *appends* to
the two targets the builder already carries, so registering two of our own once
gave four targets, two byte-identical log files and a rotation ceiling twice the
one in the source (ADR-0049). Use `.targets([...])`, which replaces.

CI runs this in the `rust` job. It runs on ubuntu, so it does **not** prove the
Windows build logs, and it proves nothing about recording — there is no audio
device on a runner.

### Running the Rust suite locally
The ONNX runtime is not on the default library path, and it is needed at two
different moments:

```bash
cd frontend/src-tauri
ORT_LIB_LOCATION=/tmp/ort/lib cargo test --all     # build time
```

If a test binary is run directly, it also needs `LD_LIBRARY_PATH=/tmp/ort/lib`
at run time. Omitting it is loud, not silent — the loader fails with
`libonnxruntime.so.1: cannot open shared object file` and exit code 127 — so it
cannot be mistaken for a passing run.

**None of this is the smoke test.** CLAUDE.md §4 requires a human
record→transcript run on macOS and Windows for any change to the audio or
recording-start paths. No check in CI can stand in for it: no runner has a
microphone.

## Git / PR
- Branches: `feat/<slug>`, `fix/<slug>`, `chore/<slug>`, `refactor/<slug>`.
- Conventional commits: `feat: …`, `fix: …`, `refactor: …`, `docs: …`, `test: …`, `chore: …`.
- PR checklist = CLAUDE.md Definition of Done + `/tenant-check` (server) + `/security-review` (before release). CI must be green.
- One concern per PR. Never mix an audio-pipeline change with a schema change.

## Secrets & config
- LLM keys: OS keychain / Tauri secure store only. Never in SQLite plaintext, source, logs, analytics, or git.
- No hardcoded paths (use Tauri path APIs) or hardcoded ports as required infra.
