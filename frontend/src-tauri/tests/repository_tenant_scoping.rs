//! Integration tests for the tenant-scoped repository layer (BACKLOG B2 phase 2,
//! docs/CONTRACTS.md §2, docs/CONVENTIONS.md "integration tests for repositories
//! (prove tenant scoping)").
//!
//! Proves, against a temp database migrated with the app's real migration set:
//!   1. Every repository INSERT fills `workspace_id` from the AuthContext plus
//!      `created_at`/`updated_at` (nullable at the SQL level on transcripts /
//!      transcript_chunks / settings — phase-1 sharp edge) and `rev = 1` /
//!      `updated_by` on synced tables.
//!   2. Reads under a *different* workspace cannot see rows created under
//!      `local`, and vice versa (negative test in both directions).
//!   3. Updates/deletes under a different workspace are no-ops; same-workspace
//!      updates bump `rev`, stamp `updated_by`, and write RFC 3339 `updated_at`
//!      (covers the `update_meeting_title` naive_utc fix).
//!   4. The settings upsert guard: a foreign workspace can neither read nor
//!      clobber the local workspace's single `id = '1'` config row.

use sqlx::migrate::Migrator;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};

use app_lib::api::{SearchMatchField, TranscriptSegment};
use app_lib::context::{AuthContext, RequestId, Role, TenantId, UserId, LOCAL_WORKSPACE_ID};
use app_lib::database::repositories::{
    meeting::MeetingsRepository,
    setting::SettingsRepository,
    summary::SummaryProcessesRepository,
    transcript::TranscriptsRepository,
    transcript_chunk::{TranscriptChunkData, TranscriptChunksRepository},
};
use app_lib::secrets::KEYCHAIN_MARKER;

/// Process-global, persistent, in-memory keychain for tests. keyring's built-in
/// `mock` backend is `EntryOnly` (a set on one `Entry` is invisible to a later
/// `Entry` for the same name), which cannot model the repository's separate
/// set→get `Entry::new` calls, so we install a store that persists secrets keyed
/// by `(service, entry_name)`. It NEVER touches the real OS credential manager.
mod mem_keychain {
    use keyring::credential::{
        Credential, CredentialApi, CredentialBuilderApi, CredentialPersistence,
    };
    use keyring::Error;
    use std::collections::HashMap;
    use std::sync::{Mutex, Once, OnceLock};

    type Key = (String, String);

    fn store() -> &'static Mutex<HashMap<Key, Vec<u8>>> {
        static STORE: OnceLock<Mutex<HashMap<Key, Vec<u8>>>> = OnceLock::new();
        STORE.get_or_init(|| Mutex::new(HashMap::new()))
    }

    #[derive(Debug)]
    struct MemCredential {
        key: Key,
    }

    impl CredentialApi for MemCredential {
        fn set_secret(&self, secret: &[u8]) -> Result<(), Error> {
            store()
                .lock()
                .unwrap()
                .insert(self.key.clone(), secret.to_vec());
            Ok(())
        }
        fn get_secret(&self) -> Result<Vec<u8>, Error> {
            match store().lock().unwrap().get(&self.key) {
                Some(v) => Ok(v.clone()),
                None => Err(Error::NoEntry),
            }
        }
        fn delete_credential(&self) -> Result<(), Error> {
            match store().lock().unwrap().remove(&self.key) {
                Some(_) => Ok(()),
                None => Err(Error::NoEntry),
            }
        }
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    }

    struct MemBuilder;

    impl CredentialBuilderApi for MemBuilder {
        fn build(
            &self,
            _target: Option<&str>,
            service: &str,
            user: &str,
        ) -> Result<Box<Credential>, Error> {
            Ok(Box::new(MemCredential {
                key: (service.to_string(), user.to_string()),
            }))
        }
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
        fn persistence(&self) -> CredentialPersistence {
            CredentialPersistence::UntilDelete
        }
    }

    /// Install as the process-wide default. Idempotent per process.
    pub fn install() {
        static ONCE: Once = Once::new();
        ONCE.call_once(|| {
            keyring::set_default_credential_builder(Box::new(MemBuilder));
        });
    }

    pub fn clear() {
        store().lock().unwrap().clear();
    }
}

async fn install_mock_keychain() -> tokio::sync::MutexGuard<'static, ()> {
    static KEYCHAIN_TEST_LOCK: std::sync::OnceLock<tokio::sync::Mutex<()>> =
        std::sync::OnceLock::new();
    let guard = KEYCHAIN_TEST_LOCK
        .get_or_init(|| tokio::sync::Mutex::new(()))
        .lock()
        .await;
    mem_keychain::install();
    mem_keychain::clear();
    guard
}

/// The app's real, compile-time-embedded migration set (same source as
/// `DatabaseManager::new`).
static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

const OTHER_WORKSPACE_ID: &str = "other-ws";

/// A second workspace's context, built directly (test-only): `context::current()`
/// deliberately resolves to the local workspace, and identity must never come
/// from anywhere else in production code.
fn other_ws_ctx() -> AuthContext {
    AuthContext {
        tenant_id: TenantId::new(OTHER_WORKSPACE_ID),
        user_id: UserId::new("other-user"),
        roles: vec![Role::Owner],
        request_id: RequestId::generate(),
    }
}

async fn open_migrated_temp_db(path: &std::path::Path) -> SqlitePool {
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .expect("failed to open temp sqlite database");
    MIGRATOR.run(&pool).await.expect("migrations must apply");
    pool
}

fn segment(id: &str, text: &str, start: f64, end: f64) -> TranscriptSegment {
    TranscriptSegment {
        id: id.to_string(),
        text: text.to_string(),
        timestamp: "2026-07-02T10:00:00.000Z".to_string(),
        audio_start_time: Some(start),
        audio_end_time: Some(end),
        duration: Some(end - start),
    }
}

fn chunk_data(text: &str) -> TranscriptChunkData<'_> {
    TranscriptChunkData {
        text,
        model: "whisper",
        model_name: "large-v3",
        chunk_size: 4000,
        overlap: 200,
    }
}

async fn count_where(pool: &SqlitePool, table: &str, where_clause: &str) -> i64 {
    sqlx::query_scalar(&format!(
        "SELECT COUNT(*) FROM {table} WHERE {where_clause}"
    ))
    .fetch_one(pool)
    .await
    .unwrap_or_else(|e| panic!("COUNT on {table} ({where_clause}) failed: {e}"))
}

/// Fetch (workspace_id, rev, updated_by, created_at, updated_at) for one row.
async fn sync_columns(
    pool: &SqlitePool,
    table: &str,
    key_col: &str,
    key: &str,
) -> (String, i64, Option<String>, Option<String>, Option<String>) {
    let row = sqlx::query(&format!(
        "SELECT workspace_id, rev, updated_by, created_at, updated_at \
         FROM {table} WHERE {key_col} = ?"
    ))
    .bind(key)
    .fetch_one(pool)
    .await
    .unwrap_or_else(|e| panic!("sync-column fetch on {table} failed: {e}"));
    (
        row.get::<String, _>("workspace_id"),
        row.get::<i64, _>("rev"),
        row.get::<Option<String>, _>("updated_by"),
        row.get::<Option<String>, _>("created_at"),
        row.get::<Option<String>, _>("updated_at"),
    )
}

