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
| `provider_credential` | BYOK key **reference** | provider, key stored in OS keychain/secure store (NOT here in plaintext) | never synced raw |
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

## SQLite ↔ Postgres compatibility rules
- Same entity names, same field semantics, compatible types (uuid as TEXT in SQLite, `uuid` in PG; timestamps as ISO-8601 TEXT in SQLite, `timestamptz` in PG).
- Additive evolution only on synced tables; renames/drops are two-step (deprecate → migrate → drop).
- Server tables get an RLS policy in the same migration that creates them.
- A synced-table change ships with a **sync-compatibility note** so older offline clients don't break on next sync.

## Sync semantics (Phase 2+)
- Client pushes local changes with `rev`; server merges per-field last-write-wins, writing an audit entry on conflict.
- Deletes are soft (`deleted_at`) and propagate; hard delete is a separate, audited retention job.
- `provider_credential` secrets never leave the device; only a non-secret reference/label may sync.
