-- Migration: verifiable local deletion maintenance (BACKLOG C6a / ADR-0026)
--
-- SQLite ordinary tables and FTS5 use different secure-deletion controls.
-- `PRAGMA secure_delete=ON` is applied to every runtime connection in
-- database/manager.rs; this persistent FTS5 setting prevents deleted index
-- terms from being retained in merge segments. The local maintenance marker
-- makes the one-time historical compaction and every later meeting deletion
-- crash-resumable without storing meeting content or identifiers.
--
-- This table is local operational metadata, not a domain entity and not
-- syncable. It therefore deliberately has no workspace/sync columns.
--
-- Compatibility: FTS5 secure-delete requires SQLite >= 3.42. Mityu's bundled
-- SQLCipher/SQLite is 3.45.3. Downgrade is unsupported by the SQLx migration
-- ledger. A documented rollback would set secure-delete to 0, rebuild the FTS
-- index for the older format, and drop local_privacy_maintenance in a NEW
-- forward migration; applied migrations are never edited.

INSERT INTO transcript_search_fts(transcript_search_fts, rank)
VALUES('secure-delete', 1);

CREATE TABLE IF NOT EXISTS local_privacy_maintenance (
    singleton    INTEGER PRIMARY KEY CHECK (singleton = 1),
    required     INTEGER NOT NULL CHECK (required IN (0, 1)),
    completed_at TEXT
);

-- New and upgraded databases both receive one checked optimize/checkpoint/
-- VACUUM cycle after migrations. The manager changes required to 0 only after
-- the cycle succeeds; a crash leaves this row pending for the next launch.
INSERT INTO local_privacy_maintenance (singleton, required, completed_at)
VALUES (1, 1, NULL)
ON CONFLICT(singleton) DO NOTHING;
