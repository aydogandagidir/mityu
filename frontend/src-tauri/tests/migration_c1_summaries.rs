//! Integration tests for migration `20260706000000_add_summaries_and_action_items`
//! (BACKLOG C1.2 — schema only; the summaries/action-items repositories are a
//! follow-up C1 step).
//!
//! Proves, using the app's real migration runner (`sqlx::migrate!` ledger):
//!   1. Fresh/empty DB: `summaries` and `action_items` exist with the EXACT
//!      expected column set/order, types, NOT-NULL flags and defaults
//!      (`workspace_id` `'local'` = `context::LOCAL_WORKSPACE_ID`, `status`
//!      `'draft'`, `rev` 1), plus all three workspace-scoped indexes; minimal
//!      explicit-column INSERTs prove the defaults and the one-summary-per-meeting
//!      UNIQUE constraint.
//!   2. Populated DB at the previous schema version: upgrading applies exactly the
//!      new migration, preserves every pre-existing row byte-identical, and
//!      creates the two new tables EMPTY.
//!   3. Re-running the full migrator is a no-op (forward-only, ledger-guarded).
//!   4. FK semantics: deleting a meeting CASCADEs its `summaries`/`action_items`
//!      rows; deleting a `transcripts` row leaves them UNTOUCHED — there is
//!      DELIBERATELY no FK on `source_chunk_id` (retranscription deletes and
//!      re-inserts segment rows, so a CASCADE would silently destroy approved
//!      evidence links; resolvability is enforced at the repository layer at
//!      write- and approve-time instead).

use sqlx::migrate::Migrator;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};
use std::path::Path;

use app_lib::context::LOCAL_WORKSPACE_ID;

/// The app's real, compile-time-embedded migration set (same source as
/// `DatabaseManager::new`).
static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

/// Version under test — must stay the maximum version in ./migrations until a
/// newer migration is added.
const NEW_VERSION: i64 = 20260706000000;

/// Exact expected `summaries` column order (PRAGMA table_info order =
/// declaration order in the migration).
///
/// `applied_rules` trails the original C1 set because `20260716000000`
/// (ADR-0030) appends it with `ALTER TABLE ... ADD COLUMN`, and SQLite puts an
/// added column last regardless of where it would read best.
const SUMMARIES_COLS: [&str; 16] = [
    "id",
    "meeting_id",
    "workspace_id",
    "status",
    "model",
    "template_id",
    "sections",
    "generated_at",
    "approved_at",
    "approved_by",
    "created_at",
    "updated_at",
    "updated_by",
    "rev",
    "deleted_at",
    "applied_rules",
];

/// Exact expected `action_items` column order.
const ACTION_ITEMS_COLS: [&str; 15] = [
    "id",
    "meeting_id",
    "workspace_id",
    "text",
    "assignee",
    "due",
    "status",
    "source_chunk_id",
    "position",
    "original_text",
    "created_at",
    "updated_at",
    "updated_by",
    "rev",
    "deleted_at",
];

/// A minimal but shape-correct `sections` JSON payload (the CONTRACTS §4
/// `Section`/`Block` shapes from `summary::draft`), anchored to a given chunk.
fn sections_json(chunk_id: &str) -> String {
    format!(
        r#"[{{"title":"Decisions","blocks":[{{"id":"b1","type":"text","content":"We agreed on the Q3 budget.","source_chunk_id":"{chunk_id}","status":"draft"}}]}}]"#
    )
}

#[derive(Debug)]
struct ColInfo {
    name: String,
    col_type: String,
    notnull: bool,
    dflt: Option<String>,
    pk: bool,
}

/// Open a temp DB with `foreign_keys(true)` — matching the real pool
/// (`database/manager.rs` sets `.foreign_keys(true)`), so the CASCADE proofs in
/// test 4 exercise exactly what the shipped app enforces.
async fn open_temp(path: &Path) -> SqlitePool {
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true)
        .foreign_keys(true);
    SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .expect("failed to open temp sqlite database")
}

async fn table_columns(pool: &SqlitePool, table: &str) -> Vec<ColInfo> {
    let rows = sqlx::query(&format!("PRAGMA table_info({table})"))
        .fetch_all(pool)
        .await
        .unwrap_or_else(|e| panic!("PRAGMA table_info({table}) failed: {e}"));
    rows.iter()
        .map(|row| ColInfo {
            name: row.get::<String, _>("name"),
            col_type: row.get::<String, _>("type"),
            notnull: row.get::<i64, _>("notnull") != 0,
            dflt: row.get::<Option<String>, _>("dflt_value"),
            pk: row.get::<i64, _>("pk") != 0,
        })
        .collect()
}

