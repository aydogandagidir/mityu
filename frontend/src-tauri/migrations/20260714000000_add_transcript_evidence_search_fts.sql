-- Migration: local transcript evidence search (Product Intelligence / MEM-01)
--
-- Purpose:
--   Replace the unranked LIKE scan in `TranscriptsRepository::search_transcripts`
--   with a derived SQLite FTS5 index. Every indexed document is one persisted
--   transcript segment, so every search hit resolves to a real `transcripts.id`
--   (`source_chunk_id`) and can jump to evidence in the transcript UI.
--
-- Trust boundary:
--   This first slice deliberately indexes ONLY transcript segments. Legacy
--   `summary_processes.result` and unapproved structured-summary content are not
--   indexed: retrieval must not turn an unreviewed AI claim into apparent
--   workspace knowledge. Approved summary blocks/action items can be promoted in
--   a later additive migration after the HITL defaults are hardened.
--
-- Storage / sync classification:
--   `transcript_search_documents` + `transcript_search_fts` are a local DERIVED
--   INDEX, not domain entities and not syncable. Both live inside the same
--   SQLCipher database as their source rows and can always be rebuilt.
--
-- Tenant isolation:
--   The indexed document map routes an FTS rowid back to a source segment.
--   Repository queries still join back to BOTH `transcripts` and `meetings` and
--   apply workspace and soft-delete predicates to every table; neither derived
--   table is ever an authority.
--
-- Compatibility:
--   Additive only for domain/sync schemas. Newer binaries opening an older DB
--   apply this migration and backfill active transcript rows. SQLx's default
--   migrator rejects a newer ledger version on application downgrade, so a DB
--   at version 20260714000000 requires this or a newer app build; downgrade is
--   not supported. No synced table/column is renamed, dropped or retyped.
--
-- Idempotency:
--   sqlx's migration ledger runs this once. The SQL is also safe to re-execute:
--   the virtual table/triggers use IF NOT EXISTS and the derived index is cleared
--   then deterministically rebuilt before triggers are installed.
--
-- DOWN (documented only; migrations are forward-only):
--   DROP TRIGGER IF EXISTS transcripts_search_fts_ai;
--   DROP TRIGGER IF EXISTS transcripts_search_fts_au;
--   DROP TRIGGER IF EXISTS transcripts_search_fts_ad;
--   DROP TRIGGER IF EXISTS meetings_search_fts_au;
--   DROP TRIGGER IF EXISTS meetings_search_fts_ad;
--   DROP TABLE IF EXISTS transcript_search_fts;
--   DROP INDEX IF EXISTS idx_transcript_search_documents_workspace_meeting;
--   DROP TABLE IF EXISTS transcript_search_documents;

-- FTS5 rowids are integers, while the domain segment id is TEXT. This explicit
-- mapping owns the synthetic integer document id and indexes both maintenance
-- paths. Ordinary transcript updates/deletes therefore never scan the FTS
-- corpus through UNINDEXED routing columns.
CREATE TABLE IF NOT EXISTS transcript_search_documents (
    id              INTEGER PRIMARY KEY,
    workspace_id    TEXT NOT NULL,
    meeting_id      TEXT NOT NULL,
    source_chunk_id TEXT NOT NULL,
    UNIQUE (workspace_id, source_chunk_id)
);

CREATE INDEX IF NOT EXISTS idx_transcript_search_documents_workspace_meeting
    ON transcript_search_documents (workspace_id, meeting_id);

CREATE VIRTUAL TABLE IF NOT EXISTS transcript_search_fts USING fts5(
    transcript,
    tokenize = 'unicode61 remove_diacritics 2'
);

-- A derived index is disposable. Clearing before backfill makes a manual/recovery
-- re-run deterministic instead of duplicating FTS rows.
DELETE FROM transcript_search_fts;
DELETE FROM transcript_search_documents;

INSERT INTO transcript_search_documents (
    workspace_id,
    meeting_id,
    source_chunk_id
)
SELECT
    t.workspace_id,
    t.meeting_id,
    t.id
FROM transcripts AS t
JOIN meetings AS m
  ON m.id = t.meeting_id
 AND m.workspace_id = t.workspace_id
WHERE t.deleted_at IS NULL
  AND m.deleted_at IS NULL
ORDER BY t.workspace_id, t.meeting_id, t.id;

INSERT INTO transcript_search_fts (rowid, transcript)
SELECT
    d.id,
    -- unicode61 does not fold Turkish I/İ/ı as users expect. Store a
    -- search-normalized copy while the authoritative/original text remains in
    -- `transcripts` and is used for UI snippets.
    LOWER(REPLACE(REPLACE(REPLACE(t.transcript, 'İ', 'i'), 'I', 'i'), 'ı', 'i'))
FROM transcript_search_documents AS d
JOIN transcripts AS t
  ON t.id = d.source_chunk_id
 AND t.meeting_id = d.meeting_id
 AND t.workspace_id = d.workspace_id;

