-- Migration: give diarization a place to store TIMED SPEAKER TURNS, and record
-- whether a diarization pass has run for a meeting (docs/DECISIONS.md ADR-0034).
--
-- Purpose (ADR-0034; DESIGN_READAI Phase E):
--   Diarization is the most visible parity gap -- every competitor labels
--   speakers. This is step (b) of the ADR's four-step order and is DELIBERATELY
--   schema-only: `CLAUDE.md` §10 forbids changing the audio pipeline and the DB
--   schema in one change, and diarization needs both. Nothing populates
--   anything here yet.
--
-- WHY TURNS AND NOT A LABEL ON THE TRANSCRIPT ROW
--   The first draft of this migration re-purposed the dead `transcripts.speaker`
--   column to hold one anonymous label per transcript row. That is WRONG, and
--   the code says so:
--     * A transcript row is one VAD speech segment, and VAD is deliberately
--       tuned to BRIDGE pauses rather than cut at them: the import and
--       retranscription paths pass `VAD_REDEMPTION_TIME_MS = 2000`
--       (`audio/import.rs:57`, `audio/retranscription.rs:52`). A two-second
--       bridge is longer than the gap between two people taking turns, so one
--       row routinely spans more than one speaker.
--     * Rows are not even bounded to a conversational turn: `audio/import.rs`
--       counts segments longer than 25 s as a normal case ("Segments >25s (would
--       be split)").
--     * Per-speaker talk-time -- the number the feature exists to show -- is a
--       sum of DURATIONS. Row labels cannot produce it without assuming one
--       speaker per row, which is the same false assumption.
--   Storing one label per row would therefore mis-attribute speech and quietly
--   produce a wrong talk-time chart. Speaker identity is a property of a TIME
--   RANGE, so it is stored as time ranges.
--
--   `transcripts.speaker` (added 2025-11-10 by 20251110000001_add_speaker_field.sql,
--   inherited from upstream Meetily) therefore STAYS DEAD. It is not read, not
--   written, and is NOT the diarization surface. It is left in place rather than
--   dropped because dropping a column is not additive and this migration touches
--   a synced table's neighbours; a future cleanup can remove it on its own.
--
-- WHAT A TURN HOLDS
--   An ANONYMOUS label -- `Speaker 1`, `Speaker 2` -- over a millisecond range.
--   Never a real name. Attaching a real identity is a manual human action;
--   Mityu performs no voice-based identification (ADR-0034 decision 4,
--   DESIGN_READAI guardrail 3). Keeping the purpose away from "unique
--   identification" is also what keeps this out of biometric special-category
--   processing under KVKK m.6 / GDPR Art. 9 -- see docs/legal/A5_KVKK_PACK.md
--   §4.1, which draws the same distinction for the eval recordings.
--
--   Times are INTEGER milliseconds from the start of the recording, matching the
--   unit VAD already produces (`SpeechSegment.start_timestamp_ms`), so rendering
--   a turn against a transcript row is an overlap test in one unit with no
--   conversion.
--
-- WHY `meetings.diarized_at` IS NEEDED IN ADDITION
--   An empty turn list cannot distinguish "diarization never ran" from
--   "diarization ran and found nothing it could separate". Those must be told
--   apart or the UI lies: the first should offer to run a pass, the second
--   should report an inconclusive result. Same rule the inference capability
--   probe follows for NPUs (ADR-0033) -- "could not tell" is not "there is none".
--
-- AVAILABILITY IS NOT STORED (deliberate)
--   Diarization is post-hoc and needs the saved audio, but saved audio is
--   OPTIONAL: `RecordingSaver::start_accumulation(false)` discards every mixed
--   chunk (`audio/recording_saver.rs:236-249`) and `stop_and_save` then returns
--   without producing a file (`:434-440`). A transcripts-only meeting can never
--   be diarized, and the retention goal in ADR-0027 (delete audio after
--   transcription) means audio can also disappear LATER.
--   So availability is a question about the filesystem right now, not a fact
--   about the row, and a stored flag would go stale the moment audio is deleted.
--   It is probed at run time with the same helper retranscription already uses
--   for the identical problem (`find_audio_file`, `audio/retranscription.rs:149`).
--   Resulting UI states: no audio -> "not available for this recording";
--   audio + NULL `diarized_at` -> offer to run; non-NULL with no turns ->
--   inconclusive; non-NULL with turns -> show them.
--
-- Sync compatibility (docs/DATA_MODEL.md; ADR-0012):
--   `speaker_turns` is NOT a sync entity. ADR-0012 pins the synced set to
--   meeting/transcript/summary/action_item, and turns are DERIVED data that a
--   peer can regenerate from audio it may not even have. It therefore carries no
--   `rev`/`updated_by`/`deleted_at`; per `CLAUDE.md` §6 those columns are the
--   "for sync" set. Syncing turns later would require amending ADR-0012 and is
--   listed there as an open question. `workspace_id` is present regardless --
--   tenant scoping is unconditional (CLAUDE.md prime directive 3), and every
--   read goes through a workspace-scoped repository.
--   `meetings` IS synced, so the additive-column discipline applies:
--     * `diarized_at` is nullable with no default, so existing rows are
--       untouched and no backfill is required or performed.
--     * `sync/protocol.rs` carries an entity's fields as an OPAQUE
--       `serde_json::Value` payload and does not enumerate them, so a peer on an
--       older build simply omits the key and a newer build reads its absence as
--       NULL. No protocol version bump is needed.
--     * Additive only. A future RENAME or DROP would need the documented
--       two-step (add new -> dual-write -> migrate -> drop old), because a
--       synced peer may still be writing the old name.
--   No `rev`/`updated_by` change is made to `meetings`: adding a column is not a
--   row edit, and bumping every row's `rev` would make every existing meeting
--   look freshly modified to a sync peer.
--
-- Idempotency: executed exactly once per database by the app's sqlx migrator,
--   which records applied versions. The table and indexes additionally use
--   IF NOT EXISTS. SQLite has no `ADD COLUMN IF NOT EXISTS`, so the ALTER is
--   plain, exactly as 20260702000000 does.
--
-- Down (documented, not automated -- forward-only, CLAUDE.md §6):
--   DROP TABLE speaker_turns;
--   On SQLite >= 3.35:  ALTER TABLE meetings DROP COLUMN diarized_at;
--   Older engines require the 12-step table rebuild. `transcripts.speaker` is
--   left untouched by a down: it predates this migration.