/// Assert one column's exact declared shape.
fn assert_col(
    table: &str,
    cols: &[ColInfo],
    name: &str,
    col_type: &str,
    notnull: bool,
    dflt: Option<&str>,
    pk: bool,
) {
    let c = cols
        .iter()
        .find(|c| c.name == name)
        .unwrap_or_else(|| panic!("expected column '{name}' in {table}: {cols:?}"));
    assert_eq!(c.col_type, col_type, "{table}.{name} type");
    assert_eq!(c.notnull, notnull, "{table}.{name} NOT NULL flag");
    assert_eq!(c.dflt.as_deref(), dflt, "{table}.{name} default");
    assert_eq!(c.pk, pk, "{table}.{name} primary-key flag");
}

async fn count(pool: &SqlitePool, table: &str) -> i64 {
    sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table}"))
        .fetch_one(pool)
        .await
        .unwrap_or_else(|e| panic!("COUNT(*) on {table} failed: {e}"))
}

async fn table_exists(pool: &SqlitePool, table: &str) -> bool {
    let n: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?")
            .bind(table)
            .fetch_one(pool)
            .await
            .expect("sqlite_master lookup");
    n > 0
}

async fn index_names(pool: &SqlitePool, table: &str) -> Vec<String> {
    let rows = sqlx::query(&format!("PRAGMA index_list({table})"))
        .fetch_all(pool)
        .await
        .unwrap_or_else(|e| panic!("PRAGMA index_list({table}) failed: {e}"));
    rows.iter().map(|r| r.get::<String, _>("name")).collect()
}

async fn applied_versions(pool: &SqlitePool) -> Vec<i64> {
    sqlx::query_scalar("SELECT version FROM _sqlx_migrations ORDER BY version")
        .fetch_all(pool)
        .await
        .expect("failed to read _sqlx_migrations ledger")
}

/// Assert the full declared shape of both new tables (exact column set, order,
/// types, NOT-NULL flags, defaults, PK).
async fn assert_new_tables_shape(pool: &SqlitePool) {
    let local_default = format!("'{LOCAL_WORKSPACE_ID}'");

    let cols = table_columns(pool, "summaries").await;
    let names: Vec<&str> = cols.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, SUMMARIES_COLS, "summaries exact column set/order");
    let t = "summaries";
    assert_col(t, &cols, "id", "TEXT", false, None, true);
    assert_col(t, &cols, "meeting_id", "TEXT", true, None, false);
    assert_col(
        t,
        &cols,
        "workspace_id",
        "TEXT",
        true,
        Some(&local_default),
        false,
    );
    assert_col(t, &cols, "status", "TEXT", true, Some("'draft'"), false);
    assert_col(t, &cols, "model", "TEXT", false, None, false);
    assert_col(t, &cols, "template_id", "TEXT", false, None, false);
    assert_col(t, &cols, "sections", "TEXT", true, None, false);
    assert_col(t, &cols, "generated_at", "TEXT", false, None, false);
    assert_col(t, &cols, "approved_at", "TEXT", false, None, false);
    assert_col(t, &cols, "approved_by", "TEXT", false, None, false);
    assert_col(t, &cols, "created_at", "TEXT", true, None, false);
    assert_col(t, &cols, "updated_at", "TEXT", true, None, false);
    assert_col(t, &cols, "updated_by", "TEXT", false, None, false);
    assert_col(t, &cols, "rev", "INTEGER", true, Some("1"), false);
    assert_col(t, &cols, "deleted_at", "TEXT", false, None, false);
    // ADR-0030 §5: nullable, no default — NULL means "generated before the
    // learning system, or with no active rules", which must stay distinguishable
    // from an empty snapshot.
    assert_col(t, &cols, "applied_rules", "TEXT", false, None, false);

    let cols = table_columns(pool, "action_items").await;
    let names: Vec<&str> = cols.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(
        names, ACTION_ITEMS_COLS,
        "action_items exact column set/order"
    );
    let t = "action_items";
    assert_col(t, &cols, "id", "TEXT", false, None, true);
    assert_col(t, &cols, "meeting_id", "TEXT", true, None, false);
    assert_col(
        t,
        &cols,
        "workspace_id",
        "TEXT",
        true,
        Some(&local_default),
        false,
    );
    assert_col(t, &cols, "text", "TEXT", true, None, false);
    assert_col(t, &cols, "assignee", "TEXT", false, None, false);
    assert_col(t, &cols, "due", "TEXT", false, None, false);
    assert_col(t, &cols, "status", "TEXT", true, Some("'draft'"), false);
    assert_col(t, &cols, "source_chunk_id", "TEXT", true, None, false);
    assert_col(t, &cols, "position", "INTEGER", true, Some("0"), false);
    assert_col(t, &cols, "original_text", "TEXT", false, None, false);
    assert_col(t, &cols, "created_at", "TEXT", true, None, false);
    assert_col(t, &cols, "updated_at", "TEXT", true, None, false);
    assert_col(t, &cols, "updated_by", "TEXT", false, None, false);
    assert_col(t, &cols, "rev", "INTEGER", true, Some("1"), false);
    assert_col(t, &cols, "deleted_at", "TEXT", false, None, false);
}

