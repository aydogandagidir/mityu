-- Migration: add workspace_id + sync columns to every domain table (BACKLOG B2, phase 1)
--
-- Purpose (docs/MULTITENANCY.md rule 1, docs/DATA_MODEL.md "Common columns"):
--   Every persisted domain entity carries `workspace_id` from day one; entities that
--   will sync in Phase 2 also carry `updated_by`, `rev`, `deleted_at` (soft delete).
--   Local-first mode has exactly one implicit workspace whose fixed id is 'local'
--   (must match `context::LOCAL_WORKSPACE_ID` in src/context.rs).
--
-- Scope: ADDITIVE ONLY. No renames, no drops, no type changes. Existing rows are
--   backfilled by the column DEFAULTs (workspace_id='local', rev=1) and by the
--   guarded UPDATEs below. Existing INSERT/SELECT call sites keep working unchanged:
--   all writers use explicit column lists and all readers map rows by column name.
--   Query call sites are NOT rewired here — that is B2 phase 2 (repositories).
--
-- Table classification (real client table names -> docs/DATA_MODEL.md logical names):
--   meetings           -> meeting            SYNCED   (workspace_id + updated_by/rev/deleted_at)
--   transcripts        -> transcript_chunk   SYNCED   (per-segment rows; gains created_at/updated_at too)
--   summary_processes  -> summary            SYNCED
--   transcript_chunks  -> transcript         SYNCED   (full-text-per-meeting; gains updated_at)
--   meeting_notes      -> summary_block-like SYNCED   (per-meeting user notes; meeting content)
--   settings           -> settings           PER-SCOPE, NOT SYNCED (holds plaintext key columns
--                                            today - legacy; secrets must never sync raw)
--   transcript_settings-> settings           PER-SCOPE, NOT SYNCED (same reason)
--   licensing          -> (none)             UNTOUCHED: device-scoped license-activation state,
--                                            not workspace domain data (deliberate decision)
--
-- Idempotency: this file is executed exactly once per database by the app's existing
--   migration runner (sqlx::migrate! ledger in `_sqlx_migrations`; see
--   database/manager.rs) - the same guarantee every prior ALTER-based migration in
--   this directory relies on. SQLite has no `ADD COLUMN IF NOT EXISTS` and plain SQL
--   cannot branch on PRAGMA table_info, so column-existence safety is instead proven
--   by construction: none of the column names added here exist in ANY database
--   lineage this app can encounter (fresh 20250916100000 schema, any later version,
--   or a legacy `meeting_minutes.db` copied by DatabaseManager::new - verified
--   against backend/app/db.py). The UPDATE backfills and CREATE INDEX statements are
--   additionally idempotent at the SQL level (WHERE ... IS NULL / IF NOT EXISTS).
--
-- Sync-compatibility note (docs/DATA_MODEL.md is updated in the same change):
--   Client-only today (no server, no sync protocol yet). All columns are additive
--   with constant defaults, so any older client binary opening an upgraded database
--   keeps working: its explicit-column INSERTs let the defaults apply and its
--   name-based row mapping ignores the new columns. An upgraded client opening an
--   older database simply applies this migration first. Nothing is renamed or
--   dropped, so no two-step deprecation is required.
--
-- DOWN (documented only - migrations are forward-only, per docs/CONVENTIONS.md):
--   1. DROP INDEX IF EXISTS idx_meetings_workspace_created;
--      DROP INDEX IF EXISTS idx_transcripts_workspace_meeting;
--      (must happen first: SQLite refuses to drop an indexed column)
--   2. On SQLite >= 3.35: ALTER TABLE <t> DROP COLUMN <c> for each column added
--      below, i.e. workspace_id/updated_by/rev/deleted_at on meetings, transcripts,
--      summary_processes, transcript_chunks, meeting_notes; created_at/updated_at on
--      transcripts; updated_at on transcript_chunks; workspace_id/created_at/
--      updated_at on settings and transcript_settings.
--      On older SQLite: 12-step table rebuild (create new table with the old shape,
--      INSERT ... SELECT the old columns, drop, rename) inside PRAGMA foreign_keys=off.
--   3. Record the rollback as a NEW forward migration; never edit or delete this file.

------------------------------------------------------------------------------
-- meetings (synced) - already has created_at/updated_at
------------------------------------------------------------------------------
ALTER TABLE meetings ADD COLUMN workspace_id TEXT NOT NULL DEFAULT 'local';
ALTER TABLE meetings ADD COLUMN updated_by TEXT;
ALTER TABLE meetings ADD COLUMN rev INTEGER NOT NULL DEFAULT 1;
ALTER TABLE meetings ADD COLUMN deleted_at TEXT;