------------------------------------------------------------------------------
-- speaker_turns (local-derived; many rows per meeting, ordered by start_ms)
--
-- One row = one contiguous stretch of audio attributed to one anonymous
-- speaker. Turns may be adjacent; overlapping turns are allowed because people
-- talk over each other and a diarizer can report that honestly.
------------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS speaker_turns (
    id            TEXT PRIMARY KEY,
    meeting_id    TEXT NOT NULL REFERENCES meetings(id) ON DELETE CASCADE,
    workspace_id  TEXT NOT NULL DEFAULT 'local',
    -- Anonymous label only, e.g. 'Speaker 1'. Never a person's name.
    speaker_label TEXT NOT NULL,
    -- Milliseconds from the start of the recording; same unit as VAD emits.
    start_ms      INTEGER NOT NULL,
    end_ms        INTEGER NOT NULL,
    -- Diarizer confidence in [0,1] when the engine reports one, else NULL.
    -- NULL means "not reported", which is not the same as "low".
    confidence    REAL,
    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL
);

------------------------------------------------------------------------------
-- Workspace-scoped hot-path index (docs/CONVENTIONS.md: pragmatic indexes):
--   render + talk-time: WHERE workspace_id = ? AND meeting_id = ? ORDER BY start_ms
-- One index covers both, since talk-time sums the same rows it renders.
------------------------------------------------------------------------------
CREATE INDEX IF NOT EXISTS idx_speaker_turns_workspace_meeting
    ON speaker_turns(workspace_id, meeting_id, start_ms);

-- When a diarization pass last completed for this meeting (RFC 3339).
-- NULL means no pass has ever completed, which is NOT the same as "no speakers
-- were found" -- see the availability note above for the four UI states.
ALTER TABLE meetings ADD COLUMN diarized_at TEXT;