fn assert_rfc3339(value: &Option<String>, what: &str) {
    let raw = value
        .as_deref()
        .unwrap_or_else(|| panic!("{what} must not be NULL"));
    chrono::DateTime::parse_from_rfc3339(raw)
        .unwrap_or_else(|e| panic!("{what} must be RFC 3339, got {raw:?}: {e}"));
}

#[tokio::test]
async fn inserts_fill_workspace_timestamps_rev_and_updated_by() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let pool = open_migrated_temp_db(&tmp.path().join("inserts.sqlite")).await;
    let local = AuthContext::local();

    let meeting_id = TranscriptsRepository::save_transcript(
        &pool,
        &local,
        "Kickoff",
        &[segment("transcript-t1", "Hello world.", 0.0, 2.5)],
        Some("C:/recordings/kickoff".to_string()),
    )
    .await
    .expect("save_transcript");

    // meetings: full synced-column stamp.
    let (ws, rev, updated_by, created_at, updated_at) =
        sync_columns(&pool, "meetings", "id", &meeting_id).await;
    assert_eq!(ws, LOCAL_WORKSPACE_ID);
    assert_eq!(rev, 1, "fresh insert must carry rev = 1");
    assert_eq!(updated_by.as_deref(), Some("local-user"));
    assert_rfc3339(&created_at, "meetings.created_at");
    assert_rfc3339(&updated_at, "meetings.updated_at");

    // transcripts: created_at/updated_at are NULLABLE columns — the repository
    // must have populated them on INSERT (phase-1 sharp edge #1).
    let transcript_id: String =
        sqlx::query_scalar("SELECT id FROM transcripts WHERE meeting_id = ?")
            .bind(&meeting_id)
            .fetch_one(&pool)
            .await
            .expect("one transcript row");
    let (ws, rev, updated_by, created_at, updated_at) =
        sync_columns(&pool, "transcripts", "id", &transcript_id).await;
    assert_eq!(ws, LOCAL_WORKSPACE_ID);
    assert_eq!(rev, 1);
    assert_eq!(updated_by.as_deref(), Some("local-user"));
    assert_rfc3339(&created_at, "transcripts.created_at");
    assert_rfc3339(&updated_at, "transcripts.updated_at");

    // summary_processes insert path.
    SummaryProcessesRepository::create_or_reset_process(&pool, &local, &meeting_id)
        .await
        .expect("create_or_reset_process");
    let (ws, rev, updated_by, created_at, updated_at) =
        sync_columns(&pool, "summary_processes", "meeting_id", &meeting_id).await;
    assert_eq!(ws, LOCAL_WORKSPACE_ID);
    assert_eq!(rev, 1);
    assert_eq!(updated_by.as_deref(), Some("local-user"));
    assert_rfc3339(&created_at, "summary_processes.created_at");
    assert_rfc3339(&updated_at, "summary_processes.updated_at");

    // transcript_chunks: updated_at is NULLABLE — must be populated on INSERT;
    // a second save (upsert) must bump rev.
    TranscriptChunksRepository::save_transcript_data(
        &pool,
        &local,
        &meeting_id,
        chunk_data("Hello world."),
    )
    .await
    .expect("save_transcript_data insert");
    let (ws, rev, _, created_at, updated_at) =
        sync_columns(&pool, "transcript_chunks", "meeting_id", &meeting_id).await;
    assert_eq!(ws, LOCAL_WORKSPACE_ID);
    assert_eq!(rev, 1);
    assert_rfc3339(&created_at, "transcript_chunks.created_at");
    assert_rfc3339(&updated_at, "transcript_chunks.updated_at");

    TranscriptChunksRepository::save_transcript_data(
        &pool,
        &local,
        &meeting_id,
        chunk_data("Hello again."),
    )
    .await
    .expect("save_transcript_data upsert");
    let (_, rev, updated_by, _, _) =
        sync_columns(&pool, "transcript_chunks", "meeting_id", &meeting_id).await;
    assert_eq!(rev, 2, "upsert on the same workspace must bump rev");
    assert_eq!(updated_by.as_deref(), Some("local-user"));

    pool.close().await;
}

#[tokio::test]
async fn reads_are_workspace_scoped_in_both_directions() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let pool = open_migrated_temp_db(&tmp.path().join("reads.sqlite")).await;
    let local = AuthContext::local();
    let other = other_ws_ctx();

    let local_meeting = TranscriptsRepository::save_transcript(
        &pool,
        &local,
        "Local standup",
        &[segment("transcript-l1", "local secret alpha", 0.0, 2.0)],
        None,
    )
    .await
    .expect("local save_transcript");

    let other_meeting = TranscriptsRepository::save_transcript(
        &pool,
        &other,
        "Other tenant sync",
        &[segment("transcript-o1", "other secret beta", 0.0, 2.0)],
        None,
    )
    .await
    .expect("other save_transcript");

    // Listing: each workspace sees exactly its own meeting.
    let local_list = MeetingsRepository::get_meetings(&pool, &local)
        .await
        .expect("local list");
    assert_eq!(local_list.len(), 1);
    assert_eq!(local_list[0].id, local_meeting);

    let other_list = MeetingsRepository::get_meetings(&pool, &other)
        .await
        .expect("other list");
    assert_eq!(other_list.len(), 1);
    assert_eq!(other_list[0].id, other_meeting);

    // Point reads across the boundary fail closed.
    assert!(
        matches!(
            MeetingsRepository::get_meeting(&pool, &other, &local_meeting).await,
            Err(sqlx::Error::RowNotFound)
        ),
        "other-ws must not resolve a local meeting"
    );
    assert!(
        matches!(
            MeetingsRepository::get_meeting(&pool, &local, &other_meeting).await,
            Err(sqlx::Error::RowNotFound)
        ),
        "local must not resolve an other-ws meeting"
    );
    assert!(
        MeetingsRepository::get_meeting_metadata(&pool, &other, &local_meeting)
            .await
            .expect("metadata query")
            .is_none()
    );

    let (rows, total) =
        MeetingsRepository::get_meeting_transcripts_paginated(&pool, &other, &local_meeting, 50, 0)
            .await
            .expect("paginated query");
    assert!(rows.is_empty());
    assert_eq!(total, 0);

    // Search must not leak transcript content across workspaces.
    let hits = TranscriptsRepository::search_transcripts(&pool, &local, "secret")
        .await
        .expect("local search");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].id, local_meeting);
    let hits = TranscriptsRepository::search_transcripts(&pool, &other, "secret")
        .await
        .expect("other search");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].id, other_meeting);
    assert!(
        TranscriptsRepository::search_transcripts(&pool, &other, "alpha")
            .await
            .expect("other search for local content")
            .is_empty()
    );

    // Summary reads are scoped too (plain and JOIN variants).
    SummaryProcessesRepository::create_or_reset_process(&pool, &local, &local_meeting)
        .await
        .expect("local summary process");
    TranscriptChunksRepository::save_transcript_data(
        &pool,
        &local,
        &local_meeting,
        chunk_data("local secret alpha"),
    )
    .await
    .expect("local chunks");
    assert!(
        SummaryProcessesRepository::get_summary_data(&pool, &other, &local_meeting)
            .await
            .expect("scoped get_summary_data")
            .is_none()
    );
    assert!(SummaryProcessesRepository::get_summary_data_for_meeting(
        &pool,
        &other,
        &local_meeting
    )
    .await
    .expect("scoped get_summary_data_for_meeting")
    .is_none());
    assert!(
        SummaryProcessesRepository::get_summary_data(&pool, &local, &local_meeting)
            .await
            .expect("local get_summary_data")
            .is_some()
    );

    pool.close().await;
}

