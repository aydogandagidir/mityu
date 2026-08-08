//! C6a sentinel proof for SQLite/FTS/free-page/WAL and recording artifacts.
//!
//! SQLCipher production databases encrypt content before it reaches disk, so a
//! plaintext temp database is intentionally used here to prove that deletion
//! semantics themselves remove a known sentinel rather than merely hiding it
//! behind encryption.

use app_lib::context::{AuthContext, RequestId, Role, TenantId, UserId};
use app_lib::database::deletion::{
    complete_privacy_maintenance, delete_database_records, ensure_folder_not_shared,
    erase_recording_folder, privacy_maintenance_required,
};
use sqlx::migrate::Migrator;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::path::{Path, PathBuf};

static MIGRATOR: Migrator = sqlx::migrate!("./migrations");
const SENTINEL: &str = "mityu_secure_delete_sentinel_c6a_7f91b3";

async fn open_migrated(path: &Path) -> SqlitePool {
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Wal)
        .pragma("secure_delete", "ON");
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .expect("open sentinel database");
    MIGRATOR.run(&pool).await.expect("apply real migrations");
    // Finish the migration's one-time historical maintenance before the test
    // creates its sentinel, matching an initialized v1.0.4 application.
    complete_privacy_maintenance(&pool)
        .await
        .expect("initial privacy maintenance");
    pool
}

async fn seed_sensitive_meeting(pool: &SqlitePool, folder: &Path) {
    let now = "2026-07-14T10:00:00.000Z";
    sqlx::query(
        r#"INSERT INTO meetings
           (id, workspace_id, title, created_at, updated_at, folder_path)
           VALUES ('meeting-c6a', 'local', ?, ?, ?, ?)"#,
    )
    .bind(format!("Sensitive {SENTINEL}"))
    .bind(now)
    .bind(now)
    .bind(folder.to_string_lossy().to_string())
    .execute(pool)
    .await
    .expect("seed meeting");

    sqlx::query(
        r#"INSERT INTO transcripts
           (id, meeting_id, workspace_id, transcript, timestamp, created_at, updated_at)
           VALUES ('segment-c6a', 'meeting-c6a', 'local', ?, ?, ?, ?)"#,
    )
    .bind(format!("Transcript contains {SENTINEL}"))
    .bind(now)
    .bind(now)
    .bind(now)
    .execute(pool)
    .await
    .expect("seed transcript");

    sqlx::query(
        r#"INSERT INTO transcript_chunks
           (meeting_id, workspace_id, meeting_name, transcript_text, model, model_name,
            created_at, updated_at)
           VALUES ('meeting-c6a', 'local', 'Sensitive', ?, 'whisper', 'large-v3', ?, ?)"#,
    )
    .bind(format!("Full transcript {SENTINEL}"))
    .bind(now)
    .bind(now)
    .execute(pool)
    .await
    .expect("seed transcript chunk");

    sqlx::query(
        r#"INSERT INTO summary_processes
           (meeting_id, workspace_id, status, created_at, updated_at, result)
           VALUES ('meeting-c6a', 'local', 'completed', ?, ?, ?)"#,
    )
    .bind(now)
    .bind(now)
    .bind(format!("{{\"summary\":\"{SENTINEL}\"}}"))
    .execute(pool)
    .await
    .expect("seed legacy summary");

    sqlx::query(
        r#"INSERT INTO meeting_notes
           (meeting_id, workspace_id, notes_markdown, created_at, updated_at)
           VALUES ('meeting-c6a', 'local', ?, ?, ?)"#,
    )
    .bind(format!("Human note {SENTINEL}"))
    .bind(now)
    .bind(now)
    .execute(pool)
    .await
    .expect("seed notes");

    sqlx::query(
        r#"INSERT INTO summaries
           (id, meeting_id, workspace_id, status, sections, created_at, updated_at)
           VALUES ('summary-c6a', 'meeting-c6a', 'local', 'draft', ?, ?, ?)"#,
    )
    .bind(format!("[{{\"content\":\"{SENTINEL}\"}}]"))
    .bind(now)
    .bind(now)
    .execute(pool)
    .await
    .expect("seed summary");

    sqlx::query(
        r#"INSERT INTO action_items
           (id, meeting_id, workspace_id, text, status, source_chunk_id, created_at, updated_at)
           VALUES ('action-c6a', 'meeting-c6a', 'local', ?, 'draft', 'segment-c6a', ?, ?)"#,
    )
    .bind(format!("Action {SENTINEL}"))
    .bind(now)
    .bind(now)
    .execute(pool)
    .await
    .expect("seed action");

    // The learning system's correction record. Seeded with the sentinel in the
    // HUMAN'S rewrite, because that column is the most sensitive thing this
    // schema derives from a meeting (ADR-0030) — and because listing a table in
    // the residual-count assertion without ever writing a row to it proves
    // nothing: `COUNT(*) = 0` passes trivially.
    sqlx::query(
        r#"INSERT INTO correction_events
           (id, workspace_id, meeting_id, subject_kind, subject_id, action,
            original_text, final_text, created_at, updated_at)
           VALUES ('correction-c6a', 'local', 'meeting-c6a', 'summary_block', 'summary-c6a',
                   'edited', 'AI draft', ?, ?, ?)"#,
    )
    .bind(format!("Human rewrite {SENTINEL}"))
    .bind(now)
    .bind(now)
    .execute(pool)
    .await
    .expect("seed correction event");

    // Diarization turns (ADR-0034). Nothing writes these yet; seeding one keeps
    // the erasure contract honest from the moment the engine lands rather than
    // after someone notices.
    sqlx::query(
        r#"INSERT INTO speaker_turns
           (id, meeting_id, workspace_id, speaker_label, start_ms, end_ms, created_at, updated_at)
           VALUES ('turn-c6a', 'meeting-c6a', 'local', ?, 0, 9000, ?, ?)"#,
    )
    .bind(format!("Speaker {SENTINEL}"))
    .bind(now)
    .bind(now)
    .execute(pool)
    .await
    .expect("seed speaker turn");
}

