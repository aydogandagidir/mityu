//! Integration tests for migration `20260702000000_add_workspace_and_sync_columns`
//! (BACKLOG B2 phase 1 — schema only; repository rewiring is phase 2).
//!
//! Proves, using the app's real migration runner (`sqlx::migrate!` ledger):
//!   1. Fresh/empty DB: full schema is created; every domain table carries
//!      `workspace_id` (default `'local'` = `context::LOCAL_WORKSPACE_ID`) and the
//!      synced tables carry `updated_by`/`rev`/`deleted_at`.
//!   2. Populated DB at the previous schema version: upgrading applies exactly the
//!      new migration, keeps row counts, backfills workspace/sync/timestamp columns
//!      and leaves every pre-existing value byte-identical.
//!   3. Re-running the full migrator is a no-op (forward-only, ledger-guarded).
//!   4. (env-gated) The same populated-upgrade proof against a COPY of a real
//!      database: set `MITYU_TEST_POPULATED_DB` to the copy's path. Never point it
//!      at the live file.

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
const NEW_VERSION: i64 = 20260702000000;

/// Tables that sync in Phase 2 → full common-column set.
const SYNCED_TABLES: [&str; 5] = [
    "meetings",
    "transcripts",
    "summary_processes",
    "transcript_chunks",
    "meeting_notes",
];

/// Per-workspace config tables → `workspace_id` + timestamps, but NO sync columns.
const CONFIG_TABLES: [&str; 2] = ["settings", "transcript_settings"];

#[derive(Debug)]
struct ColInfo {
    name: String,
    col_type: String,
    notnull: bool,
    dflt: Option<String>,
}

async fn open_temp(path: &Path) -> SqlitePool {
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true);
    SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .expect("failed to open temp sqlite database")
}

async fn open_existing(path: &Path) -> SqlitePool {
    let options = SqliteConnectOptions::new().filename(path);
    SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .expect("failed to open existing sqlite database")
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
        })
        .collect()
}

fn col<'a>(cols: &'a [ColInfo], name: &str) -> &'a ColInfo {
    cols.iter()
        .find(|c| c.name == name)
        .unwrap_or_else(|| panic!("expected column '{name}' in {cols:?}"))
}

fn has_col(cols: &[ColInfo], name: &str) -> bool {
    cols.iter().any(|c| c.name == name)
}

async fn count(pool: &SqlitePool, table: &str) -> i64 {
    sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table}"))
        .fetch_one(pool)
        .await
        .unwrap_or_else(|e| panic!("COUNT(*) on {table} failed: {e}"))
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

/// Assert the common workspace/sync column shape added by the migration.
async fn assert_synced_shape(pool: &SqlitePool, table: &str) {
    let cols = table_columns(pool, table).await;

    let ws = col(&cols, "workspace_id");
    assert_eq!(ws.col_type, "TEXT", "{table}.workspace_id type");
    assert!(ws.notnull, "{table}.workspace_id must be NOT NULL");
    assert_eq!(
        ws.dflt.as_deref(),
        Some("'local'"),
        "{table}.workspace_id default must be 'local'"
    );

    let rev = col(&cols, "rev");
    assert_eq!(rev.col_type, "INTEGER", "{table}.rev type");
    assert!(rev.notnull, "{table}.rev must be NOT NULL");
    assert_eq!(
        rev.dflt.as_deref(),
        Some("1"),
        "{table}.rev default must be 1"
    );

    assert!(
        !col(&cols, "updated_by").notnull,
        "{table}.updated_by nullable"
    );
    assert!(
        !col(&cols, "deleted_at").notnull,
        "{table}.deleted_at nullable"
    );

    // DATA_MODEL common columns: every domain entity has created_at + updated_at.
    assert!(has_col(&cols, "created_at"), "{table} must have created_at");
    assert!(has_col(&cols, "updated_at"), "{table} must have updated_at");
}