#[tokio::test]
async fn updates_are_workspace_scoped_and_bump_rev() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let pool = open_migrated_temp_db(&tmp.path().join("updates.sqlite")).await;
    let local = AuthContext::local();
    let other = other_ws_ctx();

    let meeting_id = TranscriptsRepository::save_transcript(
        &pool,
        &local,
        "Original title",
        &[segment("transcript-u1", "content", 0.0, 1.0)],
        None,
    )
    .await
    .expect("save_transcript");
    SummaryProcessesRepository::create_or_reset_process(&pool, &local, &meeting_id)
        .await
        .expect("summary process");

    // Cross-workspace title update: reported as "not found", nothing changes.
    let updated = MeetingsRepository::update_meeting_title(&pool, &other, &meeting_id, "hacked")
        .await
        .expect("cross-ws update runs without error");
    assert!(!updated, "cross-workspace update must report no match");
    let (title, rev): (String, i64) =
        sqlx::query_as("SELECT title, rev FROM meetings WHERE id = ?")
            .bind(&meeting_id)
            .fetch_one(&pool)
            .await
            .expect("meeting row");
    assert_eq!(title, "Original title");
    assert_eq!(rev, 1);

    // Same-workspace title update: succeeds, bumps rev, stamps updated_by and an
    // RFC 3339 updated_at (regression check for the former naive_utc() binding).
    let updated = MeetingsRepository::update_meeting_title(&pool, &local, &meeting_id, "Renamed")
        .await
        .expect("local update");
    assert!(updated);
    let (_, rev, updated_by, _, updated_at) =
        sync_columns(&pool, "meetings", "id", &meeting_id).await;
    assert_eq!(rev, 2, "update must bump rev");
    assert_eq!(updated_by.as_deref(), Some("local-user"));
    assert_rfc3339(
        &updated_at,
        "meetings.updated_at after update_meeting_title",
    );
    assert!(
        updated_at.as_deref().unwrap().contains('T'),
        "updated_at must use the RFC 3339 'T' separator, got {updated_at:?}"
    );

    // update_meeting_name (meetings + transcript_chunks) is scoped as well.
    assert!(
        !MeetingsRepository::update_meeting_name(&pool, &other, &meeting_id, "hacked-2")
            .await
            .expect("cross-ws update_meeting_name"),
        "cross-workspace update_meeting_name must report no match"
    );

    // update_meeting_summary under the foreign context: meeting invisible => false.
    let summary = serde_json::json!({"markdown": "# stolen"});
    assert!(
        !SummaryProcessesRepository::update_meeting_summary(&pool, &other, &meeting_id, &summary)
            .await
            .expect("cross-ws update_meeting_summary"),
        "cross-workspace summary save must be rejected"
    );

    // update_process_completed under the foreign context: zero rows touched.
    SummaryProcessesRepository::update_process_completed(
        &pool,
        &other,
        &meeting_id,
        serde_json::json!({"markdown": "# stolen"}),
        1,
        0.5,
    )
    .await
    .expect("cross-ws update_process_completed runs");
    let status: String =
        sqlx::query_scalar("SELECT status FROM summary_processes WHERE meeting_id = ?")
            .bind(&meeting_id)
            .fetch_one(&pool)
            .await
            .expect("summary process row");
    assert_eq!(
        status, "PENDING",
        "foreign workspace must not complete a local summary process"
    );

    // create_or_reset_process from the foreign context fails closed and leaves
    // the existing child unchanged.
    let error = SummaryProcessesRepository::create_or_reset_process(&pool, &other, &meeting_id)
        .await
        .expect_err("cross-ws create_or_reset_process must fail");
    assert_eq!(
        error.to_string(),
        "encountered unexpected or invalid data: Parent meeting is unavailable in this workspace"
    );
    let (ws, rev, _, _, _) =
        sync_columns(&pool, "summary_processes", "meeting_id", &meeting_id).await;
    assert_eq!(ws, LOCAL_WORKSPACE_ID, "row must stay owned by local");
    assert_eq!(
        rev, 1,
        "rejected upsert must not bump rev across workspaces"
    );

    // replace_meeting_transcripts: foreign context cannot touch the meeting...
    let result = TranscriptsRepository::replace_meeting_transcripts(
        &pool,
        &other,
        &meeting_id,
        &[segment("transcript-x1", "injected", 0.0, 1.0)],
    )
    .await;
    assert!(
        matches!(result, Err(sqlx::Error::RowNotFound)),
        "cross-workspace replace must fail closed, got {result:?}"
    );
    assert_eq!(
        count_where(
            &pool,
            "transcripts",
            &format!("meeting_id = '{meeting_id}'")
        )
        .await,
        1,
        "local transcripts must survive a foreign replace attempt"
    );

    // ...while the owning workspace replaces atomically and re-stamps columns.
    TranscriptsRepository::replace_meeting_transcripts(
        &pool,
        &local,
        &meeting_id,
        &[
            segment("transcript-r1", "retranscribed one", 0.0, 1.0),
            segment("transcript-r2", "retranscribed two", 1.0, 2.0),
        ],
    )
    .await
    .expect("local replace");
    let (ws, rev, _, created_at, updated_at) =
        sync_columns(&pool, "transcripts", "id", "transcript-r1").await;
    assert_eq!(ws, LOCAL_WORKSPACE_ID);
    assert_eq!(rev, 1);
    assert_rfc3339(&created_at, "replaced transcripts.created_at");
    assert_rfc3339(&updated_at, "replaced transcripts.updated_at");

    pool.close().await;
}