fn contains_sentinel(path: &Path) -> bool {
    std::fs::read(path)
        .map(|bytes| {
            bytes
                .windows(SENTINEL.len())
                .any(|window| window == SENTINEL.as_bytes())
        })
        .unwrap_or(false)
}

fn sqlite_artifact_paths(database: &Path) -> Vec<PathBuf> {
    let raw = database.to_string_lossy();
    vec![
        database.to_path_buf(),
        PathBuf::from(format!("{raw}-wal")),
        PathBuf::from(format!("{raw}-shm")),
        PathBuf::from(format!("{raw}-journal")),
    ]
}

#[tokio::test]
async fn owner_delete_removes_sentinel_from_sqlite_fts_wal_and_managed_files() {
    let temp = tempfile::tempdir().expect("tempdir");
    let database = temp.path().join("sentinel.sqlite");
    let recording_root = temp.path().join("recordings");
    let meeting_folder = recording_root.join("Meeting_2026-07-14_10-00");
    let checkpoints = meeting_folder.join(".checkpoints");
    std::fs::create_dir_all(&checkpoints).expect("recording folders");
    std::fs::write(
        meeting_folder.join("audio.mp4"),
        format!("audio bytes {SENTINEL}"),
    )
    .expect("audio sentinel");
    std::fs::write(
        meeting_folder.join("metadata.json"),
        format!("{{\"sentinel\":\"{SENTINEL}\"}}"),
    )
    .expect("metadata sentinel");
    std::fs::write(
        meeting_folder.join("transcripts.json"),
        format!("{{\"text\":\"{SENTINEL}\"}}"),
    )
    .expect("transcript file sentinel");
    std::fs::write(
        checkpoints.join("chunk_0001.mp4"),
        format!("checkpoint {SENTINEL}"),
    )
    .expect("checkpoint sentinel");
    std::fs::write(meeting_folder.join("keep.txt"), b"user-owned file").expect("unknown user file");

    let pool = open_migrated(&database).await;
    seed_sensitive_meeting(&pool, &meeting_folder).await;
    sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
        .execute(&pool)
        .await
        .expect("pre-delete checkpoint");
    assert!(
        contains_sentinel(&database),
        "test precondition: plaintext DB must contain the sentinel"
    );

    let foreign = AuthContext {
        tenant_id: TenantId::new("other-workspace"),
        user_id: UserId::new("other-user"),
        roles: vec![Role::Owner],
        request_id: RequestId::generate(),
    };
    assert!(!delete_database_records(&pool, &foreign, "meeting-c6a")
        .await
        .expect("foreign delete remains a no-op"));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM meetings WHERE id='meeting-c6a'")
            .fetch_one(&pool)
            .await
            .expect("meeting still exists"),
        1
    );

    let report = erase_recording_folder(&meeting_folder, &[recording_root], &AuthContext::local())
        .expect("erase Mityu-managed recording artifacts");
    assert_eq!(report.managed_files_removed, 4);
    assert_eq!(report.retained_user_entries, 1);
    assert!(meeting_folder.join("keep.txt").exists());
    assert!(!meeting_folder.join("audio.mp4").exists());
    assert!(!meeting_folder.join("metadata.json").exists());
    assert!(!meeting_folder.join("transcripts.json").exists());
    assert!(!meeting_folder.join(".checkpoints").exists());

    let local = AuthContext::local();
    assert!(delete_database_records(&pool, &local, "meeting-c6a")
        .await
        .expect("tenant-scoped logical delete"));
    assert!(
        privacy_maintenance_required(&pool)
            .await
            .expect("pending marker"),
        "delete commit and maintenance marker must be atomic"
    );
    complete_privacy_maintenance(&pool)
        .await
        .expect("verified physical maintenance");
    assert!(!privacy_maintenance_required(&pool)
        .await
        .expect("completed marker"));

    for table in [
        "meetings",
        "transcripts",
        "transcript_chunks",
        "summary_processes",
        "meeting_notes",
        "summaries",
        "action_items",
        "correction_events",
        "speaker_turns",
        "transcript_search_documents",
        "transcript_search_fts",
    ] {
        let count: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table}"))
            .fetch_one(&pool)
            .await
            .unwrap_or_else(|error| panic!("count {table}: {error}"));
        assert_eq!(count, 0, "{table} must not retain meeting data");
    }

    let fts_secure_delete: i64 = sqlx::query_scalar(
        r#"SELECT CAST(v AS INTEGER) FROM transcript_search_fts_config
           WHERE k='secure-delete'"#,
    )
    .fetch_one(&pool)
    .await
    .expect("FTS secure-delete config");
    assert_eq!(fts_secure_delete, 1);
    let core_secure_delete: i64 = sqlx::query_scalar("PRAGMA secure_delete")
        .fetch_one(&pool)
        .await
        .expect("core secure-delete config");
    assert_eq!(core_secure_delete, 1);
    let free_pages: i64 = sqlx::query_scalar("PRAGMA freelist_count")
        .fetch_one(&pool)
        .await
        .expect("freelist count");
    assert_eq!(free_pages, 0);

    pool.close().await;
    for path in sqlite_artifact_paths(&database) {
        if path.exists() {
            assert!(
                !contains_sentinel(&path),
                "deleted sentinel remained in {}",
                path.display()
            );
            if path.extension().and_then(|value| value.to_str()) == Some("wal") {
                assert_eq!(
                    std::fs::metadata(&path).expect("WAL metadata").len(),
                    0,
                    "WAL must be truncated"
                );
            }
        }
    }
}