/// Count rows still carrying pristine local-workspace sync state.
async fn count_local_pristine(pool: &SqlitePool, table: &str) -> i64 {
    sqlx::query_scalar(&format!(
        "SELECT COUNT(*) FROM {table} \
         WHERE workspace_id = 'local' AND rev = 1 \
           AND deleted_at IS NULL AND updated_by IS NULL"
    ))
    .fetch_one(pool)
    .await
    .unwrap_or_else(|e| panic!("pristine-count on {table} failed: {e}"))
}

/// Seed one realistic row per table using the PRE-migration column lists
/// (exactly what the shipped app writes today).
async fn seed_pre_migration_data(pool: &SqlitePool) {
    sqlx::query(
        "INSERT INTO meetings (id, title, created_at, updated_at, folder_path) \
         VALUES ('meeting-legacy-1', 'Kickoff', '2026-01-05T09:00:00.000Z', \
                 '2026-01-05T09:30:00.000Z', NULL), \
                ('meeting-legacy-2', 'Site visit', '2026-02-01T14:00:00.000Z', \
                 '2026-02-01T15:00:00.000Z', 'C:/recordings/site-visit')",
    )
    .execute(pool)
    .await
    .expect("seed meetings");

    sqlx::query(
        "INSERT INTO transcripts \
         (id, meeting_id, transcript, timestamp, summary, action_items, key_points, \
          audio_start_time, audio_end_time, duration, speaker) \
         VALUES \
         ('transcript-legacy-1', 'meeting-legacy-1', 'Hello world.', \
          '2026-01-05T09:00:05.000Z', NULL, NULL, NULL, 0.0, 2.5, 2.5, 'microphone'), \
         ('transcript-legacy-2', 'meeting-legacy-1', 'Second segment.', \
          '2026-01-05T09:00:08.000Z', 'a summary', '[]', 'points', 2.5, 5.0, 2.5, 'system'), \
         ('transcript-legacy-3', 'meeting-legacy-2', 'On site now.', \
          '2026-02-01T14:00:03.000Z', NULL, NULL, NULL, 0.0, 1.8, 1.8, 'microphone')",
    )
    .execute(pool)
    .await
    .expect("seed transcripts");

    sqlx::query(
        "INSERT INTO summary_processes \
         (meeting_id, status, created_at, updated_at, error, result, start_time, \
          end_time, chunk_count, processing_time, metadata) \
         VALUES ('meeting-legacy-1', 'COMPLETED', '2026-01-05T09:31:00.000Z', \
                 '2026-01-05T09:32:00.000Z', NULL, '{\"MeetingName\":\"Kickoff\"}', \
                 '2026-01-05T09:31:00.000Z', '2026-01-05T09:32:00.000Z', 2, 60.0, NULL)",
    )
    .execute(pool)
    .await
    .expect("seed summary_processes");

    sqlx::query(
        "INSERT INTO transcript_chunks \
         (meeting_id, meeting_name, transcript_text, model, model_name, chunk_size, \
          overlap, created_at) \
         VALUES ('meeting-legacy-1', 'Kickoff', 'Hello world. Second segment.', \
                 'whisper', 'large-v3', 4000, 200, '2026-01-05T09:30:30.000Z')",
    )
    .execute(pool)
    .await
    .expect("seed transcript_chunks");

    sqlx::query(
        "INSERT INTO meeting_notes \
         (meeting_id, notes_markdown, notes_json, created_at, updated_at) \
         VALUES ('meeting-legacy-1', '# Notes', '{\"blocks\":[]}', \
                 '2026-01-05T09:10:00.000Z', '2026-01-05T09:20:00.000Z')",
    )
    .execute(pool)
    .await
    .expect("seed meeting_notes");

    sqlx::query(
        "INSERT INTO settings \
         (id, provider, model, whisperModel, groqApiKey, openaiApiKey, anthropicApiKey, \
          ollamaApiKey, openRouterApiKey, ollamaEndpoint, customOpenAIConfig, geminiApiKey) \
         VALUES ('1', 'ollama', 'llama3.2', 'large-v3', 'test-groq-key', NULL, NULL, \
                 NULL, NULL, 'http://localhost:11434', NULL, NULL)",
    )
    .execute(pool)
    .await
    .expect("seed settings");

    sqlx::query(
        "INSERT INTO transcript_settings \
         (id, provider, model, whisperApiKey, deepgramApiKey, elevenLabsApiKey, \
          groqApiKey, openaiApiKey) \
         VALUES ('1', 'local', 'large-v3', NULL, NULL, NULL, NULL, NULL)",
    )
    .execute(pool)
    .await
    .expect("seed transcript_settings");

    sqlx::query(
        "INSERT INTO licensing \
         (license_key, encrypted_key, signature_hash, activation_date, expiry_date, \
          soft_expiry_date, max_activation_time, duration, generated_on, \
          is_soft_expired, grace_period) \
         VALUES ('LIC-TEST-1', 'enc-blob', 'sig-hash', '2026-01-01T00:00:00Z', \
                 '2027-01-01T00:00:00Z', '2027-01-08T00:00:00Z', '2026-06-01T00:00:00Z', \
                 31536000, '2026-01-01T00:00:00Z', 0, 604800)",
    )
    .execute(pool)
    .await
    .expect("seed licensing");
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
        MIGRATOR.iter().filter(|m| m.version >= NEW_VERSION).count(),
        1,
        "exactly one migration at/after NEW_VERSION expected; bump NEW_VERSION when \
         a newer migration lands"
    );
    Migrator::new(dir).await.expect("resolve prior migrations")
}