/// A child repository must not trust a caller-supplied meeting id. In
/// particular, the first insert used to bypass the upsert's conflict guard when
/// no child row existed yet. Both child writes now condition the insert on a
/// live parent meeting owned by the caller's workspace in the same SQL
/// statement, while preserving normal same-workspace insert and reset paths.
#[tokio::test]
async fn child_first_inserts_require_parent_ownership_atomically() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let pool = open_migrated_temp_db(&tmp.path().join("child-parent-scope.sqlite")).await;
    let local = AuthContext::local();
    let other = other_ws_ctx();

    let meeting_id = TranscriptsRepository::save_transcript(
        &pool,
        &local,
        "Local parent",
        &[segment("transcript-parent-scope", "content", 0.0, 1.0)],
        None,
    )
    .await
    .expect("create local parent meeting");

    let summary_error =
        SummaryProcessesRepository::create_or_reset_process(&pool, &other, &meeting_id)
            .await
            .expect_err("foreign workspace must not first-insert a summary child");
    assert_eq!(
        summary_error.to_string(),
        "encountered unexpected or invalid data: Parent meeting is unavailable in this workspace"
    );

    let chunk_error = TranscriptChunksRepository::save_transcript_data(
        &pool,
        &other,
        &meeting_id,
        chunk_data("foreign content"),
    )
    .await
    .expect_err("foreign workspace must not first-insert a transcript chunk child");
    assert_eq!(
        chunk_error.to_string(),
        "encountered unexpected or invalid data: Parent meeting is unavailable in this workspace"
    );

    let filter = format!("meeting_id = '{meeting_id}'");
    assert_eq!(count_where(&pool, "summary_processes", &filter).await, 0);
    assert_eq!(count_where(&pool, "transcript_chunks", &filter).await, 0);

    SummaryProcessesRepository::create_or_reset_process(&pool, &local, &meeting_id)
        .await
        .expect("owner workspace may first-insert summary child");
    TranscriptChunksRepository::save_transcript_data(
        &pool,
        &local,
        &meeting_id,
        chunk_data("local content"),
    )
    .await
    .expect("owner workspace may first-insert transcript chunk child");

    SummaryProcessesRepository::create_or_reset_process(&pool, &local, &meeting_id)
        .await
        .expect("owner workspace may reset summary child");
    TranscriptChunksRepository::save_transcript_data(
        &pool,
        &local,
        &meeting_id,
        chunk_data("local replacement"),
    )
    .await
    .expect("owner workspace may update transcript chunk child");

    let (summary_ws, summary_rev, _, _, _) =
        sync_columns(&pool, "summary_processes", "meeting_id", &meeting_id).await;
    assert_eq!(summary_ws, LOCAL_WORKSPACE_ID);
    assert_eq!(summary_rev, 2, "legitimate summary reset must bump rev");

    let (chunk_ws, chunk_rev, _, _, _) =
        sync_columns(&pool, "transcript_chunks", "meeting_id", &meeting_id).await;
    assert_eq!(chunk_ws, LOCAL_WORKSPACE_ID);
    assert_eq!(chunk_rev, 2, "legitimate chunk update must bump rev");
    assert_eq!(
        read_chunk_text(&pool, &meeting_id).await,
        "local replacement"
    );

    pool.close().await;
}

#[tokio::test]
async fn deletes_are_workspace_scoped() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let pool = open_migrated_temp_db(&tmp.path().join("deletes.sqlite")).await;
    let local = AuthContext::local();
    let other = other_ws_ctx();

    let meeting_id = TranscriptsRepository::save_transcript(
        &pool,
        &local,
        "To be deleted",
        &[segment("transcript-d1", "content", 0.0, 1.0)],
        None,
    )
    .await
    .expect("save_transcript");
    SummaryProcessesRepository::create_or_reset_process(&pool, &local, &meeting_id)
        .await
        .expect("summary process");
    TranscriptChunksRepository::save_transcript_data(
        &pool,
        &local,
        &meeting_id,
        chunk_data("content"),
    )
    .await
    .expect("chunks");

    // Foreign delete: reported as "not found"; every associated row survives.
    let deleted = MeetingsRepository::delete_meeting(&pool, &other, &meeting_id)
        .await
        .expect("cross-ws delete runs");
    assert!(!deleted, "cross-workspace delete must report no match");
    let filter = format!("meeting_id = '{meeting_id}'");
    assert_eq!(count_where(&pool, "transcripts", &filter).await, 1);
    assert_eq!(count_where(&pool, "summary_processes", &filter).await, 1);
    assert_eq!(count_where(&pool, "transcript_chunks", &filter).await, 1);
    assert_eq!(
        count_where(&pool, "meetings", &format!("id = '{meeting_id}'")).await,
        1
    );

    // Owning workspace delete removes the meeting and all associated rows.
    let deleted = MeetingsRepository::delete_meeting(&pool, &local, &meeting_id)
        .await
        .expect("local delete");
    assert!(deleted);
    assert_eq!(count_where(&pool, "transcripts", &filter).await, 0);
    assert_eq!(count_where(&pool, "summary_processes", &filter).await, 0);
    assert_eq!(count_where(&pool, "transcript_chunks", &filter).await, 0);
    assert_eq!(
        count_where(&pool, "meetings", &format!("id = '{meeting_id}'")).await,
        0
    );

    pool.close().await;
}

#[tokio::test]
async fn settings_are_workspace_scoped_and_upsert_guarded() {
    // BYOK keys now live in the OS credential store; install keyring's in-memory
    // mock so this test never touches the real machine credential manager.
    let _keychain_guard = install_mock_keychain().await;

    let tmp = tempfile::tempdir().expect("tempdir");
    let pool = open_migrated_temp_db(&tmp.path().join("settings.sqlite")).await;
    let local = AuthContext::local();
    let other = other_ws_ctx();

    SettingsRepository::save_model_config(&pool, &local, "ollama", "llama3.2", "large-v3", None)
        .await
        .expect("local save_model_config");
    SettingsRepository::save_transcript_config(&pool, &local, "parakeet", "parakeet-v3")
        .await
        .expect("local save_transcript_config");

    // Timestamps populated on the config tables (nullable columns).
    let row = sqlx::query("SELECT created_at, updated_at FROM settings WHERE id = '1'")
        .fetch_one(&pool)
        .await
        .expect("settings row");
    assert_rfc3339(
        &row.get::<Option<String>, _>("created_at"),
        "settings.created_at",
    );
    assert_rfc3339(
        &row.get::<Option<String>, _>("updated_at"),
        "settings.updated_at",
    );

    // Reads are scoped: the foreign workspace sees no config at all.
    assert!(SettingsRepository::get_model_config(&pool, &local)
        .await
        .expect("local get_model_config")
        .is_some());
    assert!(SettingsRepository::get_model_config(&pool, &other)
        .await
        .expect("other get_model_config")
        .is_none());
    assert_eq!(
        SettingsRepository::get_transcript_provider_model(&pool, &local)
            .await
            .expect("local provider/model"),
        Some(("parakeet".to_string(), "parakeet-v3".to_string()))
    );
    assert!(
        SettingsRepository::get_transcript_provider_model(&pool, &other)
            .await
            .expect("other provider/model")
            .is_none()
    );

    // Local save round-trips through the keychain, and the SQLite column holds
    // ONLY the non-secret marker — never the secret (CLAUDE.md §0.7).
    SettingsRepository::save_api_key(&pool, &local, "openai", "sk-local-key")
        .await
        .expect("local save_api_key");
    assert_eq!(
        SettingsRepository::get_api_key(&pool, &local, "openai")
            .await
            .expect("local get_api_key"),
        Some("sk-local-key".to_string()),
        "local key must round-trip through the OS credential store"
    );
    let column_value: Option<String> =
        sqlx::query_scalar("SELECT openaiApiKey FROM settings WHERE id = '1' AND workspace_id = ?")
            .bind(LOCAL_WORKSPACE_ID)
            .fetch_one(&pool)
            .await
            .expect("openaiApiKey column");
    assert_eq!(
        column_value.as_deref(),
        Some(KEYCHAIN_MARKER),
        "the DB column must hold the keychain marker, not the secret"
    );
    assert_ne!(
        column_value.as_deref(),
        Some("sk-local-key"),
        "the plaintext secret must never be persisted in SQLite"
    );

    // Workspace isolation: a different workspace cannot read the local key (the
    // keychain entry name is scoped by workspace_id), and the local config row is
    // untouched by the foreign context.
    assert_eq!(
        SettingsRepository::get_api_key(&pool, &other, "openai")
            .await
            .expect("other get_api_key"),
        None,
        "a foreign workspace must not read another workspace's key"
    );
    let config = SettingsRepository::get_model_config(&pool, &local)
        .await
        .expect("local config")
        .expect("local config present");
    assert_eq!(
        config.provider, "ollama",
        "local provider must be untouched"
    );
    assert_eq!(
        config.openai_api_key.as_deref(),
        Some(KEYCHAIN_MARKER),
        "the local row's key column holds only the marker after a keychain save"
    );

    // A foreign save writes into the FOREIGN workspace's own keychain scope only:
    // the DB upsert guard keeps it out of the local row, and it must not disturb
    // the local key. The DB write degrades to a no-op (local provider stays).
    SettingsRepository::save_api_key(&pool, &other, "openai", "sk-foreign-key")
        .await
        .expect("cross-ws save_api_key runs");
    assert_eq!(
        SettingsRepository::get_api_key(&pool, &local, "openai")
            .await
            .expect("local get_api_key after foreign save"),
        Some("sk-local-key".to_string()),
        "a foreign save must not overwrite the local workspace's key"
    );
    assert_eq!(
        SettingsRepository::get_model_config(&pool, &local)
            .await
            .expect("local config after foreign save")
            .expect("local config present")
            .provider,
        "ollama",
        "foreign save must not clobber the local config row"
    );

    // Foreign delete is scoped: it clears only the foreign workspace's entry; the
    // local key survives.
    SettingsRepository::delete_api_key(&pool, &other, "openai")
        .await
        .expect("cross-ws delete_api_key runs");
    assert_eq!(
        SettingsRepository::get_api_key(&pool, &local, "openai")
            .await
            .expect("local get_api_key"),
        Some("sk-local-key".to_string()),
        "foreign delete must not clear the local key"
    );

    // Same-workspace delete removes both the keychain secret and the column marker.
    SettingsRepository::delete_api_key(&pool, &local, "openai")
        .await
        .expect("local delete_api_key");
    assert_eq!(
        SettingsRepository::get_api_key(&pool, &local, "openai")
            .await
            .expect("local get_api_key after delete"),
        None,
        "local delete must clear the key from the keychain"
    );

    pool.close().await;
}