#[tokio::test]
async fn recording_folder_collision_in_same_workspace_fails_closed() {
    let temp = tempfile::tempdir().expect("tempdir");
    let database = temp.path().join("folder-collision.sqlite");
    let shared_folder = temp.path().join("recordings").join("shared-meeting");
    std::fs::create_dir_all(&shared_folder).expect("shared folder");
    let pool = open_migrated(&database).await;
    let now = "2026-07-14T10:00:00.000Z";

    for id in ["local-meeting", "second-local-meeting"] {
        sqlx::query(
            "INSERT INTO meetings \
              (id, workspace_id, title, created_at, updated_at, folder_path) \
              VALUES (?, 'local', 'Collision test', ?, ?, ?)",
        )
        .bind(id)
        .bind(now)
        .bind(now)
        .bind(shared_folder.to_string_lossy().to_string())
        .execute(&pool)
        .await
        .expect("seed colliding meeting");
    }

    let error = ensure_folder_not_shared(
        &pool,
        &AuthContext::local(),
        "local-meeting",
        &shared_folder.to_string_lossy(),
    )
    .await
    .expect_err("same-workspace folder collision must block deletion");
    assert!(error.to_string().contains("another meeting"));
    assert!(shared_folder.exists());
}

#[tokio::test]
async fn foreign_workspace_folder_reference_is_not_enumerated_by_collision_guard() {
    let temp = tempfile::tempdir().expect("tempdir");
    let database = temp.path().join("foreign-folder-reference.sqlite");
    let shared_folder = temp.path().join("recordings").join("shared-meeting");
    std::fs::create_dir_all(&shared_folder).expect("shared folder");
    let pool = open_migrated(&database).await;
    let now = "2026-07-14T10:00:00.000Z";

    for (id, workspace) in [
        ("local-meeting", "local"),
        ("foreign-meeting", "other-workspace"),
    ] {
        sqlx::query(
            "INSERT INTO meetings \
             (id, workspace_id, title, created_at, updated_at, folder_path) \
             VALUES (?, ?, 'Scoped collision test', ?, ?, ?)",
        )
        .bind(id)
        .bind(workspace)
        .bind(now)
        .bind(now)
        .bind(shared_folder.to_string_lossy().to_string())
        .execute(&pool)
        .await
        .expect("seed scoped folder reference");
    }

    ensure_folder_not_shared(
        &pool,
        &AuthContext::local(),
        "local-meeting",
        &shared_folder.to_string_lossy(),
    )
    .await
    .expect("foreign workspace rows must not participate in the scoped collision query");
}