/// Seed two meetings + three transcript segments using the explicit column
/// lists the shipped app writes today (defaults fill the workspace/sync
/// columns). Meetings first: the pools here run with foreign_keys ON.
async fn seed_meetings_and_transcripts(pool: &SqlitePool) {
    sqlx::query(
        "INSERT INTO meetings (id, title, created_at, updated_at) \
         VALUES ('meeting-c1-1', 'Kickoff', '2026-07-01T09:00:00.000Z', \
                 '2026-07-01T09:30:00.000Z'), \
                ('meeting-c1-2', 'Site visit', '2026-07-02T14:00:00.000Z', \
                 '2026-07-02T15:00:00.000Z')",
    )
    .execute(pool)
    .await
    .expect("seed meetings");

    sqlx::query(
        "INSERT INTO transcripts \
         (id, meeting_id, transcript, timestamp, audio_start_time, audio_end_time, \
          duration, speaker) \
         VALUES \
         ('chunk-c1-1', 'meeting-c1-1', 'Hello world.', '2026-07-01T09:00:05.000Z', \
          0.0, 2.5, 2.5, 'microphone'), \
         ('chunk-c1-2', 'meeting-c1-1', 'Second segment.', '2026-07-01T09:00:08.000Z', \
          2.5, 5.0, 2.5, 'system'), \
         ('chunk-c1-3', 'meeting-c1-2', 'On site now.', '2026-07-02T14:00:03.000Z', \
          0.0, 1.8, 1.8, 'microphone')",
    )
    .execute(pool)
    .await
    .expect("seed transcripts");
}

/// Insert one summary + one action item for a meeting, anchored to a chunk.
async fn seed_summary_and_action_item(
    pool: &SqlitePool,
    suffix: &str,
    meeting_id: &str,
    chunk_id: &str,
) {
    sqlx::query(
        "INSERT INTO summaries (id, meeting_id, sections, created_at, updated_at) \
         VALUES (?, ?, ?, '2026-07-06T10:00:00.000Z', '2026-07-06T10:00:00.000Z')",
    )
    .bind(format!("summary-{suffix}"))
    .bind(meeting_id)
    .bind(sections_json(chunk_id))
    .execute(pool)
    .await
    .expect("seed summary");

    sqlx::query(
        "INSERT INTO action_items \
         (id, meeting_id, text, source_chunk_id, created_at, updated_at) \
         VALUES (?, ?, 'Ship the deck by Friday', ?, \
                 '2026-07-06T10:00:00.000Z', '2026-07-06T10:00:00.000Z')",
    )
    .bind(format!("action-{suffix}"))
    .bind(meeting_id)
    .bind(chunk_id)
    .execute(pool)
    .await
    .expect("seed action item");
}