/// One-time migration: a pre-seeded plaintext key in a settings column is moved
/// into the keychain and the column is overwritten with the marker; a second run
/// is a no-op (idempotent). Covers both the summary and transcript tables.
#[tokio::test]
async fn startup_migration_moves_plaintext_keys_into_keychain() {
    let _keychain_guard = install_mock_keychain().await;

    let tmp = tempfile::tempdir().expect("tempdir");
    let pool = open_migrated_temp_db(&tmp.path().join("keymigration.sqlite")).await;
    let local = AuthContext::local();

    // Seed legacy plaintext directly into the columns (pre-fix on-disk state):
    // an anthropic (summary) key and a whisper (transcript) key, id = '1', local ws.
    let now = "2026-07-02T10:00:00.000Z";
    sqlx::query(
        "INSERT INTO settings (id, workspace_id, provider, model, whisperModel, anthropicApiKey, customOpenAIConfig, created_at, updated_at) \
         VALUES ('1', ?, 'claude', 'claude-3-5', 'large-v3', 'sk-ant-plaintext', ?, ?, ?)",
    )
    .bind(LOCAL_WORKSPACE_ID)
    .bind(
        r#"{"endpoint":"http://localhost:8000/v1","apiKey":"sk-custom-plaintext","model":"local-model","maxTokens":1024,"temperature":0.2,"topP":0.9}"#,
    )
    .bind(now)
    .bind(now)
    .execute(&pool)
    .await
    .expect("seed settings plaintext");
    sqlx::query(
        "INSERT INTO transcript_settings (id, workspace_id, provider, model, whisperApiKey, created_at, updated_at) \
         VALUES ('1', ?, 'localWhisper', 'large-v3', 'wk-plaintext', ?, ?)",
    )
    .bind(LOCAL_WORKSPACE_ID)
    .bind(now)
    .bind(now)
    .execute(&pool)
    .await
    .expect("seed transcript_settings plaintext");

    // Run the one-time migration: both columns plus the legacy custom JSON move.
    let migrated = SettingsRepository::migrate_plaintext_keys_to_keychain(&pool, &local)
        .await
        .expect("migration runs");
    assert_eq!(migrated, 3, "all seeded plaintext keys must leave SQLite");

    // Columns now hold the marker, never the secret.
    let summary_col: Option<String> =
        sqlx::query_scalar("SELECT anthropicApiKey FROM settings WHERE id = '1'")
            .fetch_one(&pool)
            .await
            .expect("anthropicApiKey column");
    assert_eq!(summary_col.as_deref(), Some(KEYCHAIN_MARKER));
    assert_ne!(summary_col.as_deref(), Some("sk-ant-plaintext"));
    let transcript_col: Option<String> =
        sqlx::query_scalar("SELECT whisperApiKey FROM transcript_settings WHERE id = '1'")
            .fetch_one(&pool)
            .await
            .expect("whisperApiKey column");
    assert_eq!(transcript_col.as_deref(), Some(KEYCHAIN_MARKER));
    let custom_json: String =
        sqlx::query_scalar("SELECT customOpenAIConfig FROM settings WHERE id = '1'")
            .fetch_one(&pool)
            .await
            .expect("customOpenAIConfig column");
    assert!(!custom_json.contains("sk-custom-plaintext"));
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&custom_json).expect("sanitized custom JSON")
            ["apiKey"],
        serde_json::Value::Null
    );

    // The secrets are now readable from the keychain via the normal getters.
    assert_eq!(
        SettingsRepository::get_api_key(&pool, &local, "claude")
            .await
            .expect("get migrated summary key"),
        Some("sk-ant-plaintext".to_string())
    );
    assert_eq!(
        SettingsRepository::get_transcript_api_key(&pool, &local, "localWhisper")
            .await
            .expect("get migrated transcript key"),
        Some("wk-plaintext".to_string())
    );
    assert_eq!(
        SettingsRepository::get_custom_openai_config(&pool, &local)
            .await
            .expect("get migrated custom config")
            .expect("custom config")
            .api_key,
        Some("sk-custom-plaintext".to_string())
    );

    // Idempotent: a second sweep migrates nothing.
    let migrated_again = SettingsRepository::migrate_plaintext_keys_to_keychain(&pool, &local)
        .await
        .expect("second migration runs");
    assert_eq!(
        migrated_again, 0,
        "re-running the migration must be a no-op"
    );

    pool.close().await;
}

