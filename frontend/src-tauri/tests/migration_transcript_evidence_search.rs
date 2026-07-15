//! Integration tests for migration
//! `20260714000000_add_transcript_evidence_search_fts` (Product Intelligence v1).
//!
//! The FTS table is a local derived index, not a domain/sync entity. These tests
//! prove fresh creation, populated-database backfill, ledger idempotency and
//! trigger coherence for insert/update/soft-delete/hard-delete paths.

use sqlx::migrate::{MigrateError, Migrator};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};
use std::path::Path;

static MIGRATOR: Migrator = sqlx::migrate!("./migrations");
const NEW_VERSION: i64 = 20260714000000;

async fn open_temp(path: &Path) -> SqlitePool {
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true)
        .foreign_keys(true);
    SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .expect("open temp sqlite")
}

async fn prior_migrator(dir: &Path) -> Result<Migrator, MigrateError> {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
    for entry in std::fs::read_dir(source).expect("read migrations") {
        let entry = entry.expect("migration entry");
        let name = entry.file_name().to_string_lossy().to_string();
        let version: i64 = name
            .split('_')
            .next()
            .and_then(|value| value.parse().ok())
            .unwrap_or_else(|| panic!("unversioned migration: {name}"));
        if version < NEW_VERSION {
            std::fs::copy(entry.path(), dir.join(name)).expect("copy prior migration");
        }
    }
    Migrator::new(dir).await
}

async fn seed_segment(pool: &SqlitePool, id: &str, text: &str) {
    sqlx::query(
        "INSERT INTO meetings (id, title, created_at, updated_at) \
         VALUES ('meeting-search', 'Search fixture', \
                 '2026-07-14T09:00:00.000Z', '2026-07-14T09:00:00.000Z')",
    )
    .execute(pool)
    .await
    .expect("seed meeting");

    sqlx::query(
        "INSERT INTO transcripts \
         (id, meeting_id, transcript, timestamp, audio_start_time, audio_end_time, duration) \
         VALUES (?, 'meeting-search', ?, '2026-07-14T09:00:05.000Z', 5.0, 8.0, 3.0)",
    )
    .bind(id)
    .bind(text)
    .execute(pool)
    .await
    .expect("seed transcript");
}

async fn fts_count(pool: &SqlitePool) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM transcript_search_fts")
        .fetch_one(pool)
        .await
        .expect("count fts rows")
}

async fn document_count(pool: &SqlitePool) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM transcript_search_documents")
        .fetch_one(pool)
        .await
        .expect("count indexed document rows")
}

async fn match_count(pool: &SqlitePool, query: &str) -> i64 {
    sqlx::query_scalar(
        "SELECT COUNT(*) FROM transcript_search_fts \
         WHERE transcript_search_fts MATCH ?",
    )
    .bind(query)
    .fetch_one(pool)
    .await
    .expect("query fts")
}

#[tokio::test]
async fn fresh_database_creates_fts_and_live_triggers() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let pool = open_temp(&tmp.path().join("fresh.sqlite")).await;
    MIGRATOR.run(&pool).await.expect("migrate fresh database");

    let table_sql: String = sqlx::query_scalar(
        "SELECT sql FROM sqlite_master \
         WHERE type = 'table' AND name = 'transcript_search_fts'",
    )
    .fetch_one(&pool)
    .await
    .expect("fts virtual table exists");
    assert!(table_sql.to_lowercase().contains("fts5"));

    let secure_delete: i64 = sqlx::query_scalar(
        r#"SELECT CAST(v AS INTEGER) FROM transcript_search_fts_config
           WHERE k = 'secure-delete'"#,
    )
    .fetch_one(&pool)
    .await
    .expect("persistent FTS5 secure-delete config");
    assert_eq!(secure_delete, 1);

    let privacy_maintenance_pending: i64 =
        sqlx::query_scalar("SELECT required FROM local_privacy_maintenance WHERE singleton = 1")
            .fetch_one(&pool)
            .await
            .expect("privacy maintenance marker");
    assert_eq!(privacy_maintenance_pending, 1);

    let document_table_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master \
         WHERE type = 'table' AND name = 'transcript_search_documents'",
    )
    .fetch_one(&pool)
    .await
    .expect("document map exists");
    assert_eq!(document_table_count, 1);

    let maintenance_index_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master \
         WHERE type = 'index' \
           AND name = 'idx_transcript_search_documents_workspace_meeting'",
    )
    .fetch_one(&pool)
    .await
    .expect("meeting maintenance index exists");
    assert_eq!(maintenance_index_count, 1);

    let trigger_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master \
         WHERE type = 'trigger' AND name IN ( \
             'transcripts_search_fts_ai', \
             'transcripts_search_fts_au', \
             'transcripts_search_fts_ad', \
             'meetings_search_fts_au', \
             'meetings_search_fts_ad' \
         )",
    )
    .fetch_one(&pool)
    .await
    .expect("count search triggers");
    assert_eq!(trigger_count, 5);

    seed_segment(&pool, "chunk-fresh", "İstanbul karar toplantısı").await;
    assert_eq!(fts_count(&pool).await, 1);
    assert_eq!(document_count(&pool).await, 1);
    assert_eq!(match_count(&pool, "istanbul").await, 1);
    assert_eq!(match_count(&pool, "karar*").await, 1);
    assert_eq!(match_count(&pool, "toplanti*").await, 1);

    pool.close().await;
}

