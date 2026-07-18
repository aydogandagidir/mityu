-- Migration: the local learning system — `correction_events` + `learned_rules`,
-- a per-summary snapshot of the rules that shaped it, and the per-workspace
-- `learningConfig` blob (docs/DECISIONS.md ADR-0024).
--
-- Purpose (ADR-0024; CLAUDE.md §0.5 HITL, §0.6 privacy-is-architectural):
--   Mityu already CAPTURES the human-in-the-loop correction signal: every
--   AI-drafted block keeps `original_content` beside the human's `content`
--   (summaries.sections JSON), every action item keeps `original_text` beside
--   `text`. But no generation path has ever READ it — verified 2026-07-16:
--   `original_content`/`original_text` occur only in the repositories that write
--   them, the type definitions, the UI that displays them, and tests. The signal
--   is captured and inert; the gap is consumption, not capture.
--
--   These tables make that signal durable, minable, and — once mined into
--   plain-language rules — injectable back into the prompt, WITHOUT touching
--   model weights. Weights were rejected on legal grounds, not preference
--   (ADR-0024 context §6): KVKK/GDPR erasure is not satisfiable against a
--   fine-tune, and under BYOK we do not own the model anyway. Learning is DATA.
--
--   `correction_events` is APPEND-ONLY: one row per human HITL action
--   (edit / reject / approve / restore). It exists as its OWN table precisely
--   because `SummaryDraftRepository::upsert_draft` (summary_draft.rs:323-338)
--   rewrites `summaries.sections` WHOLESALE on every regeneration — which today
--   silently destroys every prior `original_content`, i.e. the learning signal
--   evaporates exactly when the user is iterating most. A separate table is not
--   a convenience here; it IS the fix.
--
--   `learned_rules` holds the derived rules in PLAIN LANGUAGE ("this user says
--   'takip', not 'aksiyon'") — not weights, not embeddings — so the user can
--   read, rewrite and delete each one. That is simultaneously the product
--   surface (ADR-0024 §9) and the KVKK/EU-AI-Act defence: an opaque "our AI
--   learns you" is precisely what cannot be defended.
--
-- Table classification (docs/CONTRACTS.md §7; docs/DATA_MODEL.md logical names):
--   correction_events -> correction_event  LOCAL-ONLY by policy; full common-column
--                                          set per CLAUDE.md §6
--   learned_rules     -> learned_rule      LOCAL-ONLY by policy; same set
--   Both carry the columns §6 mandates (workspace_id + created_at/updated_at +
--   updated_by/rev/deleted_at), but NEITHER gets a `SyncEntity` in this change and
--   neither may acquire one without a deliberate ADR: `correction_events` stores raw
--   MEETING CONTENT (the drafted text AND the human's rewrite of it), making it the
--   most sensitive table in this schema after `transcripts`. Phase-2 sync policy for
--   corrections and for rules is explicitly OUT OF SCOPE, recorded as owed in
--   ADR-0024 §11. workspace_id DEFAULT 'local' must equal `context::LOCAL_WORKSPACE_ID`
--   (src/context.rs); a fresh row's `rev = 1` with `updated_by` NULL is the
--   never-synced baseline, matching 20260706000000.
--
-- ERASURE — why `meeting_id` CASCADEs, and why rules deliberately do NOT
--   (ADR-0024 §10; docs/SECURITY_PRIVACY.md):
--   Deleting a meeting deletes the corrections derived from it, so "delete my data"
--   stays a real DELETE with no residue anywhere — the guarantee a fine-tune could
--   never make. The `learned_rules` rows derived from those events SURVIVE, on
--   purpose: a rule is an abstraction the human explicitly approved, holds no
--   transcript text, and is independently deletable by the user. Its `evidence` id
--   list may therefore dangle, and readers MUST degrade to "kanıt silindi" rather
--   than erroring. This asymmetry is the entire argument for data-over-weights:
--   an unwanted rule is one DELETE; an unwanted fine-tune is a retrain.
--
-- DELIBERATE: NO foreign key on `correction_events.subject_id`.
--   `subject_id` is polymorphic over `subject_kind`: for 'summary_block' it names a
--   block id that lives INSIDE summaries.sections JSON (a SQL FK could never cover
--   it), and for 'action_item' it names an action_items row that a later
--   regeneration may legitimately outlive — an append-only correction log must not
--   be rewritten by what happens to its subject afterwards. Resolvability is
--   therefore a REPOSITORY-layer concern, exactly the precedent 20260706000000 set
--   for `source_chunk_id`. `meeting_id`, by contrast, DOES cascade (see ERASURE).
--
-- APPEND-ONLY is a REPOSITORY-layer invariant, not a SQL one.
--   SQLite cannot express append-only without a trigger, and a trigger blocking
--   DELETE would fight the CASCADE that ERASURE above requires. CorrectionEventRepository
--   therefore exposes insert + read only; `updated_at`/`updated_by`/`rev`/`deleted_at`
--   exist solely to satisfy the §6 common-column contract and stay at their defaults.
--
-- Scope: ADDITIVE ONLY. Two brand-new tables, five indexes, two nullable columns.
--   No existing table, row, or index is touched; no renames, drops, or type changes.
--
-- Idempotency: executed exactly once per database by the app's sqlx::migrate! ledger
--   (`_sqlx_migrations`; see database/manager.rs). Additionally idempotent BY
--   CONSTRUCTION for the tables/indexes (CREATE ... IF NOT EXISTS). SQLite has no
--   `ADD COLUMN IF NOT EXISTS`, so the two ALTERs rest on the same argument
--   20260702000000 and 20260704000000 rest on: the names exist in NO database
--   lineage this app can encounter. VERIFIED 2026-07-16 — `correction_events`,
--   `learned_rules`, `applied_rules` and `learningConfig` return zero hits across all
--   13 files in this directory AND across the legacy `meeting_minutes.db` import
--   schema (backend/app/db.py, the DatabaseManager::new copy path).
--
-- Sync-compatibility note (docs/DATA_MODEL.md "SQLite <-> Postgres compatibility
--   rules"): purely ADDITIVE, and no sync protocol is live yet (client-only today).
--   An OLDER client binary opening an upgraded database keeps working: it contains no
--   SQL referencing either new table, `summaries` is written with explicit column
--   lists and read via `SELECT *` (sqlx `FromRow` ignores unmapped columns — the same
--   property 20260704000000 relies on for `settings`), so the two new columns are
--   inert to it. A NEWER binary opening an older database applies this migration
--   first via the ledger. Nothing is renamed, dropped, or retyped, so no two-step
--   deprecation applies.
--
-- DOWN (documented only — migrations are forward-only, per docs/CONVENTIONS.md):
--   1. DROP TABLE IF EXISTS correction_events;
--      DROP TABLE IF EXISTS learned_rules;
--      (SQLite drops each table's indexes with it — the five idx_* below need no
--      separate DROP.)
--   2. On SQLite >= 3.35: ALTER TABLE summaries DROP COLUMN applied_rules;
--                         ALTER TABLE settings  DROP COLUMN learningConfig;
--      On older SQLite: 12-step table rebuild inside PRAGMA foreign_keys=off.
--   3. Record the rollback as a NEW forward migration; never edit or delete this file.

------------------------------------------------------------------------------
-- correction_events (local-only; APPEND-ONLY log of every human HITL action)
--
--   subject_kind : 'summary_block' | 'action_item'
--   subject_id   : block id inside summaries.sections JSON, or action_items.id
--                  (polymorphic — see DELIBERATE above; no FK)
--   action       : 'edit' | 'reject' | 'approve' | 'restore'
--   original_text: the AI draft as it stood at the moment of the action
--   final_text   : the human's text after it (NULL for reject / bare approve)
--   reason       : optional free-text rationale. "This was wrong" is not a
--                  teachable signal; "wrong because X" is. Never blocks the action.
--   block_type / section_title / template_id / model:
--                  the generation context, captured HERE because the miner needs to
--                  scope a rule ("in Risks sections", "with the standup template")
--                  and because summaries.sections will not survive to tell it.
------------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS correction_events (
    id              TEXT PRIMARY KEY,
    workspace_id    TEXT NOT NULL DEFAULT 'local',
    meeting_id      TEXT NOT NULL REFERENCES meetings(id) ON DELETE CASCADE,
    subject_kind    TEXT NOT NULL,
    subject_id      TEXT NOT NULL,
    action          TEXT NOT NULL,
    original_text   TEXT,
    final_text      TEXT,
    reason          TEXT,
    block_type      TEXT,
    section_title   TEXT,
    template_id     TEXT,
    model           TEXT,
    source_chunk_id TEXT,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL,
    updated_by      TEXT,
    rev             INTEGER NOT NULL DEFAULT 1,
    deleted_at      TEXT
);

------------------------------------------------------------------------------
-- learned_rules (local-only; the derived, plain-language, user-owned rule set)
--
--   scope        : 'global' | 'template:<id>' | 'section:<title>'
--   kind         : 'term_substitution' | 'style' | 'section_preference' | 'freeform'
--   rule_text    : PLAIN LANGUAGE, injected verbatim into the prompt, user-editable
--   status       : 'proposed' | 'active' | 'dismissed'
--                  Mined rules are born 'proposed'. 'user_authored' rules are born
--                  'active' — the user writing the rule IS the approval. Auto-
--                  activation of mined rules is allowed ONLY behind
--                  learningConfig.autoActivate, and ONLY because the output HITL
--                  gate still requires per-block human approval (upsert_draft forces
--                  status='draft'; approve_summary demands every block approved) AND
--                  because summaries.applied_rules makes the result reproducible.
--   origin       : 'mined_deterministic' | 'mined_llm' | 'user_authored'
--   support_count: how many correction events back this rule (autoActivateMinSupport)
--   evidence     : JSON array of correction_events.id — MAY DANGLE after a meeting
--                  is deleted (see ERASURE); readers degrade, never error.
--   signature    : the MINER'S identity for this rule (learning/miner.rs), e.g.
--                  'term_substitution|global|aksiyon=>takip'. NULL for a rule the
--                  user wrote — nothing mined it, so nothing can re-propose it.
--
--                  It exists because `rule_text` CANNOT be the identity: rules are
--                  user-editable, so a miner matching on text would cheerfully
--                  re-propose a rule the moment its owner reworded it. Deliberately
--                  NOT UNIQUE — a signature must be free to recur across the
--                  soft-deleted rows it leaves behind, since deleting a rule is
--                  precisely how a user says "offer this to me again if I keep
--                  doing it" (dismissing is how they say "never again"). The
--                  miner filters on live rows instead.
------------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS learned_rules (
    id            TEXT PRIMARY KEY,
    workspace_id  TEXT NOT NULL DEFAULT 'local',
    scope         TEXT NOT NULL DEFAULT 'global',
    kind          TEXT NOT NULL,
    rule_text     TEXT NOT NULL,
    status        TEXT NOT NULL DEFAULT 'proposed',
    origin        TEXT NOT NULL,
    support_count INTEGER NOT NULL DEFAULT 0,
    evidence      TEXT,
    signature     TEXT,
    created_at    TEXT NOT NULL,
    activated_at  TEXT,
    activated_by  TEXT,
    updated_at    TEXT NOT NULL,
    updated_by    TEXT,
    rev           INTEGER NOT NULL DEFAULT 1,
    deleted_at    TEXT
);

------------------------------------------------------------------------------
-- summaries.applied_rules — the AI Act Art.50 reproducibility snapshot.
--   JSON array of { rule_id, rule_text, scope } captured AT GENERATION TIME.
--   The TEXT is stored, not just the id, deliberately: rules are user-editable and
--   deletable, so an id alone cannot reproduce how a six-month-old summary came to
--   read the way it does — and that reproducibility is the product's evidence claim
--   (CLAUDE.md §0.5). This column is the precondition that makes auto-activation
--   defensible.
--
--   `[]` AND NULL MEAN DIFFERENT THINGS, and the difference is the whole point of
--   an audit trail:
--     '[]'  = the learning system ran for this summary and found NOTHING to apply
--             ("no, and we checked");
--     NULL  = nothing was ever recorded here — a row generated before this
--             migration, the first-run sample meeting (database/sample_meeting.rs
--             inserts its own row directly), or a summary that came from the legacy
--             markdown path ("we do not know").
--   Hence: nullable, and NO DEFAULT. A DEFAULT '[]' would silently upgrade every
--   pre-existing row from "unknown" to "audited, nothing applied" — a claim this
--   migration is in no position to make.
------------------------------------------------------------------------------
ALTER TABLE summaries ADD COLUMN applied_rules TEXT;

------------------------------------------------------------------------------
-- settings.learningConfig — per-workspace learning policy, following the
--   `redactionConfig` precedent exactly (20260704000000): a small plain JSON blob,
--   NO secret, nullable, absent/NULL == Rust-side LearningConfig::default().
--   Holds e.g. { enabled, autoActivate, autoActivateMinSupport, llmMinerEnabled }.
--   `settings` already carries workspace_id (20260702000000), so every read/write of
--   this column is workspace-scoped.
------------------------------------------------------------------------------
ALTER TABLE settings ADD COLUMN learningConfig TEXT;

------------------------------------------------------------------------------
-- Workspace-scoped hot-path indexes (docs/CONVENTIONS.md: pragmatic indexes):
--   generation (hottest — every summary):  WHERE workspace_id = ? AND status = 'active'
--   miner scan / burden over time:         WHERE workspace_id = ? ORDER BY created_at
--   per-meeting evidence + cascade:        WHERE workspace_id = ? AND meeting_id = ?
--   miner by action:                       WHERE workspace_id = ? AND action = ?
--   scoped rule fetch:                     WHERE workspace_id = ? AND scope = ?
------------------------------------------------------------------------------
CREATE INDEX IF NOT EXISTS idx_correction_events_workspace_meeting ON correction_events(workspace_id, meeting_id);
CREATE INDEX IF NOT EXISTS idx_correction_events_workspace_created ON correction_events(workspace_id, created_at);
CREATE INDEX IF NOT EXISTS idx_correction_events_workspace_action  ON correction_events(workspace_id, action);
CREATE INDEX IF NOT EXISTS idx_learned_rules_workspace_status      ON learned_rules(workspace_id, status);
CREATE INDEX IF NOT EXISTS idx_learned_rules_workspace_scope       ON learned_rules(workspace_id, scope);
CREATE INDEX IF NOT EXISTS idx_learned_rules_workspace_signature   ON learned_rules(workspace_id, signature);