------------------------------------------------------------------------------
-- transcripts (synced; one row per segment) - had NO created_at/updated_at.
-- New columns are nullable (SQLite cannot ADD COLUMN NOT NULL with a
-- non-constant default); backfill below, then phase-2 repositories set them
-- on every INSERT/UPDATE.
------------------------------------------------------------------------------
ALTER TABLE transcripts ADD COLUMN workspace_id TEXT NOT NULL DEFAULT 'local';
ALTER TABLE transcripts ADD COLUMN created_at TEXT;
ALTER TABLE transcripts ADD COLUMN updated_at TEXT;
ALTER TABLE transcripts ADD COLUMN updated_by TEXT;
ALTER TABLE transcripts ADD COLUMN rev INTEGER NOT NULL DEFAULT 1;
ALTER TABLE transcripts ADD COLUMN deleted_at TEXT;

-- Backfill: a segment was created during its meeting, so the parent meeting's
-- created_at is the truthful per-row creation time (the segment `timestamp`
-- column has no guaranteed format). Fall back to "now" (strict ISO-8601 UTC)
-- for orphaned rows. Idempotent via the IS NULL guards.
UPDATE transcripts
SET created_at = COALESCE(
        (SELECT m.created_at FROM meetings m WHERE m.id = transcripts.meeting_id),
        STRFTIME('%Y-%m-%dT%H:%M:%fZ', 'now'))
WHERE created_at IS NULL;

UPDATE transcripts SET updated_at = created_at WHERE updated_at IS NULL;

------------------------------------------------------------------------------
-- summary_processes (synced) - already has created_at/updated_at
------------------------------------------------------------------------------
ALTER TABLE summary_processes ADD COLUMN workspace_id TEXT NOT NULL DEFAULT 'local';
ALTER TABLE summary_processes ADD COLUMN updated_by TEXT;
ALTER TABLE summary_processes ADD COLUMN rev INTEGER NOT NULL DEFAULT 1;
ALTER TABLE summary_processes ADD COLUMN deleted_at TEXT;

------------------------------------------------------------------------------
-- transcript_chunks (synced) - had created_at but NO updated_at
------------------------------------------------------------------------------
ALTER TABLE transcript_chunks ADD COLUMN workspace_id TEXT NOT NULL DEFAULT 'local';
ALTER TABLE transcript_chunks ADD COLUMN updated_at TEXT;
ALTER TABLE transcript_chunks ADD COLUMN updated_by TEXT;
ALTER TABLE transcript_chunks ADD COLUMN rev INTEGER NOT NULL DEFAULT 1;
ALTER TABLE transcript_chunks ADD COLUMN deleted_at TEXT;

UPDATE transcript_chunks SET updated_at = created_at WHERE updated_at IS NULL;

------------------------------------------------------------------------------
-- meeting_notes (synced; per-meeting user notes) - already has created_at/updated_at
------------------------------------------------------------------------------
ALTER TABLE meeting_notes ADD COLUMN workspace_id TEXT NOT NULL DEFAULT 'local';
ALTER TABLE meeting_notes ADD COLUMN updated_by TEXT;
ALTER TABLE meeting_notes ADD COLUMN rev INTEGER NOT NULL DEFAULT 1;
ALTER TABLE meeting_notes ADD COLUMN deleted_at TEXT;

------------------------------------------------------------------------------
-- settings (per-workspace config; NOT synced - carries plaintext key columns
-- today, and secrets never sync raw). Gains workspace_id + timestamps only;
-- deliberately NO updated_by/rev/deleted_at.
------------------------------------------------------------------------------
ALTER TABLE settings ADD COLUMN workspace_id TEXT NOT NULL DEFAULT 'local';
ALTER TABLE settings ADD COLUMN created_at TEXT;
ALTER TABLE settings ADD COLUMN updated_at TEXT;

UPDATE settings
SET created_at = STRFTIME('%Y-%m-%dT%H:%M:%fZ', 'now')
WHERE created_at IS NULL;
UPDATE settings SET updated_at = created_at WHERE updated_at IS NULL;

------------------------------------------------------------------------------
-- transcript_settings (per-workspace config; NOT synced - same rationale)
------------------------------------------------------------------------------
ALTER TABLE transcript_settings ADD COLUMN workspace_id TEXT NOT NULL DEFAULT 'local';
ALTER TABLE transcript_settings ADD COLUMN created_at TEXT;
ALTER TABLE transcript_settings ADD COLUMN updated_at TEXT;

UPDATE transcript_settings
SET created_at = STRFTIME('%Y-%m-%dT%H:%M:%fZ', 'now')
WHERE created_at IS NULL;
UPDATE transcript_settings SET updated_at = created_at WHERE updated_at IS NULL;

------------------------------------------------------------------------------
-- Indexes for the phase-2 workspace-scoped hot paths only
-- (docs/CONVENTIONS.md: pragmatic indexes; the other domain tables are keyed
-- by meeting_id PRIMARY KEY - one row per meeting - so the PK already serves
-- their scoped point lookups):
--   meetings list:  WHERE workspace_id = ? ORDER BY created_at DESC
--   segments fetch: WHERE workspace_id = ? AND meeting_id = ?
------------------------------------------------------------------------------
CREATE INDEX IF NOT EXISTS idx_meetings_workspace_created
    ON meetings(workspace_id, created_at);
CREATE INDEX IF NOT EXISTS idx_transcripts_workspace_meeting
    ON transcripts(workspace_id, meeting_id);