#[tokio::test]
async fn populated_upgrade_backfills_once_and_preserves_ledger_idempotency() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let prior_dir = tmp.path().join("prior");
    std::fs::create_dir(&prior_dir).expect("create prior dir");
    let pool = open_temp(&tmp.path().join("upgrade.sqlite")).await;

    prior_migrator(&prior_dir)
        .await
        .expect("resolve prior migrations")
        .run(&pool)
        .await
        .expect("run prior migrations");
    seed_segment(&pool, "chunk-upgrade", "backfill-evidence-token").await;

    let absent: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master \
         WHERE type = 'table' AND name = 'transcript_search_fts'",
    )
    .fetch_one(&pool)
    .await
    .expect("check precondition");
    assert_eq!(absent, 0);

    MIGRATOR.run(&pool).await.expect("upgrade migrations");
    assert_eq!(fts_count(&pool).await, 1);
    assert_eq!(document_count(&pool).await, 1);
    assert_eq!(match_count(&pool, "backfill").await, 1);

    let versions_before: Vec<i64> =
        sqlx::query_scalar("SELECT version FROM _sqlx_migrations ORDER BY version")
            .fetch_all(&pool)
            .await
            .expect("read ledger");
    assert!(versions_before.contains(&NEW_VERSION));

    MIGRATOR.run(&pool).await.expect("rerun full migrator");
    let versions_after: Vec<i64> =
        sqlx::query_scalar("SELECT version FROM _sqlx_migrations ORDER BY version")
            .fetch_all(&pool)
            .await
            .expect("read ledger after rerun");
    assert_eq!(versions_after, versions_before);
    assert_eq!(fts_count(&pool).await, 1);
    assert_eq!(document_count(&pool).await, 1);

    pool.close().await;
}

#[tokio::test]
async fn triggers_follow_update_soft_delete_restore_and_hard_delete() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let pool = open_temp(&tmp.path().join("triggers.sqlite")).await;
    MIGRATOR.run(&pool).await.expect("migrations");
    seed_segment(&pool, "chunk-trigger", "old-evidence-token").await;

    sqlx::query(
        "UPDATE transcripts SET transcript = 'new-evidence-token' \
         WHERE id = 'chunk-trigger'",
    )
    .execute(&pool)
    .await
    .expect("update transcript");
    assert_eq!(match_count(&pool, "old").await, 0);
    assert_eq!(match_count(&pool, "new").await, 1);
    assert_eq!(fts_count(&pool).await, 1);

    sqlx::query(
        "UPDATE transcripts SET deleted_at = '2026-07-14T10:00:00.000Z' \
         WHERE id = 'chunk-trigger'",
    )
    .execute(&pool)
    .await
    .expect("soft delete transcript");
    assert_eq!(fts_count(&pool).await, 0);
    assert_eq!(document_count(&pool).await, 0);

    sqlx::query("UPDATE transcripts SET deleted_at = NULL WHERE id = 'chunk-trigger'")
        .execute(&pool)
        .await
        .expect("restore transcript");
    assert_eq!(fts_count(&pool).await, 1);
    assert_eq!(document_count(&pool).await, 1);

    sqlx::query(
        "UPDATE meetings SET deleted_at = '2026-07-14T10:05:00.000Z' \
         WHERE id = 'meeting-search'",
    )
    .execute(&pool)
    .await
    .expect("soft delete meeting");
    assert_eq!(fts_count(&pool).await, 0);
    assert_eq!(document_count(&pool).await, 0);

    sqlx::query("UPDATE meetings SET deleted_at = NULL WHERE id = 'meeting-search'")
        .execute(&pool)
        .await
        .expect("restore meeting");
    assert_eq!(fts_count(&pool).await, 1);
    assert_eq!(document_count(&pool).await, 1);

    sqlx::query("DELETE FROM transcripts WHERE id = 'chunk-trigger'")
        .execute(&pool)
        .await
        .expect("hard delete transcript");
    assert_eq!(fts_count(&pool).await, 0);
    assert_eq!(document_count(&pool).await, 0);

    pool.close().await;
}