#[tokio::test]
async fn fresh_db_gets_workspace_and_sync_columns() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let db_path = tmp.path().join("fresh.sqlite");
    let pool = open_temp(&db_path).await;

    MIGRATOR.run(&pool).await.expect("migrations on fresh db");
    assert_eq!(
        applied_versions(&pool).await.last().copied(),
        Some(NEW_VERSION),
        "newest migration must be applied"
    );

    for table in SYNCED_TABLES {
        assert_synced_shape(&pool, table).await;
    }

    for table in CONFIG_TABLES {
        let cols = table_columns(&pool, table).await;
        let ws = col(&cols, "workspace_id");
        assert!(ws.notnull, "{table}.workspace_id must be NOT NULL");
        assert_eq!(
            ws.dflt.as_deref(),
            Some("'local'"),
            "{table}.workspace_id default"
        );
        assert!(has_col(&cols, "created_at"), "{table} must have created_at");
        assert!(has_col(&cols, "updated_at"), "{table} must have updated_at");
        // Config tables are NOT synced: no sync columns (plaintext key material
        // must never gain sync semantics).
        for absent in ["rev", "updated_by", "deleted_at"] {
            assert!(!has_col(&cols, absent), "{table}.{absent} must NOT exist");
        }
    }

    // licensing is device-scoped activation state, deliberately untouched.
    let licensing_cols = table_columns(&pool, "licensing").await;
    assert!(
        !has_col(&licensing_cols, "workspace_id"),
        "licensing must stay workspace-less (device-scoped)"
    );

    // A legacy-shape INSERT (today's call sites) must backfill via defaults.
    sqlx::query(
        "INSERT INTO meetings (id, title, created_at, updated_at) \
         VALUES ('meeting-x', 'Fresh', '2026-07-02T10:00:00.000Z', '2026-07-02T10:00:00.000Z')",
    )
    .execute(&pool)
    .await
    .expect("legacy-shape insert");

    let row = sqlx::query(
        "SELECT workspace_id, rev, updated_by, deleted_at FROM meetings WHERE id = 'meeting-x'",
    )
    .fetch_one(&pool)
    .await
    .expect("read defaults");
    assert_eq!(row.get::<String, _>("workspace_id"), LOCAL_WORKSPACE_ID);
    assert_eq!(row.get::<i64, _>("rev"), 1);
    assert_eq!(row.get::<Option<String>, _>("updated_by"), None);
    assert_eq!(row.get::<Option<String>, _>("deleted_at"), None);

    // Phase-2 hot-path indexes.
    assert!(
        index_names(&pool, "meetings")
            .await
            .contains(&"idx_meetings_workspace_created".to_string()),
        "meetings workspace index missing"
    );
    assert!(
        index_names(&pool, "transcripts")
            .await
            .contains(&"idx_transcripts_workspace_meeting".to_string()),
        "transcripts workspace index missing"
    );

    pool.close().await;
}