#[tokio::test]
async fn scoped_delete_leaves_malformed_foreign_workspace_dependent_untouched() {
    let temp = tempfile::tempdir().expect("tempdir");
    let database = temp.path().join("foreign-dependent.sqlite");
    let pool = open_migrated(&database).await;
    let now = "2026-07-14T10:00:00.000Z";

    sqlx::query(
        "INSERT INTO meetings (id, workspace_id, title, created_at, updated_at) \
         VALUES ('local-parent', 'local', 'Integrity test', ?, ?)",
    )
    .bind(now)
    .bind(now)
    .execute(&pool)
    .await
    .expect("seed local parent");
    sqlx::query(
        "INSERT INTO transcripts \
         (id, meeting_id, workspace_id, transcript, timestamp, created_at, updated_at) \
         VALUES ('foreign-child', 'local-parent', 'other-workspace', 'must survive', ?, ?, ?)",
    )
    .bind(now)
    .bind(now)
    .bind(now)
    .execute(&pool)
    .await
    .expect("seed malformed foreign child");

    assert!(
        delete_database_records(&pool, &AuthContext::local(), "local-parent")
            .await
            .expect("caller-scoped delete must succeed without reading foreign rows")
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM meetings WHERE id='local-parent'")
            .fetch_one(&pool)
            .await
            .expect("parent count"),
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM transcripts WHERE id='foreign-child'")
            .fetch_one(&pool)
            .await
            .expect("child count"),
        1
    );
    assert!(privacy_maintenance_required(&pool)
        .await
        .expect("maintenance marker"));
    complete_privacy_maintenance(&pool)
        .await
        .expect("complete scoped deletion maintenance");
}

/// Structural guard: every table that declares a foreign key to `meetings(id)`
/// must be deleted explicitly by the meeting-deletion path.
///
/// This exists because the obvious verification is WRONG, and two independent
/// authors already got it wrong the same way. `delete_database_records` runs
/// with `PRAGMA foreign_keys = OFF` on purpose — so that a malformed
/// foreign-workspace child cannot be cascaded away with the caller's parent —
/// which means a declared `ON DELETE CASCADE` never fires in production. Proving
/// a cascade in a sqlite3 session with foreign keys ON proves nothing about the
/// app, yet reads as convincing. Both `correction_events` (ADR-0030) and
/// `speaker_turns` (ADR-0034) shipped that way, and `correction_events` retained
/// the human's rewritten meeting text after a "delete my data" that reported
/// success.
///
/// The per-table assertion above cannot catch the NEXT such table, because it is
/// a hand-maintained list and the omission is precisely a hand-maintenance
/// failure. This one is derived from the schema, so a new child table fails here
/// the moment its migration lands.
#[tokio::test]
async fn every_child_of_meetings_is_deleted_explicitly() {
    let dir = tempfile::tempdir().expect("temp dir");
    let pool = open_migrated(&dir.path().join("fk.db")).await;

    // Tables that declare a FK to meetings(id), read from the live schema.
    let names: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",
    )
    .fetch_all(&pool)
    .await
    .expect("list tables");

    let mut children: Vec<String> = Vec::new();
    for table in names {
        let refs: Vec<String> = sqlx::query_scalar(&format!(
            "SELECT \"table\" FROM pragma_foreign_key_list('{table}')"
        ))
        .fetch_all(&pool)
        .await
        .unwrap_or_default();
        if refs.iter().any(|t| t.eq_ignore_ascii_case("meetings")) {
            children.push(table);
        }
    }
    assert!(
        !children.is_empty(),
        "no child tables found — the probe itself is broken"
    );

    // The explicit list in `delete_meeting_with_connection`. Kept as source text
    // so the test fails on a schema addition, not on a refactor.
    let source = include_str!("../src/database/repositories/meeting.rs");
    let mut missing: Vec<String> = Vec::new();
    for child in &children {
        if !source.contains(&format!("DELETE FROM {child} WHERE meeting_id = ?")) {
            missing.push(child.clone());
        }
    }

    assert!(
        missing.is_empty(),
        "these tables reference meetings(id) but are never deleted by \
         delete_meeting_with_connection: {missing:?}. Their ON DELETE CASCADE cannot save them — \
         the deletion path runs with PRAGMA foreign_keys = OFF by design. Add an explicit \
         DELETE, or document an exemption here."
    );
}
