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
| `summary_block` (or JSON) | Block/Section content | type(text/bullet/heading1/heading2), content, source_chunk_id | yes |
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

## Client SQLite physical schema (as of migration `20260702000000_add_workspace_and_sync_columns`)

The client's physical table names predate this doc (Meetily heritage) and differ from the logical names. Do **not** rename them (renames on synced tables are two-step); map at the sync/repository layer instead:

| Client table (SQLite) | Logical entity | Synced? | Common columns present |
|---|---|---|---|
| `meetings` | `meeting` | yes | `workspace_id`, `created_at`, `updated_at`, `updated_by`, `rev`, `deleted_at` |
| `transcripts` | `transcript_chunk` (one row per time segment; also carries legacy `summary`/`action_items`/`key_points` TEXT columns) | yes | all of the above (`created_at`/`updated_at` added by 20260702000000, backfilled from the parent meeting) |
| `summary_processes` | `summary` (status + result JSON per meeting) | yes | all of the above |
| `transcript_chunks` | `transcript` (full concatenated text per meeting, one row per meeting) | yes | all of the above (`updated_at` added, backfilled from `created_at`) |
| `meeting_notes` | per-meeting user notes (meeting content) | yes | all of the above |
| `settings`, `transcript_settings` | `settings` (per-workspace config) | **no** (their `*ApiKey` columns now hold only the non-secret marker `keychain:v1` — the real BYOK secret lives in the OS credential store, see ADR-0011; columns retained for schema compat, never synced raw) | `workspace_id`, `created_at`, `updated_at` only — deliberately **no** `updated_by`/`rev`/`deleted_at` |
| `licensing` | — (device-scoped license activation state, not workspace domain data) | no | none — deliberately untouched |

Notes:
- `workspace_id TEXT NOT NULL DEFAULT 'local'` everywhere — the default **must equal** `context::LOCAL_WORKSPACE_ID` (`frontend/src-tauri/src/context.rs`). `rev INTEGER NOT NULL DEFAULT 1`; `updated_by`/`deleted_at` nullable TEXT.
- There is **no dedicated `action_item` client table yet**: extracted actions live in `summary_processes.result` JSON (and the legacy `transcripts.action_items` column). Promoting them to a first-class table (with mandatory `source_chunk_id`) is a future migration.
- `transcripts.created_at`/`updated_at` and the settings tables' timestamps are nullable at the SQL level (SQLite cannot `ADD COLUMN NOT NULL` without a constant default); the migration backfills existing rows, and the tenant-scoped repositories (B2 phase 2, implemented — see ADR-0010) populate them on every insert/update. Repository writers bind `chrono::DateTime<Utc>` (RFC 3339 with offset, e.g. `2026-07-02T10:00:00.123+00:00`); migration backfills used `STRFTIME('%Y-%m-%dT%H:%M:%fZ')` (`Z` suffix). Readers must accept both (sqlx's chrono decoder does).
- Phase-2 hot-path indexes: `idx_meetings_workspace_created (workspace_id, created_at)`, `idx_transcripts_workspace_meeting (workspace_id, meeting_id)`. The other domain tables are one-row-per-meeting with `meeting_id` as PRIMARY KEY.
- **BYOK secrets are keychain-backed (ADR-0011).** The `*ApiKey` columns on `settings` (`openaiApiKey`, `anthropicApiKey`, `ollamaApiKey`, `groqApiKey`, `openRouterApiKey`, `geminiApiKey`) and `transcript_settings` (`whisperApiKey`, `deepgramApiKey`, `elevenLabsApiKey`, `groqApiKey`, `openaiApiKey`) no longer store the secret. `SettingsRepository::save_api_key`/`save_transcript_api_key` write the key to the OS credential store (`crate::secrets`) and persist only the literal marker `keychain:v1` in the column; the getters read from the credential store. Entries are scoped `com.bluedev.mityu` / `{workspace_id}:{summary|transcript}:{provider}:api_key` — workspace-scoped (tenant-aware) and domain-scoped so the two tables' overlapping provider names (`openai`, `groq`) never collide. A one-time, idempotent, offline startup migration (`migrate_plaintext_keys_to_keychain`, run inside `DatabaseManager::new`) moves any legacy plaintext still in a column into the credential store and overwrites the column with the marker. Columns are deliberately **kept** (no drop) for schema compat and older-binary tolerance; only the non-secret marker/label is ever eligible to sync.

### Sync-compatibility note — migration `20260702000000`
Client-only today: no server or sync protocol exists yet, so no wire compatibility is affected. The change is purely **additive with constant defaults** (`'local'`, `1`, NULL): an older client binary opening an upgraded database keeps working — its INSERTs use explicit column lists (defaults apply) and its reads map rows by column name (new columns ignored). An upgraded binary opening an older database applies the migration on startup via the `_sqlx_migrations` ledger. Nothing was renamed, dropped, or retyped, so no two-step deprecation applies. When sync ships (Phase 2), rows with `rev = 1` and `updated_by IS NULL` are "never synced/never edited remotely" — the correct initial state.

## SQLite ↔ Postgres compatibility rules
- Same entity names, same field semantics, compatible types (uuid as TEXT in SQLite, `uuid` in PG; timestamps as ISO-8601 TEXT in SQLite, `timestamptz` in PG).
- Additive evolution only on synced tables; renames/drops are two-step (deprecate → migrate → drop).
- Server tables get an RLS policy in the same migration that creates them.
- A synced-table change ships with a **sync-compatibility note** so older offline clients don't break on next sync.

## Sync semantics (Phase 2+)
- Client pushes local changes with `rev`; server merges per-field last-write-wins, writing an audit entry on conflict.
- Deletes are soft (`deleted_at`) and propagate; hard delete is a separate, audited retention job.
- `provider_credential` secrets never leave the device; only a non-secret reference/label may sync.
- **Dormant client seam (BACKLOG B4, ADR-0012):** the wire types and a disabled `SyncClient` live in `frontend/src-tauri/src/sync/` (`protocol.rs` = the §5 `PushItem`/`ServerAck`/`SyncEntity` shapes; `client.rs` = the `SyncConfig`/`SyncClient`/`Transport`/`RemoteApply` skeleton). It is off by default, wired to nothing, and adds no network dependency.
- **Applying a PULLED remote change must NOT go through the Phase-1 repositories** (`database/repositories/`, ADR-0010): those bump `rev = rev + 1` and stamp `updated_by = ctx.user_id` on every write, which would masquerade a remote change as a local edit, destroy the server-assigned `rev` and the `rev = 1 / updated_by IS NULL` never-synced baseline, and cause a push/ack ping-pong. Inbound application uses the distinct `sync::client::RemoteApply` seam, which writes `rev`/`updated_by`/`updated_at`/`deleted_at` **verbatim** from the payload (a future dedicated remote-apply repository path); `AuthContext` is used for tenant scoping only, never to derive those fields.
- **Local-only vs synced is structural:** `SyncEntity` has variants only for the four `Synced? = yes` entities (meeting/transcript/summary/action_item). There is no `settings`/`provider_credential` variant, so a secret cannot be represented on the wire.