#[tokio::test]
async fn populated_db_upgrade_backfills_and_preserves_data() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let db_path = tmp.path().join("populated.sqlite");
    let prior_dir = tmp.path().join("prior_migrations");
    std::fs::create_dir(&prior_dir).expect("mkdir prior_migrations");

    let pool = open_temp(&db_path).await;

    // 1) Bring the DB to the PREVIOUS schema version and populate it.
    let prior = prior_migrator(&prior_dir).await;
    prior.run(&pool).await.expect("prior migrations");
    seed_pre_migration_data(&pool).await;

    let tables = [
        "meetings",
        "transcripts",
        "summary_processes",
        "transcript_chunks",
        "meeting_notes",
        "settings",
        "transcript_settings",
        "licensing",
    ];
    let mut pre_counts = Vec::new();
    for t in tables {
        pre_counts.push((t, count(&pool, t).await));
    }
    let pre_applied = applied_versions(&pool).await;

    // 2) Upgrade with the real runner: exactly the new migration applies.
    MIGRATOR.run(&pool).await.expect("upgrade migration");
    let post_applied = applied_versions(&pool).await;
    assert_eq!(
        post_applied.len(),
        pre_applied.len() + 1,
        "exactly one new migration"
    );
    assert_eq!(post_applied.last().copied(), Some(NEW_VERSION));

    // 3) Row counts unchanged.
    for (t, pre) in &pre_counts {
        let post = count(&pool, t).await;
        assert_eq!(post, *pre, "row count changed for {t}");
    }

    // 4) Every synced row backfilled: workspace_id='local', rev=1, deleted_at/updated_by NULL.
    for t in SYNCED_TABLES {
        assert_eq!(
            count_local_pristine(&pool, t).await,
            count(&pool, t).await,
            "backfill incomplete on {t}"
        );
    }

    // 5) Pre-existing values byte-identical.
    let m = sqlx::query(
        "SELECT title, created_at, updated_at, folder_path FROM meetings \
         WHERE id = 'meeting-legacy-2'",
    )
    .fetch_one(&pool)
    .await
    .expect("meeting row");
    assert_eq!(m.get::<String, _>("title"), "Site visit");
    assert_eq!(m.get::<String, _>("created_at"), "2026-02-01T14:00:00.000Z");
    assert_eq!(m.get::<String, _>("updated_at"), "2026-02-01T15:00:00.000Z");
    assert_eq!(
        m.get::<Option<String>, _>("folder_path").as_deref(),
        Some("C:/recordings/site-visit")
    );

    let t2 = sqlx::query(
        "SELECT transcript, timestamp, summary, speaker, audio_start_time \
         FROM transcripts WHERE id = 'transcript-legacy-2'",
    )
    .fetch_one(&pool)
    .await
    .expect("transcript row");
    assert_eq!(t2.get::<String, _>("transcript"), "Second segment.");
    assert_eq!(t2.get::<String, _>("timestamp"), "2026-01-05T09:00:08.000Z");
    assert_eq!(
        t2.get::<Option<String>, _>("summary").as_deref(),
        Some("a summary")
    );
    assert_eq!(
        t2.get::<Option<String>, _>("speaker").as_deref(),
        Some("system")
    );
    assert_eq!(t2.get::<f64, _>("audio_start_time"), 2.5);

    // 6) Timestamp backfills: transcripts inherit the parent meeting's created_at;
    //    updated_at mirrors created_at; transcript_chunks.updated_at = created_at.
    let mismatched: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM transcripts t JOIN meetings m ON m.id = t.meeting_id \
         WHERE t.created_at IS NOT m.created_at OR t.updated_at IS NOT t.created_at",
    )
    .fetch_one(&pool)
    .await
    .expect("backfill check");
    assert_eq!(mismatched, 0, "transcripts timestamp backfill wrong");

    let chunk_mismatch: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM transcript_chunks WHERE updated_at IS NOT created_at",
    )
    .fetch_one(&pool)
    .await
    .expect("chunk backfill check");
    assert_eq!(
        chunk_mismatch, 0,
        "transcript_chunks.updated_at backfill wrong"
    );

    // 7) Config tables: scoped + timestamped, values untouched, still no sync columns.
    let s = sqlx::query(
        "SELECT workspace_id, created_at, updated_at, groqApiKey, ollamaEndpoint \
         FROM settings WHERE id = '1'",
    )
    .fetch_one(&pool)
    .await
    .expect("settings row");
    assert_eq!(s.get::<String, _>("workspace_id"), LOCAL_WORKSPACE_ID);
    assert!(s.get::<Option<String>, _>("created_at").is_some());
    assert_eq!(
        s.get::<Option<String>, _>("created_at"),
        s.get::<Option<String>, _>("updated_at")
    );
    assert_eq!(
        s.get::<Option<String>, _>("groqApiKey").as_deref(),
        Some("test-groq-key")
    );
    assert_eq!(
        s.get::<Option<String>, _>("ollamaEndpoint").as_deref(),
        Some("http://localhost:11434")
    );
    let settings_cols = table_columns(&pool, "settings").await;
    assert!(
        !has_col(&settings_cols, "rev"),
        "settings must not gain rev"
    );

    // 8) licensing untouched: schema and row identical.
    let licensing_cols = table_columns(&pool, "licensing").await;
    assert!(!has_col(&licensing_cols, "workspace_id"));
    let lic = sqlx::query(
        "SELECT encrypted_key, grace_period FROM licensing WHERE license_key = 'LIC-TEST-1'",
    )
    .fetch_one(&pool)
    .await
    .expect("licensing row");
    assert_eq!(lic.get::<String, _>("encrypted_key"), "enc-blob");
    assert_eq!(lic.get::<i64, _>("grace_period"), 604800);

    pool.close().await;
}

