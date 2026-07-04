-- Migration: create `summaries` + `action_items` tables (BACKLOG C1.2)
--
-- Purpose (docs/CONTRACTS.md §4, docs/DATA_MODEL.md "Entities"):
--   BACKLOG C1 structured, source-linked summary DRAFTS, and first-class action
--   items for C2. `summaries` holds ONE row per meeting (UNIQUE meeting_id) whose
--   `sections` column stores the §4 JSON shapes produced by `summary::draft`
--   (src/summary/draft.rs): an array of Section { title, blocks: [Block] } with
--   per-block { id, type, content, source_chunk_id, status, original_content } —
--   every AI-generated block keeps its mandatory transcript-evidence anchor
--   (HITL, CLAUDE.md §0.5). `action_items` is one row per extracted action with
--   the same mandatory `source_chunk_id`, `position` for per-meeting display
--   order, and `original_text` preserving the pre-edit text of a human-edited
--   item (the row-level analogue of Block.original_content).
--   `status` stores the §4 wire tokens verbatim: summaries.status is
--   'draft'|'approved' (SummaryStatus); action_items.status is
--   'draft'|'approved'|'edited'|'rejected' (BlockStatus). The 'draft' defaults
--   mean nothing is ever born approved: approval is an explicit HUMAN action,
--   recorded on `summaries` in approved_at/approved_by.
--
-- Table classification (docs/CONTRACTS.md §7; docs/DATA_MODEL.md logical names):
--   summaries    -> summary       SYNCED-class (full common-column set:
--                                  workspace_id + created_at/updated_at +
--                                  updated_by/rev/deleted_at)
--   action_items -> action_item   SYNCED-class (same set)
--   Both already have wire entities (`SyncEntity::Summary` / `::ActionItem`,
--   src/sync/protocol.rs). Wire promotion is Phase-2 POLICY: per
--   docs/MULTITENANCY.md "Data classification & sync scope", DRAFT rows default
--   LOCAL-ONLY (never leave the device); only user/policy-promoted (approved)
--   records become sync-eligible when Phase 2 ships. workspace_id DEFAULT
--   'local' must equal `context::LOCAL_WORKSPACE_ID` (src/context.rs); a fresh
--   row's `rev = 1` with `updated_by` NULL is the never-synced baseline.
--
-- DELIBERATE: NO foreign key on `source_chunk_id` -> transcripts(id).
--   Retranscription (`TranscriptRepository::replace_meeting_transcripts`)
--   DELETEs and re-INSERTs a meeting's segment rows. An ON DELETE CASCADE here
--   would silently destroy approved evidence links (approved blocks/action
--   items vanishing because the user retranscribed); RESTRICT would block
--   retranscription outright. Resolvability of `source_chunk_id` (and of the
--   per-block source_chunk_id values inside summaries.sections, which a SQL FK
--   could never cover anyway — they live in JSON) is therefore enforced at the
--   REPOSITORY layer instead, at write-time and at approve-time
--   (docs/CONTRACTS.md §4: nothing may be persisted as approved without a
--   resolvable source_chunk_id). `meeting_id`, by contrast, DOES cascade:
--   deleting a meeting removes its summary and action items exactly like its
--   transcripts (20250916100000 precedent).
--
-- Scope: ADDITIVE ONLY. Two brand-new tables + three indexes. No existing
--   table, column, row, or index is touched; no renames, drops, or type changes.
--
-- Idempotency: executed exactly once per database by the app's sqlx::migrate!
--   ledger (`_sqlx_migrations`; see database/manager.rs), and additionally
--   idempotent BY CONSTRUCTION at the SQL level: every statement is
--   CREATE TABLE IF NOT EXISTS / CREATE INDEX IF NOT EXISTS, so re-execution is
--   a no-op. Neither table name exists in any database lineage this app can
--   encounter (fresh 20250916100000 schema, any later version, or a legacy
--   `meeting_minutes.db` import — verified against backend/app/db.py).
--
-- Sync-compatibility note (docs/DATA_MODEL.md "SQLite <-> Postgres
--   compatibility rules"): purely ADDITIVE — brand-new tables only; no sync
--   protocol is live yet (client-only today). An OLDER client binary opening an
--   upgraded database keeps working untouched: it contains no SQL referencing
--   `summaries` or `action_items`, so it never reads or writes them, and the
--   legacy summary path it uses (`summary_processes.result` JSON) is unchanged.
--   A NEWER binary opening an older database applies this migration first via
--   the ledger. Nothing is renamed, dropped, or retyped, so no two-step
--   deprecation applies. When sync ships (Phase 2), both tables map 1:1 onto
--   the existing `SyncEntity::Summary` / `SyncEntity::ActionItem` entities.
--
-- DOWN (documented only - migrations are forward-only, per docs/CONVENTIONS.md):
--   1. DROP TABLE IF EXISTS action_items;
--      DROP TABLE IF EXISTS summaries;
--      (SQLite drops each table's indexes with it — the three idx_* below need
--      no separate DROP; nothing outside these two tables was created or
--      modified.)
--   2. Record the rollback as a NEW forward migration; never edit or delete
--      this file.

------------------------------------------------------------------------------
-- summaries (synced-class; ONE structured summary per meeting — UNIQUE meeting_id)
------------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS summaries (
    id            TEXT PRIMARY KEY,
    meeting_id    TEXT NOT NULL UNIQUE REFERENCES meetings(id) ON DELETE CASCADE,
    workspace_id  TEXT NOT NULL DEFAULT 'local',
    status        TEXT NOT NULL DEFAULT 'draft',
    model         TEXT,
    template_id   TEXT,
    sections      TEXT NOT NULL,
    generated_at  TEXT,
    approved_at   TEXT,
    approved_by   TEXT,
    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL,
    updated_by    TEXT,
    rev           INTEGER NOT NULL DEFAULT 1,
    deleted_at    TEXT
);

------------------------------------------------------------------------------
-- action_items (synced-class; many rows per meeting, ordered by `position`)
------------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS action_items (
    id              TEXT PRIMARY KEY,
    meeting_id      TEXT NOT NULL REFERENCES meetings(id) ON DELETE CASCADE,
    workspace_id    TEXT NOT NULL DEFAULT 'local',
    text            TEXT NOT NULL,
    assignee        TEXT,
    due             TEXT,
    status          TEXT NOT NULL DEFAULT 'draft',
    source_chunk_id TEXT NOT NULL,
    position        INTEGER NOT NULL DEFAULT 0,
    original_text   TEXT,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL,
    updated_by      TEXT,
    rev             INTEGER NOT NULL DEFAULT 1,
    deleted_at      TEXT
);

------------------------------------------------------------------------------
-- Workspace-scoped hot-path indexes (docs/CONVENTIONS.md: pragmatic indexes):
--   summary fetch:     WHERE workspace_id = ? AND meeting_id = ?
--   items per meeting: WHERE workspace_id = ? AND meeting_id = ? ORDER BY position
--   open-items views:  WHERE workspace_id = ? AND status = ?
------------------------------------------------------------------------------
CREATE INDEX IF NOT EXISTS idx_summaries_workspace_meeting    ON summaries(workspace_id, meeting_id);
CREATE INDEX IF NOT EXISTS idx_action_items_workspace_meeting ON action_items(workspace_id, meeting_id);
CREATE INDEX IF NOT EXISTS idx_action_items_workspace_status  ON action_items(workspace_id, status);
