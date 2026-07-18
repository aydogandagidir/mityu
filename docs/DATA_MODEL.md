# Data Model

One logical model, two physical stores that must stay compatible: **client SQLite** (local-first, source of truth for a user's own captures) and **server PostgreSQL** (Phase 2+, source of truth for shared/team data, RLS-isolated).

## Entities (logical)

| Entity | Purpose | Key fields (beyond the common set) | Synced? |
|---|---|---|---|
| `tenant` (server) / implicit `workspace` (client) | Isolation boundary | name, region, plan, settings | server only |
| `user` (server) / implicit local user (client) | Identity | email, display_name, status | server only |
| `membership` (server) | user↔tenant + role | tenant_id, user_id, role(owner/admin/member/viewer) | server only |
| `meeting` | A recording session / on-site conversation | title, started_at, ended_at, participants | yes |
| `transcript` | Full transcript for a meeting | language, engine(whisper/parakeet), model | yes |
| `transcript_chunk` | Time-segmented transcript pieces | meeting_id, speaker, text, t_start, t_end | yes |
| `summary` | Structured summary (draft→approved) | meeting_id, status(draft/approved), model, sections(JSON) | yes |
| `summary_block` (or JSON) | Block/Section content (client: embedded in `summaries.sections` JSON, no per-block rows — ADR-0019) | type(text/bullet/heading1/heading2), content, source_chunk_id | yes (inside `summary`) |
| `action_item` | Extracted action | meeting_id, text, assignee, due, status, **source_chunk_id** | yes |
| `settings` | Per-workspace/tenant config | allowed_providers, default_model, retention_days, redaction | per-scope |
| `provider_credential` | BYOK key **reference** | provider, key stored in OS keychain/secure store (NOT here in plaintext); the SQLite column holds only the non-secret marker `keychain:v1` | never synced raw |
| `audit_log` (server) | Append-only actions | tenant_id, actor, action, resource, ts, request_id | server only |

## Common columns on every domain entity
`id uuid` · `workspace_id`/`tenant_id` · `created_at` · `updated_at`.
Synced entities also: `updated_by` · `rev bigint` (monotonic) · `deleted_at` (soft delete).

## Canonical summary schema (keep from the legacy pydantic-ai code as reference)
```
MeetingNotes { meeting_name, sections: [ Section ] }
Section      { title, blocks: [ Block ] }
Block        { id, type: text|bullet|heading1|heading2, content, color, source_chunk_id }
```
`source_chunk_id` is our addition and is **mandatory** — every generated block/action links back to the transcript segment + timestamp it came from (HITL + evidence value).

## Client SQLite physical schema (as of migration `20260714010000_enable_verifiable_local_deletion`)

The client's physical table names predate this doc (Meetily heritage) and differ from the logical names. Do **not** rename them (renames on synced tables are two-step); map at the sync/repository layer instead:

| Client table (SQLite) | Logical entity | Synced? | Common columns present |
|---|---|---|---|
| `meetings` | `meeting` | yes | `workspace_id`, `created_at`, `updated_at`, `updated_by`, `rev`, `deleted_at` |
| `transcripts` | `transcript_chunk` (one row per time segment; also carries legacy `summary`/`action_items`/`key_points` TEXT columns) | yes | all of the above (`created_at`/`updated_at` added by 20260702000000, backfilled from the parent meeting) |
| `summary_processes` | legacy generation-process status + result JSON per meeting (pre-C1; the sync `summary` entity maps to `summaries` below) | yes | all of the above |
| `transcript_chunks` | `transcript` (full concatenated text per meeting, one row per meeting) | yes | all of the above (`updated_at` added, backfilled from `created_at`) |
| `meeting_notes` | per-meeting user notes (meeting content) | yes | all of the above |
| `summaries` | `summary` — structured source-linked summary draft, ONE row per meeting (UNIQUE `meeting_id`); `sections` JSON holds the CONTRACTS §4 `Section`/`Block` shapes incl. per-block `source_chunk_id`/`status`/`original_content`; HITL approval recorded in `approved_at`/`approved_by` | yes | all of the above (created by `20260706000000`, C1) |
| `action_items` | `action_item` — first-class extracted action with **mandatory `source_chunk_id`**; `position` orders items per meeting, `original_text` preserves pre-edit text | yes | all of the above (created by `20260706000000`, C1) |
| `transcript_search_documents` | **Derived local routing map** from an indexed synthetic integer document id to `(workspace_id, meeting_id, source_chunk_id)`; maintenance-only, never an authority/domain entity | **no** | none — UNIQUE source lookup plus indexed workspace/meeting lookup; no transcript text |
| `transcript_search_fts` | **Derived local FTS5 index**, one document per active `transcripts` segment; BM25 retrieval only, never an authority/domain entity | **no** | none — stores only normalized transcript text under the routing map's integer document id |
| `local_privacy_maintenance` | **Content-free local maintenance marker** for crash-resumable FTS/WAL/free-page compaction | **no** | none — singleton operational state only; no meeting/workspace id or content |
| `settings`, `transcript_settings` | `settings` (per-workspace config) | **no** (their `*ApiKey` columns now hold only the non-secret marker `keychain:v1` — the real BYOK secret lives in the OS credential store, see ADR-0011; columns retained for schema compat, never synced raw) | `workspace_id`, `created_at`, `updated_at` only — deliberately **no** `updated_by`/`rev`/`deleted_at` |
| `licensing` | — (device-scoped license activation state, not workspace domain data) | no | none — deliberately untouched |

Notes:
- `workspace_id TEXT NOT NULL DEFAULT 'local'` everywhere — the default **must equal** `context::LOCAL_WORKSPACE_ID` (`frontend/src-tauri/src/context.rs`). `rev INTEGER NOT NULL DEFAULT 1`; `updated_by`/`deleted_at` nullable TEXT.
- Extracted actions historically lived in `summary_processes.result` JSON (and the legacy `transcripts.action_items` column). Migration `20260706000000` (C1) promoted them to the first-class `action_items` table (with mandatory `source_chunk_id`) and added `summaries` for structured source-linked drafts; the legacy JSON locations remain readable for pre-C1 data and for older binaries.
- **Action Center v1 is a projection, not a schema expansion (ADR-0025).** It reads only first-class `action_items.status = 'approved'` rows joined to an active same-workspace meeting and active same-meeting transcript source. `edited` awaits re-approval. The existing status is exclusively the HITL review lifecycle; it is not `open`/`done` work progress, and `due` remains uninterpreted free text. The response is paginated but no table, column, migration or sync entity is added.
- **Deliberately NO foreign key on `source_chunk_id` → `transcripts(id)`** (on `action_items`, and per-block inside `summaries.sections` JSON where a SQL FK is impossible anyway): retranscription deletes+reinserts segment rows, so a CASCADE would silently destroy approved evidence links and RESTRICT would block retranscription. Resolvability is enforced at the repository layer at write- and approve-time (CONTRACTS §4). `meeting_id` on both tables DOES cascade, like `transcripts`.
- `transcripts.created_at`/`updated_at` and the settings tables' timestamps are nullable at the SQL level (SQLite cannot `ADD COLUMN NOT NULL` without a constant default); the migration backfills existing rows, and the tenant-scoped repositories (B2 phase 2, implemented — see ADR-0010) populate them on every insert/update. Repository writers bind `chrono::DateTime<Utc>` (RFC 3339 with offset, e.g. `2026-07-02T10:00:00.123+00:00`); migration backfills used `STRFTIME('%Y-%m-%dT%H:%M:%fZ')` (`Z` suffix). Readers must accept both (sqlx's chrono decoder does).
- Phase-2 hot-path indexes: `idx_meetings_workspace_created (workspace_id, created_at)`, `idx_transcripts_workspace_meeting (workspace_id, meeting_id)`; C1 adds `idx_summaries_workspace_meeting`, `idx_action_items_workspace_meeting`, `idx_action_items_workspace_status`. The remaining domain tables are one-row-per-meeting with `meeting_id` as PRIMARY KEY.
- **Evidence search is derived and transcript-only in v1 (ADR-0024).** Migration `20260714000000` backfills `transcript_search_documents` + `transcript_search_fts` and installs transcript insert/update/delete plus meeting soft-delete/restore/delete triggers. The B-tree routing map makes source and meeting maintenance indexed; FTS deletes use its integer rowid rather than scanning UNINDEXED metadata. The repository joins every FTS candidate back to active, workspace-scoped `transcripts` + `meetings`; stale index rows are never trusted. Legacy/unapproved summary JSON is deliberately outside this surface. Approved summary/action search is a later HITL-gated derived-document addition.
- FTS stores a second copy of transcript text **inside the same SQLCipher database**. It creates no plaintext sidecar and is covered by the SQLCipher conversion test. ADR-0026 and migration `20260714010000` now pair per-connection SQLite `PRAGMA secure_delete=ON` with persistent FTS5 `secure-delete=1`; a successful meeting deletion then optimizes FTS, performs checked WAL truncation, runs `VACUUM`, verifies `freelist_count = 0`, and checkpoints the content-free maintenance marker. Startup resumes any pending cycle. This is a verified best-effort cleanup of Mityu-controlled stores, **not** a forensic-erasure claim for SSD wear-leveling, copy-on-write filesystems, snapshots, backups, exports, swap, or WebView/browser physical remnants.
- BM25 corpus statistics are global to an FTS5 virtual table. Phase 1 has exactly one implicit local workspace, and the raw BM25 value is not exposed. A client that supports multiple local workspaces MUST partition retrieval statistics (or adopt a tenant-local ranker) before enabling that mode; this derived index is never reused by the multi-tenant server.
- **BYOK secrets are keychain-backed (ADR-0011).** The `*ApiKey` columns on `settings` (`openaiApiKey`, `anthropicApiKey`, `ollamaApiKey`, `groqApiKey`, `openRouterApiKey`, `geminiApiKey`) and `transcript_settings` (`whisperApiKey`, `deepgramApiKey`, `elevenLabsApiKey`, `groqApiKey`, `openaiApiKey`) no longer store the secret. `SettingsRepository::save_api_key`/`save_transcript_api_key` write the key to the OS credential store (`crate::secrets`) and persist only the literal marker `keychain:v1` in the column; the getters read from the credential store. Entries are scoped `com.bluedev.mityu` / `{workspace_id}:{summary|transcript}:{provider}:api_key` — workspace-scoped (tenant-aware) and domain-scoped so the two tables' overlapping provider names (`openai`, `groq`) never collide. A one-time, idempotent, offline startup migration (`migrate_plaintext_keys_to_keychain`, run inside `DatabaseManager::new`) moves any legacy plaintext still in a column into the credential store and overwrites the column with the marker. Columns are deliberately **kept** (no drop) for schema compat and older-binary tolerance; only the non-secret marker/label is ever eligible to sync.