#[tokio::test]
async fn rerunning_full_migrator_is_a_noop() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let db_path = tmp.path().join("rerun.sqlite");
    let pool = open_temp(&db_path).await;

    MIGRATOR.run(&pool).await.expect("first run");
    sqlx::query(
        "INSERT INTO meetings (id, title, created_at, updated_at) \
         VALUES ('meeting-rerun', 'Rerun', '2026-07-02T11:00:00.000Z', '2026-07-02T11:00:00.000Z')",
    )
    .execute(&pool)
    .await
    .expect("insert between runs");

    let versions_before = applied_versions(&pool).await;
    let meetings_before = count(&pool, "meetings").await;

    MIGRATOR
        .run(&pool)
        .await
        .expect("second run (must be a no-op)");

    assert_eq!(
        applied_versions(&pool).await,
        versions_before,
        "ledger changed on re-run"
    );
    assert_eq!(count(&pool, "meetings").await, meetings_before);
    let row = sqlx::query("SELECT workspace_id, rev FROM meetings WHERE id = 'meeting-rerun'")
        .fetch_one(&pool)
        .await
        .expect("row after re-run");
    assert_eq!(row.get::<String, _>("workspace_id"), LOCAL_WORKSPACE_ID);
    assert_eq!(row.get::<i64, _>("rev"), 1);

    pool.close().await;
}