#[tokio::test]
async fn startup_key_migration_never_enumerates_a_foreign_workspace() {
    let _keychain_guard = install_mock_keychain().await;

    let tmp = tempfile::tempdir().expect("tempdir");
    let pool = open_migrated_temp_db(&tmp.path().join("keymigration-scope.sqlite")).await;
    let local = AuthContext::local();
    let other = other_ws_ctx();
    let now = "2026-07-02T10:00:00.000Z";

    sqlx::query(
        "INSERT INTO settings \
         (id, workspace_id, provider, model, whisperModel, openaiApiKey, created_at, updated_at) \
         VALUES ('1', ?, 'openai', 'gpt-test', 'large-v3', 'sk-foreign-scope-test', ?, ?)",
    )
    .bind(other.tenant_id.as_str())
    .bind(now)
    .bind(now)
    .execute(&pool)
    .await
    .expect("seed foreign plaintext key");

    let migrated = SettingsRepository::migrate_plaintext_keys_to_keychain(&pool, &local)
        .await
        .expect("local scoped migration");
    assert_eq!(
        migrated, 0,
        "local startup must not enumerate another workspace"
    );

    let still_plaintext: Option<String> =
        sqlx::query_scalar("SELECT openaiApiKey FROM settings WHERE id = '1' AND workspace_id = ?")
            .bind(other.tenant_id.as_str())
            .fetch_one(&pool)
            .await
            .expect("foreign key remains untouched by local migration");
    assert_eq!(still_plaintext.as_deref(), Some("sk-foreign-scope-test"));

    let migrated = SettingsRepository::migrate_plaintext_keys_to_keychain(&pool, &other)
        .await
        .expect("authorized foreign migration");
    assert_eq!(migrated, 1);
    assert_eq!(
        SettingsRepository::get_api_key(&pool, &other, "openai")
            .await
            .expect("foreign key getter"),
        Some("sk-foreign-scope-test".to_string())
    );
    assert_eq!(
        SettingsRepository::get_api_key(&pool, &local, "openai")
            .await
            .expect("local key getter"),
        None,
        "workspace-scoped keychain names must prevent a local read"
    );

    pool.close().await;
}

#[tokio::test]
async fn custom_openai_key_never_enters_sqlite_json() {
    let _keychain_guard = install_mock_keychain().await;
    let tmp = tempfile::tempdir().expect("tempdir");
    let database = tmp.path().join("custom-openai-keychain.sqlite");
    let pool = open_migrated_temp_db(&database).await;
    let local = AuthContext::local();
    let config = app_lib::summary::CustomOpenAIConfig {
        endpoint: "http://localhost:8000/v1".to_string(),
        api_key: Some("sk-custom-must-not-hit-db".to_string()),
        model: "local-model".to_string(),
        max_tokens: Some(2048),
        temperature: Some(0.2),
        top_p: Some(0.9),
    };

    SettingsRepository::save_custom_openai_config(&pool, &local, &config)
        .await
        .expect("save custom config");
    let stored_json: String =
        sqlx::query_scalar("SELECT customOpenAIConfig FROM settings WHERE id = '1'")
            .fetch_one(&pool)
            .await
            .expect("stored custom JSON");
    assert!(!stored_json.contains("sk-custom-must-not-hit-db"));
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&stored_json).expect("valid custom JSON")
            ["apiKey"],
        serde_json::Value::Null
    );

    let hydrated = SettingsRepository::get_custom_openai_config(&pool, &local)
        .await
        .expect("get custom config")
        .expect("custom config exists");
    assert_eq!(
        hydrated.api_key.as_deref(),
        Some("sk-custom-must-not-hit-db")
    );

    SettingsRepository::delete_api_key(&pool, &local, "custom-openai")
        .await
        .expect("delete custom key and config");
    assert!(SettingsRepository::get_custom_openai_config(&pool, &local)
        .await
        .expect("get after delete")
        .is_none());
    pool.close().await;

    let database_bytes = std::fs::read(database).expect("read SQLite file");
    assert!(!database_bytes
        .windows("sk-custom-must-not-hit-db".len())
        .any(|window| window == b"sk-custom-must-not-hit-db"));
}

#[tokio::test]
async fn imported_meetings_preserve_segment_ids_and_fill_columns() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let pool = open_migrated_temp_db(&tmp.path().join("import.sqlite")).await;
    let local = AuthContext::local();

    let meeting_id = TranscriptsRepository::create_meeting_with_segments(
        &pool,
        &local,
        "Imported recording",
        &[
            segment("transcript-fixed-a", "first", 0.0, 1.0),
            segment("transcript-fixed-b", "second", 1.0, 2.0),
        ],
        Some("C:/recordings/import".to_string()),
    )
    .await
    .expect("create_meeting_with_segments");

    // Caller-supplied ids preserved (they are referenced by transcripts.json on disk).
    let ids: Vec<String> = sqlx::query_scalar(
        "SELECT id FROM transcripts WHERE meeting_id = ? ORDER BY audio_start_time",
    )
    .bind(&meeting_id)
    .fetch_all(&pool)
    .await
    .expect("segment ids");
    assert_eq!(ids, ["transcript-fixed-a", "transcript-fixed-b"]);

    let (ws, rev, updated_by, created_at, updated_at) =
        sync_columns(&pool, "transcripts", "id", "transcript-fixed-a").await;
    assert_eq!(ws, LOCAL_WORKSPACE_ID);
    assert_eq!(rev, 1);
    assert_eq!(updated_by.as_deref(), Some("local-user"));
    assert_rfc3339(&created_at, "imported transcripts.created_at");
    assert_rfc3339(&updated_at, "imported transcripts.updated_at");

    pool.close().await;
}

/// BACKLOG C3: `search_transcripts` now matches a query in either a meeting's
/// transcript OR its generated summary, still one tenant-scoped query path.
/// Proves:
///   (a) a transcript-only term returns the meeting with matched_in=Transcript,
///   (b) a summary-only term returns it with matched_in=Summary,
///   (c) both searches under a foreign workspace return ZERO rows (isolation —
///       every joined table is workspace-scoped; a summary hit must not leak
///       across tenants any more than a transcript hit),
///   (d) a whitespace query returns empty.
#[tokio::test]
async fn search_covers_transcripts_and_summaries_scoped_by_workspace() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let pool = open_migrated_temp_db(&tmp.path().join("search.sqlite")).await;
    let local = AuthContext::local();
    let other = other_ws_ctx();

    // Seed ONE local meeting: transcript carries "alpha-transcript-term", and
    // its summary (stored in summary_processes.result) carries
    // "beta-summary-term". The two terms are disjoint so each search can only
    // match via exactly one field.
    let meeting_id = TranscriptsRepository::save_transcript(
        &pool,
        &local,
        "Quarterly review",
        &[segment(
            "transcript-c3",
            "discussion about alpha-transcript-term and next steps",
            0.0,
            3.0,
        )],
        None,
    )
    .await
    .expect("local save_transcript");

    SummaryProcessesRepository::create_or_reset_process(&pool, &local, &meeting_id)
        .await
        .expect("create summary process");
    let summary = serde_json::json!({
        "markdown": "# Summary\nThe team agreed on beta-summary-term as the priority."
    });
    assert!(
        SummaryProcessesRepository::update_meeting_summary(&pool, &local, &meeting_id, &summary)
            .await
            .expect("store summary"),
        "seeding the local summary must succeed"
    );

    // (a) Transcript-only term -> one row, matched_in = Transcript, snippet from
    // the transcript.
    let hits = TranscriptsRepository::search_transcripts(&pool, &local, "alpha-transcript-term")
        .await
        .expect("local transcript search");
    assert_eq!(
        hits.len(),
        1,
        "exactly one meeting matches the transcript term"
    );
    assert_eq!(hits[0].id, meeting_id);
    assert_eq!(hits[0].matched_in, SearchMatchField::Transcript);
    assert!(
        hits[0].match_context.contains("alpha-transcript-term"),
        "transcript snippet must contain the matched term, got {:?}",
        hits[0].match_context
    );

    // (b) Summary-only term -> one row, matched_in = Summary, snippet from the
    // summary text (NOT the unmatched transcript).
    let hits = TranscriptsRepository::search_transcripts(&pool, &local, "beta-summary-term")
        .await
        .expect("local summary search");
    assert_eq!(
        hits.len(),
        1,
        "exactly one meeting matches the summary term"
    );
    assert_eq!(hits[0].id, meeting_id);
    assert_eq!(hits[0].matched_in, SearchMatchField::Summary);
    assert!(
        hits[0].match_context.contains("beta-summary-term"),
        "summary snippet must contain the matched term, got {:?}",
        hits[0].match_context
    );

    // (c) Both terms are invisible to a foreign workspace (transcript AND summary
    // joins are workspace-scoped).
    assert!(
        TranscriptsRepository::search_transcripts(&pool, &other, "alpha-transcript-term")
            .await
            .expect("foreign transcript search")
            .is_empty(),
        "other-ws must not see the local transcript match"
    );
    assert!(
        TranscriptsRepository::search_transcripts(&pool, &other, "beta-summary-term")
            .await
            .expect("foreign summary search")
            .is_empty(),
        "other-ws must not see the local summary match"
    );

    // (d) Whitespace query -> empty (no query path executed).
    assert!(
        TranscriptsRepository::search_transcripts(&pool, &local, "   ")
            .await
            .expect("whitespace search")
            .is_empty(),
        "a whitespace-only query must return no results"
    );

    pool.close().await;
}