### Sync-compatibility note — migration `20260714010000`
Purely **local and additive to the domain/sync schema**: it persists FTS5 `secure-delete=1` and adds the singleton `local_privacy_maintenance` operational table. The table contains no meeting content, workspace/tenant identifier, sync columns, `SyncEntity`, or wire representation. The per-connection SQLite core pragma is runtime configuration rather than a synced schema field. No domain table/column/index is renamed, removed, or retyped. **Application downgrade is not supported:** the SQLx ledger rejects the newer migration, and FTS5 indexes written with secure-delete require SQLite 3.42 or newer; Mityu bundles SQLCipher/SQLite 3.45.3. A rollback would require a new forward migration that disables FTS secure-delete, rebuilds the FTS index for the older format, and removes the marker table—never an edit to the applied migration.

### Sync-compatibility note — migration `20260714000000`
Purely **additive to the domain/sync schema**: one local routing table, one routing index, one FTS5 virtual table and five maintenance triggers; no domain table/column/index is renamed, removed or retyped. Newer binaries backfill both derived structures from active transcript rows. Neither structure has a `SyncEntity`, common sync columns or wire representation; each client rebuilds them independently. **Application downgrade is not supported:** SQLx's default `ignore_missing = false` migrator rejects a database whose ledger contains version `20260714000000` when opened by an older build. The minimum compatible app is therefore the first build containing this migration (or newer).