/// Build a migrator containing only the migrations BEFORE the one under test, by
/// copying the prior .sql files into a temp dir and using the public runtime
/// resolver. This reproduces a user database at the previous schema version.
async fn prior_migrator(dir: &Path) -> Migrator {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
    let mut copied = 0usize;
    for entry in std::fs::read_dir(&source).expect("read migrations dir") {
        let entry = entry.expect("read migrations dir entry");
        let name = entry.file_name().to_string_lossy().to_string();
        let version: i64 = name
            .split('_')
            .next()
            .and_then(|v| v.parse().ok())
            .unwrap_or_else(|| panic!("unversioned file in migrations dir: {name}"));
        if version < NEW_VERSION {
            std::fs::copy(entry.path(), dir.join(&name)).expect("copy prior migration");
            copied += 1;
        }
    }
    let expected_prior = MIGRATOR.iter().filter(|m| m.version < NEW_VERSION).count();
    assert_eq!(
        copied, expected_prior,
        "prior-migration copy must match the embedded set"
    );
    assert_eq!(
        MIGRATOR.iter().filter(|m| m.version == NEW_VERSION).count(),
        1,
        "the C1 migration under test (NEW_VERSION) must exist exactly once"
    );
    Migrator::new(dir).await.expect("resolve prior migrations")
}

/// 1. Fresh DB: exact table shapes, all three indexes, defaults via a
/// legacy-free minimal INSERT, and the UNIQUE one-summary-per-meeting rule.
#[tokio::test]
async fn fresh_db_creates_summaries_and_action_items() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let db_path = tmp.path().join("fresh.sqlite");
    let pool = open_temp(&db_path).await;

    MIGRATOR.run(&pool).await.expect("migrations on fresh db");
    assert!(
        applied_versions(&pool).await.contains(&NEW_VERSION),
        "the C1 migration (NEW_VERSION) must be applied"
    );

    assert_new_tables_shape(&pool).await;

    // All three workspace-scoped indexes exist.
    let summaries_idx = index_names(&pool, "summaries").await;
    assert!(
        summaries_idx.contains(&"idx_summaries_workspace_meeting".to_string()),
        "summaries workspace index missing: {summaries_idx:?}"
    );
    let action_idx = index_names(&pool, "action_items").await;
    for expected in [
        "idx_action_items_workspace_meeting",
        "idx_action_items_workspace_status",
    ] {
        assert!(
            action_idx.contains(&expected.to_string()),
            "action_items index {expected} missing: {action_idx:?}"
        );
    }

    // Minimal explicit-column INSERTs let the defaults apply.
    seed_meetings_and_transcripts(&pool).await;
    seed_summary_and_action_item(&pool, "1", "meeting-c1-1", "chunk-c1-1").await;

    let s = sqlx::query(
        "SELECT workspace_id, status, rev, updated_by, deleted_at, approved_at, approved_by \
         FROM summaries WHERE id = 'summary-1'",
    )
    .fetch_one(&pool)
    .await
    .expect("read summary defaults");
    assert_eq!(s.get::<String, _>("workspace_id"), LOCAL_WORKSPACE_ID);
    assert_eq!(s.get::<String, _>("status"), "draft");
    assert_eq!(s.get::<i64, _>("rev"), 1);
    assert_eq!(s.get::<Option<String>, _>("updated_by"), None);
    assert_eq!(s.get::<Option<String>, _>("deleted_at"), None);
    assert_eq!(s.get::<Option<String>, _>("approved_at"), None);
    assert_eq!(s.get::<Option<String>, _>("approved_by"), None);

    let a = sqlx::query(
        "SELECT workspace_id, status, rev, position, assignee, due, original_text, \
                updated_by, deleted_at \
         FROM action_items WHERE id = 'action-1'",
    )
    .fetch_one(&pool)
    .await
    .expect("read action item defaults");
    assert_eq!(a.get::<String, _>("workspace_id"), LOCAL_WORKSPACE_ID);
    assert_eq!(a.get::<String, _>("status"), "draft");
    assert_eq!(a.get::<i64, _>("rev"), 1);
    assert_eq!(a.get::<i64, _>("position"), 0);
    assert_eq!(a.get::<Option<String>, _>("assignee"), None);
    assert_eq!(a.get::<Option<String>, _>("due"), None);
    assert_eq!(a.get::<Option<String>, _>("original_text"), None);
    assert_eq!(a.get::<Option<String>, _>("updated_by"), None);
    assert_eq!(a.get::<Option<String>, _>("deleted_at"), None);

    // ONE summary per meeting: a second row for the same meeting must fail
    // (UNIQUE meeting_id).
    let dup = sqlx::query(
        "INSERT INTO summaries (id, meeting_id, sections, created_at, updated_at) \
         VALUES ('summary-dup', 'meeting-c1-1', '[]', \
                 '2026-07-06T11:00:00.000Z', '2026-07-06T11:00:00.000Z')",
    )
    .execute(&pool)
    .await;
    assert!(
        dup.is_err(),
        "a second summary for the same meeting must violate UNIQUE(meeting_id)"
    );

    pool.close().await;
}