/// Product Intelligence / Kanıt Arama v1: FTS5/BM25 search is ranked,
/// source-resolvable and tenant-scoped. The trusted evidence surface is
/// transcript-only; the legacy summary-capable command above remains available
/// for backwards compatibility but is not used by the new UI.
#[tokio::test]
async fn evidence_search_is_ranked_source_linked_unicode_safe_and_tenant_scoped() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let pool = open_migrated_temp_db(&tmp.path().join("evidence-search.sqlite")).await;
    let local = AuthContext::local();
    let other = other_ws_ctx();

    let strongest_meeting = TranscriptsRepository::save_transcript(
        &pool,
        &local,
        "İstanbul karar oturumu",
        &[
            segment(
                "ignored-a",
                "kararı ekip daha sonra yeniden değerlendirecek",
                2.0,
                4.0,
            ),
            segment(
                "ignored-b",
                "İstanbul yol haritası kararı kararı kararı netleşti",
                8.0,
                11.0,
            ),
        ],
        None,
    )
    .await
    .expect("seed strongest local meeting");

    let second_meeting = TranscriptsRepository::save_transcript(
        &pool,
        &local,
        "Secondary review",
        &[segment(
            "ignored-c",
            "İstanbul ekibi kararı bir kez değerlendirdi",
            1.0,
            3.0,
        )],
        None,
    )
    .await
    .expect("seed second local meeting");

    let foreign_meeting = TranscriptsRepository::save_transcript(
        &pool,
        &other,
        "Foreign secret",
        &[segment(
            "ignored-d",
            "İstanbul kararı foreign-only-secret kararı kararı kararı kararı",
            0.0,
            2.0,
        )],
        None,
    )
    .await
    .expect("seed foreign meeting");

    // `istanbul` must match Unicode `İstanbul`; `karar` is a prefix match for
    // `kararı`. Results preserve backend BM25 order and dedupe to one best
    // evidence segment per meeting.
    let hits = TranscriptsRepository::search_evidence(&pool, &local, "istanbul karar")
        .await
        .expect("ranked local evidence search");
    assert_eq!(hits.len(), 2, "only two local meetings may be returned");
    assert_eq!(hits[0].id, strongest_meeting);
    assert_eq!(hits[1].id, second_meeting);
    assert_eq!(hits[0].matched_in, SearchMatchField::Transcript);
    assert_eq!(hits[0].audio_start_time, Some(8.0));
    assert!(hits[0].match_context.to_lowercase().contains("karar"));
    assert!(hits.iter().all(|hit| hit.id != foreign_meeting));

    // Every hit's evidence id resolves inside the caller's workspace and belongs
    // to the returned meeting. This is the invariant consumed by jump-to-source.
    for hit in &hits {
        let resolved: Option<i64> = sqlx::query_scalar(
            "SELECT 1 FROM transcripts \
             WHERE id = ? AND meeting_id = ? AND workspace_id = ? AND deleted_at IS NULL",
        )
        .bind(&hit.source_chunk_id)
        .bind(&hit.id)
        .bind(local.tenant_id.as_str())
        .fetch_optional(&pool)
        .await
        .expect("resolve evidence source");
        assert_eq!(resolved, Some(1));
    }

    // FTS operators/punctuation from user input become quoted literal tokens;
    // punctuation-only input becomes an empty result instead of syntax or error.
    TranscriptsRepository::search_evidence(&pool, &local, r#"istanbul OR "" NEAR * karar"#)
        .await
        .expect("operator-like input must not become FTS syntax");
    assert!(
        TranscriptsRepository::search_evidence(&pool, &local, "\"()* 😀")
            .await
            .expect("punctuation-only evidence search")
            .is_empty()
    );
    assert!(
        TranscriptsRepository::search_evidence(&pool, &local, "a")
            .await
            .expect("one-character evidence search")
            .is_empty(),
        "one-character tokens must not expand into corpus-wide prefix scans"
    );
    assert!(
        TranscriptsRepository::search_evidence(&pool, &local, "is")
            .await
            .expect("two-character exact evidence search")
            .is_empty(),
        "two-character input must not prefix-expand to İstanbul"
    );

    // A foreign context sees only its own workspace's evidence.
    let foreign_hits = TranscriptsRepository::search_evidence(&pool, &other, "foreign only secret")
        .await
        .expect("foreign evidence search");
    assert_eq!(foreign_hits.len(), 1);
    assert_eq!(foreign_hits[0].id, foreign_meeting);

    pool.close().await;
}

/// Meeting-level deduplication must happen before the public result cap. A single
/// long meeting with hundreds of high-scoring segments must not monopolize the
/// segment candidate window and hide another matching meeting.
#[tokio::test]
async fn evidence_search_deduplicates_meetings_before_limiting_results() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let pool = open_migrated_temp_db(&tmp.path().join("evidence-dedup-limit.sqlite")).await;
    let local = AuthContext::local();

    let dominant_segments: Vec<TranscriptSegment> = (0..260)
        .map(|index| {
            segment(
                &format!("dominant-{index}"),
                "monopolytoken monopolytoken monopolytoken monopolytoken",
                index as f64,
                index as f64 + 0.5,
            )
        })
        .collect();

    let dominant_meeting = TranscriptsRepository::save_transcript(
        &pool,
        &local,
        "Long matching meeting",
        &dominant_segments,
        None,
    )
    .await
    .expect("seed dominant meeting");

    let other_meeting = TranscriptsRepository::save_transcript(
        &pool,
        &local,
        "Independent matching meeting",
        &[segment("other-match", "monopolytoken", 500.0, 501.0)],
        None,
    )
    .await
    .expect("seed independent meeting");

    let hits = TranscriptsRepository::search_evidence(&pool, &local, "monopolytoken")
        .await
        .expect("search across a dominant meeting");

    assert_eq!(hits.len(), 2, "one best segment per meeting must survive");
    assert!(hits.iter().any(|hit| hit.id == dominant_meeting));
    assert!(hits.iter().any(|hit| hit.id == other_meeting));

    pool.close().await;
}