### Sync-compatibility note — migration `20260706000000`
Purely **additive** — two brand-new tables (`summaries`, `action_items`) + three indexes; no existing table, column, row, or index is touched. Client-only today (no server or sync protocol yet). At the schema/data level an older binary's SQL paths would ignore these tables and keep using `summary_processes.result`; this is **not application downgrade support**, because the current SQLx migrator rejects a newer ledger when an older build opens it. A newer binary opening an older database applies the migration on startup. Nothing renamed/dropped/retyped ⇒ no two-step deprecation. Both tables are SYNCED-class (CONTRACTS §7 common columns; `SyncEntity::Summary`/`::ActionItem` already exist), but wire promotion is Phase-2 policy: **draft rows default local-only** (MULTITENANCY "Data classification"); approved rows become sync-eligible when Phase 2 ships. Fresh rows carry the never-synced baseline (`rev = 1`, `updated_by IS NULL`).

### Sync-compatibility note — migration `20260702000000`
Client-only today: no server or sync protocol exists yet, so no wire compatibility is affected. The change is purely **additive with constant defaults** (`'local'`, `1`, NULL): at the schema/data level an older binary's explicit INSERTs and name-mapped reads remain compatible, but the current SQLx migration-ledger policy still rejects application downgrade. An upgraded binary opening an older database applies the migration on startup. Nothing was renamed, dropped, or retyped, so no two-step deprecation applies. When sync ships (Phase 2), rows with `rev = 1` and `updated_by IS NULL` are "never synced/never edited remotely" — the correct initial state.