/// Maintenance must resolve source/meeting rows through B-tree indexes and
/// delete FTS documents by integer rowid. This pins the query plan at a corpus
/// size large enough to catch a regression back to UNINDEXED FTS routing scans
/// without relying on a flaky wall-clock benchmark.
#[tokio::test]
async fn large_corpus_maintenance_uses_indexed_document_map() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let pool = open_temp(&tmp.path().join("maintenance-scale.sqlite")).await;
    MIGRATOR.run(&pool).await.expect("migrations");

    sqlx::query(
        "INSERT INTO meetings (id, title, created_at, updated_at) \
         VALUES ('meeting-scale', 'Scale fixture', \
                 '2026-07-14T09:00:00.000Z', '2026-07-14T09:00:00.000Z')",
    )
    .execute(&pool)
    .await
    .expect("seed scale meeting");

    let mut tx = pool.begin().await.expect("begin scale seed");
    for index in 0..2_000 {
        sqlx::query(
            "INSERT INTO transcripts \
             (id, meeting_id, transcript, timestamp, audio_start_time, audio_end_time, duration) \
             VALUES (?, 'meeting-scale', ?, '2026-07-14T09:00:05.000Z', ?, ?, 0.5)",
        )
        .bind(format!("chunk-scale-{index}"))
        .bind(format!("scaletoken evidence {index}"))
        .bind(index as f64)
        .bind(index as f64 + 0.5)
        .execute(&mut *tx)
        .await
        .expect("seed scale transcript");
    }
    tx.commit().await.expect("commit scale seed");
    assert_eq!(fts_count(&pool).await, 2_000);
    assert_eq!(document_count(&pool).await, 2_000);

    let source_plan = sqlx::query(
        "EXPLAIN QUERY PLAN \
         SELECT id FROM transcript_search_documents \
         WHERE workspace_id = 'local' AND source_chunk_id = 'chunk-scale-1999'",
    )
    .fetch_all(&pool)
    .await
    .expect("source maintenance query plan");
    let source_details = source_plan
        .iter()
        .map(|row| row.get::<String, _>("detail"))
        .collect::<Vec<_>>()
        .join(" ");
    assert!(
        source_details.contains("INDEX"),
        "source lookup must use the unique maintenance index: {source_details}"
    );

    let meeting_plan = sqlx::query(
        "EXPLAIN QUERY PLAN \
         SELECT id FROM transcript_search_documents \
         WHERE workspace_id = 'local' AND meeting_id = 'meeting-scale'",
    )
    .fetch_all(&pool)
    .await
    .expect("meeting maintenance query plan");
    let meeting_details = meeting_plan
        .iter()
        .map(|row| row.get::<String, _>("detail"))
        .collect::<Vec<_>>()
        .join(" ");
    assert!(
        meeting_details.contains("idx_transcript_search_documents_workspace_meeting"),
        "meeting lookup must use its maintenance index: {meeting_details}"
    );

    let update_trigger_sql: String = sqlx::query_scalar(
        "SELECT sql FROM sqlite_master \
         WHERE type = 'trigger' AND name = 'transcripts_search_fts_au'",
    )
    .fetch_one(&pool)
    .await
    .expect("read transcript update trigger");
    assert!(update_trigger_sql.contains("WHERE rowid IN"));
    assert!(update_trigger_sql.contains("transcript_search_documents"));

    sqlx::query("DELETE FROM transcripts WHERE id = 'chunk-scale-1999'")
        .execute(&pool)
        .await
        .expect("delete one segment from large corpus");
    assert_eq!(fts_count(&pool).await, 1_999);
    assert_eq!(document_count(&pool).await, 1_999);

    pool.close().await;
}