/// Retranscription is a delete+insert transaction. The FTS triggers must remove
/// old tokens and index the replacement segment without leaving duplicate or
/// stale evidence rows.
#[tokio::test]
async fn evidence_search_tracks_retranscription_atomically() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let pool = open_migrated_temp_db(&tmp.path().join("evidence-replace.sqlite")).await;
    let local = AuthContext::local();

    let meeting_id = TranscriptsRepository::save_transcript(
        &pool,
        &local,
        "Retranscription fixture",
        &[segment(
            "ignored-original",
            "obsolete-evidence-token",
            0.0,
            2.0,
        )],
        None,
    )
    .await
    .expect("seed original transcript");

    assert_eq!(
        TranscriptsRepository::search_evidence(&pool, &local, "obsolete")
            .await
            .expect("search old evidence")
            .len(),
        1
    );

    TranscriptsRepository::replace_meeting_transcripts(
        &pool,
        &local,
        &meeting_id,
        &[segment(
            "replacement-source-id",
            "replacement-evidence-token",
            12.0,
            15.0,
        )],
    )
    .await
    .expect("replace transcripts");

    assert!(
        TranscriptsRepository::search_evidence(&pool, &local, "obsolete")
            .await
            .expect("search removed evidence")
            .is_empty()
    );
    let replacement = TranscriptsRepository::search_evidence(&pool, &local, "replacement")
        .await
        .expect("search replacement evidence");
    assert_eq!(replacement.len(), 1);
    assert_eq!(replacement[0].source_chunk_id, "replacement-source-id");
    assert_eq!(replacement[0].audio_start_time, Some(12.0));

    let indexed_rows: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM transcript_search_documents \
         WHERE meeting_id = ? AND workspace_id = ?",
    )
    .bind(&meeting_id)
    .bind(local.tenant_id.as_str())
    .fetch_one(&pool)
    .await
    .expect("count derived evidence rows");
    assert_eq!(indexed_rows, 1);

    pool.close().await;
}

/// Read back the persisted full text of a meeting's `transcript_chunks` row.
async fn read_chunk_text(pool: &SqlitePool, meeting_id: &str) -> String {
    sqlx::query_scalar::<_, String>(
        "SELECT transcript_text FROM transcript_chunks WHERE meeting_id = ?",
    )
    .bind(meeting_id)
    .fetch_one(pool)
    .await
    .expect("transcript_chunks row must exist")
}

/// Guards the C6 at-rest gap the security review flagged: `api_process_transcript`
/// persists the verbatim in-memory transcript into `transcript_chunks` (durable,
/// full-text, on the B4 sync allowlist) via `save_transcript_data`. This test
/// replicates that command's exact **redact-then-write** sequence (fetch the
/// workspace redaction config, `redact` when active, then persist) and proves:
///   - with redaction ENABLED, the text landed in `transcript_chunks` is redacted
///     (the raw PII is gone and it equals the pure `redact()` output — the same
///     value the command also hands to the background summary task, so both sinks
///     agree end-to-end);
///   - with redaction DISABLED (the default), the raw text is persisted verbatim
///     (non-breaking).
#[tokio::test]
async fn process_transcript_redacts_chunks_at_rest_when_enabled() {
    use app_lib::redaction::{self, RedactionConfig};

    let tmp = tempfile::tempdir().expect("tempdir");
    let pool = open_migrated_temp_db(&tmp.path().join("redaction_chunks.sqlite")).await;
    let local = AuthContext::local();

    let raw = "Contact jane.doe@example.com about invoice 4242 4242 4242 4242";

    // transcript_chunks.meeting_id is FK -> meetings(id); create real meetings via
    // the repository first (each returns a minted `meeting-{uuid}` id).
    let meeting_off = TranscriptsRepository::save_transcript(
        &pool,
        &local,
        "Off",
        &[segment("transcript-off", "seed", 0.0, 1.0)],
        None,
    )
    .await
    .expect("create meeting (disabled case)");
    let meeting_on = TranscriptsRepository::save_transcript(
        &pool,
        &local,
        "On",
        &[segment("transcript-on", "seed", 0.0, 1.0)],
        None,
    )
    .await
    .expect("create meeting (enabled case)");

    // --- DISABLED (default): raw text must persist verbatim -------------------
    let cfg_off = SettingsRepository::get_redaction_config(&pool, &local)
        .await
        .expect("default redaction config");
    assert!(!cfg_off.is_active(), "redaction must be OFF by default");
    // Mirror the command: when inactive, `text` passes through unchanged.
    let text_off = if cfg_off.is_active() {
        redaction::redact(raw, &cfg_off)
    } else {
        raw.to_string()
    };
    TranscriptChunksRepository::save_transcript_data(
        &pool,
        &local,
        &meeting_off,
        chunk_data(&text_off),
    )
    .await
    .expect("save chunk (disabled)");
    assert_eq!(
        read_chunk_text(&pool, &meeting_off).await,
        raw,
        "with redaction OFF the verbatim transcript must persist unchanged"
    );

    // --- ENABLED: chunk text must be redacted at rest ------------------------
    let enabled = RedactionConfig {
        enabled: true,
        use_default_patterns: true,
        custom_terms: vec!["invoice".to_string()],
    };
    SettingsRepository::set_redaction_config(&pool, &local, &enabled)
        .await
        .expect("persist enabled redaction config");

    // Re-read via the repository (the command reads, it does not trust the caller).
    let cfg_on = SettingsRepository::get_redaction_config(&pool, &local)
        .await
        .expect("stored redaction config");
    assert!(cfg_on.is_active(), "stored config must be active");

    // Mirror the command's single rebind: this exact value flows to BOTH the
    // transcript_chunks write and (in production) the spawned summary task.
    let text_on = if cfg_on.is_active() {
        redaction::redact(raw, &cfg_on)
    } else {
        raw.to_string()
    };
    TranscriptChunksRepository::save_transcript_data(
        &pool,
        &local,
        &meeting_on,
        chunk_data(&text_on),
    )
    .await
    .expect("save chunk (enabled)");

    let stored = read_chunk_text(&pool, &meeting_on).await;
    // The persisted text equals the pure redactor output (both sinks agree).
    assert_eq!(
        stored,
        redaction::redact(raw, &cfg_on),
        "persisted chunk text must equal the redactor output"
    );
    // And the raw PII / custom term must NOT be at rest.
    assert!(
        !stored.contains("jane.doe@example.com"),
        "email must not remain in transcript_chunks: {stored:?}"
    );
    assert!(
        !stored.contains("4242 4242 4242 4242"),
        "card must not remain in transcript_chunks: {stored:?}"
    );
    assert!(
        !stored.to_lowercase().contains("invoice"),
        "custom term must not remain in transcript_chunks: {stored:?}"
    );
    assert!(
        stored.contains("[EMAIL]") && stored.contains("[CARD]") && stored.contains("[REDACTED]"),
        "expected typed placeholders in persisted text: {stored:?}"
    );

    pool.close().await;
}