## SQLite ↔ Postgres compatibility rules
- Same entity names, same field semantics, compatible types (uuid as TEXT in SQLite, `uuid` in PG; timestamps as ISO-8601 TEXT in SQLite, `timestamptz` in PG).
- Additive evolution only on synced tables; renames/drops are two-step (deprecate → migrate → drop).
- Server tables get an RLS policy in the same migration that creates them.
- A synced-table change ships with a **sync-compatibility note** so older offline clients don't break on next sync.
- “Additive/schema compatible” describes data and future wire evolution, not executable rollback: the current SQLx client does **not** support opening a newer migration ledger with an older app. Release notes/updaters must prevent application downgrade unless a separately tested compatibility policy is implemented.

## Sync semantics (Phase 2+)
- Client pushes local changes with `rev`; server merges per-field last-write-wins, writing an audit entry on conflict.
- Deletes are soft (`deleted_at`) and propagate; hard delete is a separate, audited retention job.
- `provider_credential` secrets never leave the device; only a non-secret reference/label may sync.
- **Dormant client seam (BACKLOG B4, ADR-0012):** the wire types and a disabled `SyncClient` live in `frontend/src-tauri/src/sync/` (`protocol.rs` = the §5 `PushItem`/`ServerAck`/`SyncEntity` shapes; `client.rs` = the `SyncConfig`/`SyncClient`/`Transport`/`RemoteApply` skeleton). It is off by default, wired to nothing, and adds no network dependency.
- **Applying a PULLED remote change must NOT go through the Phase-1 repositories** (`database/repositories/`, ADR-0010): those bump `rev = rev + 1` and stamp `updated_by = ctx.user_id` on every write, which would masquerade a remote change as a local edit, destroy the server-assigned `rev` and the `rev = 1 / updated_by IS NULL` never-synced baseline, and cause a push/ack ping-pong. Inbound application uses the distinct `sync::client::RemoteApply` seam, which writes `rev`/`updated_by`/`updated_at`/`deleted_at` **verbatim** from the payload (a future dedicated remote-apply repository path); `AuthContext` is used for tenant scoping only, never to derive those fields.
- **Local-only vs synced is structural:** `SyncEntity` has variants only for the four `Synced? = yes` entities (meeting/transcript/summary/action_item). There is no `settings`/`provider_credential` variant, so a secret cannot be represented on the wire.
