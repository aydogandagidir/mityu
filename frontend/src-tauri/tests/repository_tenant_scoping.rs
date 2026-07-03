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
}

fn install_mock_keychain() {
    mem_keychain::install();
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

    // create_or_reset_process from the foreign context conflicts on the primary
    // key but the workspace guard turns it into a no-op (rev/owner unchanged).
    SummaryProcessesRepository::create_or_reset_process(&pool, &other, &meeting_id)
        .await
        .expect("cross-ws create_or_reset_process runs");
    let (ws, rev, _, _, _) =
        sync_columns(&pool, "summary_processes", "meeting_id", &meeting_id).await;
    assert_eq!(ws, LOCAL_WORKSPACE_ID, "row must stay owned by local");
    assert_eq!(rev, 1, "guarded upsert must not bump rev across workspaces");

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
    install_mock_keychain();

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
    install_mock_keychain();

    let tmp = tempfile::tempdir().expect("tempdir");
    let pool = open_migrated_temp_db(&tmp.path().join("keymigration.sqlite")).await;
    let local = AuthContext::local();

    // Seed legacy plaintext directly into the columns (pre-fix on-disk state):
    // an anthropic (summary) key and a whisper (transcript) key, id = '1', local ws.
    let now = "2026-07-02T10:00:00.000Z";
    sqlx::query(
        "INSERT INTO settings (id, workspace_id, provider, model, whisperModel, anthropicApiKey, created_at, updated_at) \
         VALUES ('1', ?, 'claude', 'claude-3-5', 'large-v3', 'sk-ant-plaintext', ?, ?)",
    )
    .bind(LOCAL_WORKSPACE_ID)
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

    // Run the one-time migration: exactly two keys move.
    let migrated = SettingsRepository::migrate_plaintext_keys_to_keychain(&pool)
        .await
        .expect("migration runs");
    assert_eq!(migrated, 2, "both seeded plaintext keys must migrate");

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

    // Idempotent: a second sweep migrates nothing.
    let migrated_again = SettingsRepository::migrate_plaintext_keys_to_keychain(&pool)
        .await
        .expect("second migration runs");
    assert_eq!(
        migrated_again, 0,
        "re-running the migration must be a no-op"
    );

    pool.close().await;
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