/// 2. Populated DB at the previous schema version: only the new migration
/// applies, pre-existing rows are preserved byte-identical, and the new tables
/// are created EMPTY.
#[tokio::test]
async fn populated_db_upgrade_creates_empty_tables_and_preserves_data() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let db_path = tmp.path().join("populated.sqlite");
    let prior_dir = tmp.path().join("prior_migrations");
    std::fs::create_dir(&prior_dir).expect("mkdir prior_migrations");

    let pool = open_temp(&db_path).await;

    // 1) Bring the DB to the PREVIOUS schema version and populate it.
    let prior = prior_migrator(&prior_dir).await;
    prior.run(&pool).await.expect("prior migrations");
    assert!(
        !table_exists(&pool, "summaries").await && !table_exists(&pool, "action_items").await,
        "precondition: the new tables must not exist at the prior version"
    );
    seed_meetings_and_transcripts(&pool).await;

    let pre_meetings = count(&pool, "meetings").await;
    let pre_transcripts = count(&pool, "transcripts").await;
    let pre_applied = applied_versions(&pool).await;

    // 2) Upgrade with the real runner: exactly the new migration applies.
    MIGRATOR.run(&pool).await.expect("upgrade migration");
    let post_applied = applied_versions(&pool).await;
    let new_migrations = MIGRATOR.iter().filter(|m| m.version >= NEW_VERSION).count();
    assert_eq!(
        post_applied.len(),
        pre_applied.len() + new_migrations,
        "all migrations at/after NEW_VERSION apply on top of the priors"
    );
    assert!(
        post_applied.contains(&NEW_VERSION),
        "the C1 migration (NEW_VERSION) must be applied"
    );

    // 3) Pre-existing row counts preserved; values byte-identical.
    assert_eq!(count(&pool, "meetings").await, pre_meetings);
    assert_eq!(count(&pool, "transcripts").await, pre_transcripts);

    let m =
        sqlx::query("SELECT title, created_at, updated_at FROM meetings WHERE id = 'meeting-c1-2'")
            .fetch_one(&pool)
            .await
            .expect("meeting row");
    assert_eq!(m.get::<String, _>("title"), "Site visit");
    assert_eq!(m.get::<String, _>("created_at"), "2026-07-02T14:00:00.000Z");
    assert_eq!(m.get::<String, _>("updated_at"), "2026-07-02T15:00:00.000Z");

    let t = sqlx::query(
        "SELECT meeting_id, transcript, timestamp, speaker FROM transcripts \
         WHERE id = 'chunk-c1-2'",
    )
    .fetch_one(&pool)
    .await
    .expect("transcript row");
    assert_eq!(t.get::<String, _>("meeting_id"), "meeting-c1-1");
    assert_eq!(t.get::<String, _>("transcript"), "Second segment.");
    assert_eq!(t.get::<String, _>("timestamp"), "2026-07-01T09:00:08.000Z");
    assert_eq!(
        t.get::<Option<String>, _>("speaker").as_deref(),
        Some("system")
    );

    // 4) The new tables exist with the full expected shape — and are EMPTY
    //    (the migration backfills nothing; drafts are generated, never invented).
    assert_new_tables_shape(&pool).await;
    assert_eq!(
        count(&pool, "summaries").await,
        0,
        "summaries must be empty"
    );
    assert_eq!(
        count(&pool, "action_items").await,
        0,
        "action_items must be empty"
    );

    pool.close().await;
}