/// Empirical proof against a COPY of a real user database.
///
/// Skipped unless `MITYU_TEST_POPULATED_DB` points at a copy (never the live file;
/// the harness copies it with `cp` first). Read/write on the copy is fine.
#[tokio::test]
async fn real_populated_copy_upgrade_when_env_set() {
    let Ok(copy_path) = std::env::var("MITYU_TEST_POPULATED_DB") else {
        eprintln!("skipping: MITYU_TEST_POPULATED_DB not set");
        return;
    };
    let copy_path = std::path::PathBuf::from(copy_path);
    assert!(copy_path.exists(), "MITYU_TEST_POPULATED_DB does not exist");
    let pool = open_existing(&copy_path).await;

    let pre_cols = table_columns(&pool, "meetings").await;
    assert!(
        !has_col(&pre_cols, "workspace_id"),
        "copy already migrated; take a fresh pre-migration copy"
    );

    let tables = [
        "meetings",
        "transcripts",
        "summary_processes",
        "transcript_chunks",
        "meeting_notes",
        "settings",
        "transcript_settings",
        "licensing",
    ];
    let mut pre_counts = Vec::new();
    for t in tables {
        pre_counts.push((t, count(&pool, t).await));
    }

    // Snapshot original column values (concatenated, sorted) for the two largest tables.
    let meetings_snapshot: Vec<String> = sqlx::query_scalar(
        "SELECT id || '|' || title || '|' || created_at || '|' || updated_at || '|' || \
         COALESCE(folder_path, '<null>') FROM meetings ORDER BY id",
    )
    .fetch_all(&pool)
    .await
    .expect("meetings snapshot");
    let transcripts_snapshot: Vec<String> = sqlx::query_scalar(
        "SELECT id || '|' || meeting_id || '|' || transcript || '|' || timestamp \
         FROM transcripts ORDER BY id",
    )
    .fetch_all(&pool)
    .await
    .expect("transcripts snapshot");
    let pre_applied = applied_versions(&pool).await;

    MIGRATOR.run(&pool).await.expect("upgrade real copy");

    let post_applied = applied_versions(&pool).await;
    assert_eq!(
        post_applied.len(),
        pre_applied.len() + 1,
        "exactly one new migration"
    );
    assert_eq!(post_applied.last().copied(), Some(NEW_VERSION));

    println!(
        "real-copy upgrade: applied {NEW_VERSION} on top of {} priors",
        pre_applied.len()
    );
    for (t, pre) in &pre_counts {
        let post = count(&pool, t).await;
        println!("  {t}: {pre} rows before, {post} after");
        assert_eq!(post, *pre, "row count changed for {t}");
    }

    for t in SYNCED_TABLES {
        assert_eq!(
            count_local_pristine(&pool, t).await,
            count(&pool, t).await,
            "backfill incomplete on {t}"
        );
    }

    let meetings_after: Vec<String> = sqlx::query_scalar(
        "SELECT id || '|' || title || '|' || created_at || '|' || updated_at || '|' || \
         COALESCE(folder_path, '<null>') FROM meetings ORDER BY id",
    )
    .fetch_all(&pool)
    .await
    .expect("meetings snapshot after");
    assert_eq!(meetings_after, meetings_snapshot, "meetings rows changed");

    let transcripts_after: Vec<String> = sqlx::query_scalar(
        "SELECT id || '|' || meeting_id || '|' || transcript || '|' || timestamp \
         FROM transcripts ORDER BY id",
    )
    .fetch_all(&pool)
    .await
    .expect("transcripts snapshot after");
    assert_eq!(
        transcripts_after, transcripts_snapshot,
        "transcripts rows changed"
    );

    // Backfilled timestamps present on every transcript row.
    let missing_ts: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM transcripts WHERE created_at IS NULL OR updated_at IS NULL",
    )
    .fetch_one(&pool)
    .await
    .expect("timestamp backfill count");
    assert_eq!(missing_ts, 0, "transcripts missing backfilled timestamps");

    pool.close().await;
}