-- Keep the derived index transactionally aligned with every transcript writer,
-- including record-save, import and retranscription (delete + insert).
CREATE TRIGGER IF NOT EXISTS transcripts_search_fts_ai
AFTER INSERT ON transcripts
WHEN NEW.deleted_at IS NULL
 AND EXISTS (
     SELECT 1
       FROM meetings AS m
      WHERE m.id = NEW.meeting_id
        AND m.workspace_id = NEW.workspace_id
        AND m.deleted_at IS NULL
 )
BEGIN
    INSERT INTO transcript_search_documents (
        workspace_id,
        meeting_id,
        source_chunk_id
    ) VALUES (
        NEW.workspace_id,
        NEW.meeting_id,
        NEW.id
    );

    INSERT INTO transcript_search_fts (rowid, transcript)
    SELECT
        d.id,
        LOWER(REPLACE(REPLACE(REPLACE(NEW.transcript, 'İ', 'i'), 'I', 'i'), 'ı', 'i'))
      FROM transcript_search_documents AS d
     WHERE d.workspace_id = NEW.workspace_id
       AND d.source_chunk_id = NEW.id;
END;

CREATE TRIGGER IF NOT EXISTS transcripts_search_fts_au
AFTER UPDATE OF id, workspace_id, meeting_id, transcript, deleted_at ON transcripts
BEGIN
    DELETE FROM transcript_search_fts
     WHERE rowid IN (
         SELECT d.id
           FROM transcript_search_documents AS d
          WHERE d.workspace_id = OLD.workspace_id
            AND d.source_chunk_id = OLD.id
     );

    DELETE FROM transcript_search_documents
     WHERE workspace_id = OLD.workspace_id
       AND source_chunk_id = OLD.id;

    INSERT INTO transcript_search_documents (
        workspace_id,
        meeting_id,
        source_chunk_id
    )
    SELECT
        NEW.workspace_id,
        NEW.meeting_id,
        NEW.id
    WHERE NEW.deleted_at IS NULL
      AND EXISTS (
          SELECT 1
            FROM meetings AS m
           WHERE m.id = NEW.meeting_id
             AND m.workspace_id = NEW.workspace_id
             AND m.deleted_at IS NULL
      );

    INSERT INTO transcript_search_fts (rowid, transcript)
    SELECT
        d.id,
        LOWER(REPLACE(REPLACE(REPLACE(NEW.transcript, 'İ', 'i'), 'I', 'i'), 'ı', 'i'))
      FROM transcript_search_documents AS d
     WHERE d.workspace_id = NEW.workspace_id
       AND d.source_chunk_id = NEW.id;
END;

-- A soft-deleted meeting is excluded at query time even if its derived rows are
-- stale. These meeting triggers additionally remove that stale copy and rebuild
-- it if a meeting is restored or moved between workspaces, so index coherence
-- does not depend on a future vacuum/rebuild job.
CREATE TRIGGER IF NOT EXISTS meetings_search_fts_au
AFTER UPDATE OF workspace_id, deleted_at ON meetings
BEGIN
    DELETE FROM transcript_search_fts
     WHERE rowid IN (
         SELECT d.id
           FROM transcript_search_documents AS d
          WHERE d.workspace_id = OLD.workspace_id
            AND d.meeting_id = OLD.id
     );

    DELETE FROM transcript_search_documents
     WHERE workspace_id = OLD.workspace_id
       AND meeting_id = OLD.id;

    INSERT INTO transcript_search_documents (
        workspace_id,
        meeting_id,
        source_chunk_id
    )
    SELECT
        t.workspace_id,
        t.meeting_id,
        t.id
      FROM transcripts AS t
     WHERE NEW.deleted_at IS NULL
       AND t.deleted_at IS NULL
       AND t.meeting_id = NEW.id
       AND t.workspace_id = NEW.workspace_id;

    INSERT INTO transcript_search_fts (rowid, transcript)
    SELECT
        d.id,
        LOWER(REPLACE(REPLACE(REPLACE(t.transcript, 'İ', 'i'), 'I', 'i'), 'ı', 'i'))
      FROM transcript_search_documents AS d
      JOIN transcripts AS t
        ON t.id = d.source_chunk_id
       AND t.meeting_id = d.meeting_id
       AND t.workspace_id = d.workspace_id
     WHERE d.workspace_id = NEW.workspace_id
       AND d.meeting_id = NEW.id;
END;

CREATE TRIGGER IF NOT EXISTS meetings_search_fts_ad
AFTER DELETE ON meetings
BEGIN
    DELETE FROM transcript_search_fts
     WHERE rowid IN (
         SELECT d.id
           FROM transcript_search_documents AS d
          WHERE d.workspace_id = OLD.workspace_id
            AND d.meeting_id = OLD.id
     );

    DELETE FROM transcript_search_documents
     WHERE workspace_id = OLD.workspace_id
       AND meeting_id = OLD.id;
END;

CREATE TRIGGER IF NOT EXISTS transcripts_search_fts_ad
AFTER DELETE ON transcripts
BEGIN
    DELETE FROM transcript_search_fts
     WHERE rowid IN (
         SELECT d.id
           FROM transcript_search_documents AS d
          WHERE d.workspace_id = OLD.workspace_id
            AND d.source_chunk_id = OLD.id
     );

    DELETE FROM transcript_search_documents
     WHERE workspace_id = OLD.workspace_id
       AND source_chunk_id = OLD.id;
END;