/// 3. Re-running the full migrator is a no-op: the ledger is unchanged and rows
/// inserted into the new tables between runs survive untouched.
#[tokio::test]
async fn rerunning_full_migrator_is_a_noop() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let db_path = tmp.path().join("rerun.sqlite");
    let pool = open_temp(&db_path).await;

    MIGRATOR.run(&pool).await.expect("first run");
    seed_meetings_and_transcripts(&pool).await;
    seed_summary_and_action_item(&pool, "rerun", "meeting-c1-1", "chunk-c1-1").await;

    let versions_before = applied_versions(&pool).await;
    let summaries_before = count(&pool, "summaries").await;
    let actions_before = count(&pool, "action_items").await;

    MIGRATOR
        .run(&pool)
        .await
        .expect("second run (must be a no-op)");

    assert_eq!(
        applied_versions(&pool).await,
        versions_before,
        "ledger changed on re-run"
    );
    assert_eq!(count(&pool, "summaries").await, summaries_before);
    assert_eq!(count(&pool, "action_items").await, actions_before);

    let row =
        sqlx::query("SELECT workspace_id, status, rev FROM summaries WHERE id = 'summary-rerun'")
            .fetch_one(&pool)
            .await
            .expect("summary row after re-run");
    assert_eq!(row.get::<String, _>("workspace_id"), LOCAL_WORKSPACE_ID);
    assert_eq!(row.get::<String, _>("status"), "draft");
    assert_eq!(row.get::<i64, _>("rev"), 1);

    pool.close().await;
}

/// 4. FK semantics: `meeting_id` CASCADEs (deleting a meeting removes its
/// summary + action items); `source_chunk_id` deliberately does NOT (deleting a
/// transcript segment — what retranscription does — leaves summaries and action
/// items untouched).
#[tokio::test]
async fn meeting_delete_cascades_but_transcript_delete_does_not() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let db_path = tmp.path().join("fk.sqlite");
    let pool = open_temp(&db_path).await;

    MIGRATOR.run(&pool).await.expect("migrations");

    // Precondition: FK enforcement is ON for this pool (as in the real app pool).
    let fk_on: i64 = sqlx::query_scalar("PRAGMA foreign_keys")
        .fetch_one(&pool)
        .await
        .expect("PRAGMA foreign_keys");
    assert_eq!(fk_on, 1, "foreign_keys must be ON for the CASCADE proof");

    seed_meetings_and_transcripts(&pool).await;
    // meeting-c1-1: one summary + two action items (both anchored to its chunks).
    seed_summary_and_action_item(&pool, "fk-1", "meeting-c1-1", "chunk-c1-1").await;
    sqlx::query(
        "INSERT INTO action_items \
         (id, meeting_id, text, source_chunk_id, position, created_at, updated_at) \
         VALUES ('action-fk-2', 'meeting-c1-1', 'Send the notes', 'chunk-c1-2', 1, \
                 '2026-07-06T10:00:00.000Z', '2026-07-06T10:00:00.000Z')",
    )
    .execute(&pool)
    .await
    .expect("seed second action item");
    // meeting-c1-2: one summary + one action item anchored to chunk-c1-3.
    seed_summary_and_action_item(&pool, "fk-3", "meeting-c1-2", "chunk-c1-3").await;

    assert_eq!(count(&pool, "summaries").await, 2);
    assert_eq!(count(&pool, "action_items").await, 3);

    // Deleting a MEETING cascades its summary + action items (and its
    // transcripts, per the 20250916100000 FK) — the other meeting's rows stay.
    sqlx::query("DELETE FROM meetings WHERE id = 'meeting-c1-1'")
        .execute(&pool)
        .await
        .expect("delete meeting");
    assert_eq!(
        count(&pool, "summaries").await,
        1,
        "meeting delete must cascade its summary"
    );
    assert_eq!(
        count(&pool, "action_items").await,
        1,
        "meeting delete must cascade its action items"
    );
    let survivor: String = sqlx::query_scalar("SELECT id FROM summaries")
        .fetch_one(&pool)
        .await
        .expect("surviving summary");
    assert_eq!(survivor, "summary-fk-3");

    // Deleting a TRANSCRIPT row (what retranscription does under the hood) must
    // NOT touch summaries/action_items: there is deliberately no FK on
    // source_chunk_id, so approved evidence links are never silently destroyed.
    sqlx::query("DELETE FROM transcripts WHERE id = 'chunk-c1-3'")
        .execute(&pool)
        .await
        .expect("delete transcript segment");
    assert_eq!(
        count(&pool, "summaries").await,
        1,
        "transcript delete must not touch summaries"
    );
    assert_eq!(
        count(&pool, "action_items").await,
        1,
        "transcript delete must not touch action_items"
    );
    let anchor: String =
        sqlx::query_scalar("SELECT source_chunk_id FROM action_items WHERE id = 'action-fk-3'")
            .fetch_one(&pool)
            .await
            .expect("surviving action item");
    assert_eq!(
        anchor, "chunk-c1-3",
        "the evidence anchor value must survive the segment delete verbatim"
    );

    pool.close().await;
}
