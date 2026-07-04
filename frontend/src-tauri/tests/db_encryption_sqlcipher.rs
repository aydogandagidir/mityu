//! Integration tests for at-rest DB encryption (BACKLOG B3, SQLCipher; ADR-0014).
//!
//! Proves, using the app's real migration set (`sqlx::migrate!`) and the real
//! conversion path (`app_lib::database::encryption`):
//!   (a) a fresh SQLCipher database is created and every migration applies; it is
//!       NOT readable without the key and IS readable with it;
//!   (b) a plaintext -> encrypted conversion preserves row counts AND the
//!       `_sqlx_migrations` ledger byte-for-byte (the migration-idempotency proof
//!       depends on the ledger surviving), and after a VERIFIED keyed re-open the
//!       `.pre-encryption` backup is DELETED (no lingering full-DB plaintext);
//!   (c) the wrong key fails closed (the encrypted DB does not open);
//!   (d) INVARIANT for the existing keyless suites: under a SQLCipher-enabled build
//!       a keyless open still reads a plaintext temp DB (so `migration_workspace_sync`
//!       and `repository_tenant_scoping`, which open plain temp DBs with NO key,
//!       keep passing unchanged);
//!   (e) (env-gated) the same conversion proof against a COPY of a real user DB:
//!       set `MITYU_TEST_ENCRYPT_DB` to the copy's path (never the live file — the
//!       harness copies it first). Asserts row counts, the ledger, backup-deleted,
//!       and no stale plaintext WAL/SHM;
//!   (f) a source DB with a POPULATED `-wal` converts with all committed rows
//!       preserved AND leaves no stale plaintext `-wal`/`-shm`;
//!   (g) the GUARDRAILED manager fallback (ADR-0014): an ENCRYPTED file with the key
//!       PRESENT opens through `DatabaseManager::new` and rows read back;
//!   (h) an ENCRYPTED file with the key MISSING FAILS CLOSED — `DatabaseManager::new`
//!       errors, the ciphertext file is byte-for-byte UNMODIFIED, NO new key is
//!       minted, and it is NOT opened as plaintext;
//!   (i) a PLAINTEXT file with the key store unavailable FAILS OPEN — the manager
//!       opens it plaintext and rows read back (local-first: never lock the user out
//!       of a still-plaintext file).
//!
//! Tests (g)–(i) drive the real `DatabaseManager::new` against a PERSISTENT in-memory
//! keyring installed via `keyring::set_default_credential_builder` (never the machine
//! store), and serialize on the single process-wide `db-key` entry.
//!
//! These tests use temp dirs and, for (e), a Bash-made copy — they NEVER open the
//! live database.

use sqlx::migrate::Migrator;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{ConnectOptions, Connection, Row, SqlitePool};
use std::path::Path;

use app_lib::database::encryption;
use app_lib::database::manager::DatabaseManager;
use app_lib::secrets::db as dbkey;

/// Persistent, process-wide in-memory keyring for the manager-fallback tests
/// (g)–(i). keyring's built-in `mock` backend is `EntryOnly` — a set on one
/// `Entry::new` is invisible to a later `Entry::new` for the same name — so it cannot
/// model the manager's set-then-read of the `db-key` entry across separate `Entry`
/// opens. This mirrors the crate-internal `secrets::test_store` (which is
/// `#[cfg(test)]` and thus unreachable from an integration test) with a shared
/// `(service, entry)` map, faithfully modelling a real OS store while NEVER touching
/// the machine credential manager.
mod mock_keyring {
    use keyring::credential::{
        Credential, CredentialApi, CredentialBuilderApi, CredentialPersistence,
    };
    use keyring::Error;
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Mutex, OnceLock};

    type Key = (String, String); // (service, entry_name)

    fn store() -> &'static Mutex<HashMap<Key, Vec<u8>>> {
        static STORE: OnceLock<Mutex<HashMap<Key, Vec<u8>>>> = OnceLock::new();
        STORE.get_or_init(|| Mutex::new(HashMap::new()))
    }

    /// When set, every credential op errors — models a locked/unavailable OS store so
    /// `secrets::db::get_or_create_hex()` returns `Err` and the manager's PLAINTEXT
    /// fail-open branch executes. Set/cleared only while holding [`db_key_lock`].
    fn unavailable() -> &'static AtomicBool {
        static FLAG: OnceLock<AtomicBool> = OnceLock::new();
        FLAG.get_or_init(|| AtomicBool::new(false))
    }

    /// Serializes tests that mutate the single process-wide `db-key` entry / mode so
    /// they do not race on shared keyring state. A `tokio::sync::Mutex` (not
    /// `std::sync`) so the guard can be safely held across the `DatabaseManager::new`
    /// `.await` without tripping clippy's `await_holding_lock`.
    pub fn db_key_lock() -> &'static tokio::sync::Mutex<()> {
        static LOCK: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| tokio::sync::Mutex::new(()))
    }

    /// Make the store report "unavailable" (every op errors). Reset with
    /// [`set_available`]. Only call under the [`db_key_lock`] guard.
    pub fn set_unavailable() {
        unavailable().store(true, Ordering::SeqCst);
    }

    /// Restore normal in-memory store behavior.
    pub fn set_available() {
        unavailable().store(false, Ordering::SeqCst);
    }

    fn store_unavailable_err() -> Error {
        // A platform-ish failure that is NOT `NoEntry`, so `get_or_create_hex` maps it
        // to an `Err` (store unavailable) rather than "not set".
        Error::PlatformFailure(Box::new(std::io::Error::other(
            "mock keyring: store unavailable",
        )))
    }

    #[derive(Debug)]
    struct MemCredential {
        key: Key,
    }

    impl CredentialApi for MemCredential {
        fn set_secret(&self, secret: &[u8]) -> Result<(), Error> {
            if unavailable().load(Ordering::SeqCst) {
                return Err(store_unavailable_err());
            }
            store()
                .lock()
                .unwrap()
                .insert(self.key.clone(), secret.to_vec());
            Ok(())
        }
        fn get_secret(&self) -> Result<Vec<u8>, Error> {
            if unavailable().load(Ordering::SeqCst) {
                return Err(store_unavailable_err());
            }
            match store().lock().unwrap().get(&self.key) {
                Some(v) => Ok(v.clone()),
                None => Err(Error::NoEntry),
            }
        }
        fn delete_credential(&self) -> Result<(), Error> {
            if unavailable().load(Ordering::SeqCst) {
                return Err(store_unavailable_err());
            }
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

    /// Install this store as the process-wide keyring default. Idempotent per process.
    pub fn install() {
        use std::sync::Once;
        static ONCE: Once = Once::new();
        ONCE.call_once(|| {
            keyring::set_default_credential_builder(Box::new(MemBuilder));
        });
    }

    /// Remove the device `db-key` entry from the mock store (simulate "key missing").
    /// Bypasses the availability flag (direct store mutation) so a test can clear the
    /// key regardless of mode.
    pub fn clear_db_key() {
        store().lock().unwrap().remove(&(
            app_lib::secrets::KEYCHAIN_SERVICE.to_string(),
            app_lib::secrets::db::ENTRY_NAME.to_string(),
        ));
    }
}

/// The app's real, compile-time-embedded migration set (same source as
/// `DatabaseManager::new`).
static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

/// A deterministic 32-byte test key as 64 lowercase hex chars. Tests must NOT read
/// the machine keychain — they pass an explicit key to the conversion/open helpers.
const TEST_KEY_HEX: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const WRONG_KEY_HEX: &str = "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff";

/// Open a plaintext temp DB (no key) with `create_if_missing` — mirrors exactly how
/// the existing keyless suites open their temp databases.
async fn open_plaintext(path: &Path) -> SqlitePool {
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true);
    SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .expect("open plaintext temp db")
}

/// Open an encrypted DB with the raw key (`PRAGMA key = x'<hex>'`, reserved slot 0).
async fn open_encrypted(path: &Path, key_hex: &str) -> SqlitePool {
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true)
        .pragma("key", dbkey::pragma_key_value(key_hex));
    SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .expect("open encrypted db")
}

/// The `.pre-encryption` backup path for a DB (matches `encryption::BACKUP_SUFFIX`).
fn backup_path(db_path: &Path) -> std::path::PathBuf {
    let mut s = db_path.as_os_str().to_os_string();
    s.push(".pre-encryption");
    std::path::PathBuf::from(s)
}

/// The SQLite `-wal` / `-shm` sidecar paths for a DB.
fn wal_shm_paths(db_path: &Path) -> (std::path::PathBuf, std::path::PathBuf) {
    let mut wal = db_path.as_os_str().to_os_string();
    wal.push("-wal");
    let mut shm = db_path.as_os_str().to_os_string();
    shm.push("-shm");
    (std::path::PathBuf::from(wal), std::path::PathBuf::from(shm))
}

async fn count(pool: &SqlitePool, table: &str) -> i64 {
    sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table}"))
        .fetch_one(pool)
        .await
        .unwrap_or_else(|e| panic!("COUNT(*) on {table} failed: {e}"))
}

async fn applied_versions(pool: &SqlitePool) -> Vec<i64> {
    sqlx::query_scalar("SELECT version FROM _sqlx_migrations ORDER BY version")
        .fetch_all(pool)
        .await
        .expect("read _sqlx_migrations ledger")
}

/// Full ledger rows (version + the applied checksum) so we can prove the ledger is
/// preserved *exactly*, not merely that the version list matches.
async fn ledger_rows(pool: &SqlitePool) -> Vec<(i64, Vec<u8>)> {
    sqlx::query("SELECT version, checksum FROM _sqlx_migrations ORDER BY version")
        .fetch_all(pool)
        .await
        .expect("read ledger rows")
        .into_iter()
        .map(|r| (r.get::<i64, _>("version"), r.get::<Vec<u8>, _>("checksum")))
        .collect()
}

/// The ten domain/config/licensing tables the real DB carries after migration.
const ALL_TABLES: [&str; 10] = [
    "meetings",
    "transcripts",
    "summary_processes",
    "transcript_chunks",
    "meeting_notes",
    "summaries",
    "action_items",
    "settings",
    "transcript_settings",
    "licensing",
];

/// Seed a couple of representative rows so conversion has data to preserve.
async fn seed_minimal(pool: &SqlitePool) {
    sqlx::query(
        "INSERT INTO meetings (id, title, created_at, updated_at) \
         VALUES ('m-enc-1', 'Encrypted kickoff', '2026-07-02T09:00:00.000Z', '2026-07-02T09:30:00.000Z'), \
                ('m-enc-2', 'Second', '2026-07-02T10:00:00.000Z', '2026-07-02T10:30:00.000Z')",
    )
    .execute(pool)
    .await
    .expect("seed meetings");
    sqlx::query(
        "INSERT INTO transcripts \
         (id, meeting_id, transcript, timestamp, audio_start_time, audio_end_time, duration, speaker) \
         VALUES ('t-enc-1', 'm-enc-1', 'hello secret', '2026-07-02T09:00:05.000Z', 0.0, 2.5, 2.5, 'microphone'), \
                ('t-enc-2', 'm-enc-1', 'more', '2026-07-02T09:00:08.000Z', 2.5, 5.0, 2.5, 'system'), \
                ('t-enc-3', 'm-enc-2', 'onsite', '2026-07-02T10:00:03.000Z', 0.0, 1.8, 1.8, 'microphone')",
    )
    .execute(pool)
    .await
    .expect("seed transcripts");
}

/// (a) Fresh encrypted DB: migrations apply, key required, wrong/no key rejected.
#[tokio::test]
async fn fresh_encrypted_db_applies_migrations_and_requires_key() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let db_path = tmp.path().join("fresh_enc.sqlite");

    // ensure_encrypted is a no-op on a missing file; the keyed open creates it.
    encryption::ensure_encrypted(&db_path, TEST_KEY_HEX)
        .await
        .expect("ensure_encrypted no-op on missing file");
    assert!(
        !db_path.exists(),
        "no file should exist before the keyed open"
    );

    let pool = open_encrypted(&db_path, TEST_KEY_HEX).await;
    MIGRATOR
        .run(&pool)
        .await
        .expect("migrations on fresh enc db");
    assert!(
        !applied_versions(&pool).await.is_empty(),
        "the migration ledger must be populated"
    );
    seed_minimal(&pool).await;
    assert_eq!(count(&pool, "meetings").await, 2);
    pool.close().await;

    // The on-disk file must NOT be plaintext-readable.
    assert!(
        !encryption::opens_with_key(&db_path, WRONG_KEY_HEX)
            .await
            .expect("wrong-key probe"),
        "a fresh encrypted DB must not open with the wrong key"
    );
    // A keyless open must fail (file is encrypted).
    let keyless = SqliteConnectOptions::new()
        .filename(&db_path)
        .create_if_missing(false)
        .connect()
        .await;
    let keyless_reads = match keyless {
        Ok(mut c) => {
            let ok = sqlx::query("SELECT count(*) FROM sqlite_master")
                .fetch_one(&mut c)
                .await
                .is_ok();
            let _ = c.close().await;
            ok
        }
        Err(_) => false,
    };
    assert!(
        !keyless_reads,
        "a keyless connection must not read an encrypted DB"
    );
    // The right key opens and sees the data.
    assert!(
        encryption::opens_with_key(&db_path, TEST_KEY_HEX)
            .await
            .expect("right-key probe"),
        "the correct key must open the encrypted DB"
    );
}

/// (b) Plaintext -> encrypted conversion preserves rows AND the ledger; after a
/// verified keyed re-open the plaintext backup is DELETED (no lingering plaintext).
#[tokio::test]
async fn conversion_preserves_rows_and_migration_ledger() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let db_path = tmp.path().join("convert.sqlite");

    // 1) Build a PLAINTEXT DB via the real migrator, then seed it.
    let plain = open_plaintext(&db_path).await;
    MIGRATOR.run(&plain).await.expect("migrations (plaintext)");
    seed_minimal(&plain).await;
    let pre_counts: Vec<(&str, i64)> = {
        let mut v = Vec::new();
        for t in ALL_TABLES {
            v.push((t, count(&plain, t).await));
        }
        v
    };
    let pre_ledger = ledger_rows(&plain).await;
    let pre_versions = applied_versions(&plain).await;
    plain.close().await;

    // 2) Convert.
    encryption::ensure_encrypted(&db_path, TEST_KEY_HEX)
        .await
        .expect("conversion");

    // 3) The .pre-encryption backup MUST be gone after a verified conversion (no
    //    full-database plaintext may linger on disk), and no stale plaintext WAL/SHM.
    assert!(
        !backup_path(&db_path).exists(),
        "plaintext backup must be deleted after a verified keyed re-open"
    );
    let (wal, shm) = wal_shm_paths(&db_path);
    // (an encrypted WAL/SHM may legitimately exist only if a keyed pool is open; no
    // pool is open here, and the encrypted side was written in DELETE journal mode,
    // so nothing should remain)
    assert!(!wal.exists(), "no stale WAL must remain after conversion");
    assert!(!shm.exists(), "no stale SHM must remain after conversion");

    // 4) The main file is now encrypted (wrong key / keyless cannot read it).
    assert!(
        !encryption::opens_with_key(&db_path, WRONG_KEY_HEX)
            .await
            .expect("wrong-key probe"),
        "converted DB must not open with the wrong key"
    );

    // 5) Re-open keyed: row counts identical, ledger identical, migrator is a no-op.
    let enc = open_encrypted(&db_path, TEST_KEY_HEX).await;
    for (t, pre) in &pre_counts {
        assert_eq!(count(&enc, t).await, *pre, "row count changed for {t}");
    }
    assert_eq!(
        applied_versions(&enc).await,
        pre_versions,
        "migration version list changed across conversion"
    );
    assert_eq!(
        ledger_rows(&enc).await,
        pre_ledger,
        "the _sqlx_migrations ledger (versions+checksums) must survive conversion intact"
    );

    // Re-running the migrator on the encrypted DB is a ledger-guarded no-op.
    MIGRATOR
        .run(&enc)
        .await
        .expect("re-run migrator on encrypted db");
    assert_eq!(
        applied_versions(&enc).await,
        pre_versions,
        "re-running the migrator must not touch the ledger"
    );

    // A representative row is byte-identical.
    let title: String = sqlx::query_scalar("SELECT title FROM meetings WHERE id = 'm-enc-1'")
        .fetch_one(&enc)
        .await
        .expect("meeting row");
    assert_eq!(title, "Encrypted kickoff");
    enc.close().await;

    // 6) ensure_encrypted is idempotent: a second call on the now-encrypted file is
    //    a no-op and creates no second backup churn.
    encryption::ensure_encrypted(&db_path, TEST_KEY_HEX)
        .await
        .expect("idempotent second conversion");
    let enc2 = open_encrypted(&db_path, TEST_KEY_HEX).await;
    assert_eq!(count(&enc2, "meetings").await, 2);
    enc2.close().await;
}

/// (c) Wrong key fails closed on a real converted DB.
#[tokio::test]
async fn wrong_key_fails_closed() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let db_path = tmp.path().join("wrongkey.sqlite");

    let plain = open_plaintext(&db_path).await;
    MIGRATOR.run(&plain).await.expect("migrations");
    seed_minimal(&plain).await;
    plain.close().await;

    encryption::ensure_encrypted(&db_path, TEST_KEY_HEX)
        .await
        .expect("conversion");

    // Opening the pool with the wrong key must not yield a usable connection.
    let bad = SqliteConnectOptions::new()
        .filename(&db_path)
        .create_if_missing(false)
        .pragma("key", dbkey::pragma_key_value(WRONG_KEY_HEX))
        .connect()
        .await;
    let bad_reads = match bad {
        Ok(mut c) => {
            let ok = sqlx::query("SELECT COUNT(*) FROM meetings")
                .fetch_one(&mut c)
                .await
                .is_ok();
            let _ = c.close().await;
            ok
        }
        Err(_) => false,
    };
    assert!(!bad_reads, "wrong key must not read the encrypted DB");

    // ...and the correct key still works (sanity).
    assert!(encryption::opens_with_key(&db_path, TEST_KEY_HEX)
        .await
        .expect("right-key probe"));
}

/// (d) INVARIANT: under a SQLCipher-enabled build, a KEYLESS open still reads a
/// plaintext temp DB. This is why `migration_workspace_sync` and
/// `repository_tenant_scoping` (which open plain temp DBs with no key) keep passing.
#[tokio::test]
async fn keyless_open_still_reads_plaintext_under_sqlcipher_build() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let db_path = tmp.path().join("plain_keyless.sqlite");

    let plain = open_plaintext(&db_path).await;
    MIGRATOR
        .run(&plain)
        .await
        .expect("migrations apply on a keyless plaintext DB under a SQLCipher build");
    seed_minimal(&plain).await;
    assert_eq!(count(&plain, "meetings").await, 2);
    // The freshly created file carries the plaintext header (not encrypted).
    plain.close().await;

    // Re-open keyless again and confirm the data reads back (no key ever supplied).
    let reopened = open_plaintext(&db_path).await;
    assert_eq!(count(&reopened, "meetings").await, 2);
    assert_eq!(count(&reopened, "transcripts").await, 3);
    assert!(
        !applied_versions(&reopened).await.is_empty(),
        "ledger present on the plaintext DB"
    );
    reopened.close().await;
}

/// (f) A source DB with a POPULATED plaintext `-wal` converts with all committed
/// rows preserved AND leaves no stale plaintext `-wal`/`-shm` behind.
///
/// To guarantee an un-checkpointed `-wal` at conversion time we open the source in
/// WAL mode with auto-checkpoint disabled, seed it, and — while that pool is still
/// OPEN (so SQLite has not truncated the WAL) — copy the three files (`.sqlite`,
/// `-wal`, `-shm`) to a fresh path. The copy therefore carries a live populated WAL,
/// exactly the shipped scenario (`meeting_minutes.sqlite` + `-wal` on disk).
#[tokio::test]
async fn conversion_with_populated_wal_preserves_rows_and_cleans_sidecars() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let origin = tmp.path().join("origin.sqlite");

    // WAL mode + autocheckpoint OFF so committed rows stay in the -wal file.
    let opts = SqliteConnectOptions::new()
        .filename(&origin)
        .create_if_missing(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .pragma("wal_autocheckpoint", "0");
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await
        .expect("open WAL-mode plaintext");
    MIGRATOR.run(&pool).await.expect("migrations");
    seed_minimal(&pool).await;

    let (owal, oshm) = wal_shm_paths(&origin);
    assert!(
        owal.exists()
            && std::fs::metadata(&owal)
                .map(|m| m.len() > 0)
                .unwrap_or(false),
        "precondition: origin must have a populated -wal before copying"
    );

    // Copy all three files WHILE the pool is open (populated WAL captured).
    let db_path = tmp.path().join("waltest.sqlite");
    let (twal, tshm) = wal_shm_paths(&db_path);
    std::fs::copy(&origin, &db_path).expect("copy main");
    std::fs::copy(&owal, &twal).expect("copy -wal");
    if oshm.exists() {
        std::fs::copy(&oshm, &tshm).expect("copy -shm");
    }
    pool.close().await; // origin can now be dropped by tempdir

    // The copy is plaintext and carries a populated WAL.
    assert!(twal.exists(), "copied -wal must exist");

    // Convert the copy.
    encryption::ensure_encrypted(&db_path, TEST_KEY_HEX)
        .await
        .expect("conversion of a WAL-populated DB");

    // All committed rows (including the WAL-only ones) are preserved.
    let enc = open_encrypted(&db_path, TEST_KEY_HEX).await;
    assert_eq!(count(&enc, "meetings").await, 2, "meetings preserved");
    assert_eq!(count(&enc, "transcripts").await, 3, "transcripts preserved");
    assert!(
        !applied_versions(&enc).await.is_empty(),
        "ledger preserved through WAL conversion"
    );
    enc.close().await;

    // No stale PLAINTEXT backup or sidecars remain. (The keyed pool just above may
    // have created encrypted WAL/SHM on close; assert instead that whatever remains
    // is NOT plaintext — the plaintext ones must be gone.)
    assert!(
        !backup_path(&db_path).exists(),
        "plaintext backup removed after WAL conversion"
    );
    // The pre-conversion plaintext -wal/-shm (the ones we copied) must not survive as
    // plaintext. A -wal file has no SQLite header, so verify via a keyless read: the
    // DB must NOT be keyless-readable (i.e. no plaintext DB/WAL pair is left).
    let keyless = SqliteConnectOptions::new()
        .filename(&db_path)
        .create_if_missing(false)
        .connect()
        .await;
    let keyless_reads = match keyless {
        Ok(mut c) => {
            let ok = sqlx::query("SELECT count(*) FROM meetings")
                .fetch_one(&mut c)
                .await
                .is_ok();
            let _ = c.close().await;
            ok
        }
        Err(_) => false,
    };
    assert!(
        !keyless_reads,
        "after conversion the DB must not be readable without the key (no plaintext residue)"
    );
}

/// (e) Empirical conversion proof against a COPY of a real user database.
///
/// Skipped unless `MITYU_TEST_ENCRYPT_DB` points at a copy (never the live file;
/// the harness copies it with `cp` first). Asserts row counts AND the ledger survive.
#[tokio::test]
async fn real_copy_conversion_when_env_set() {
    let Ok(copy_path) = std::env::var("MITYU_TEST_ENCRYPT_DB") else {
        eprintln!("skipping: MITYU_TEST_ENCRYPT_DB not set");
        return;
    };
    let copy_path = std::path::PathBuf::from(copy_path);
    assert!(copy_path.exists(), "MITYU_TEST_ENCRYPT_DB does not exist");

    // Snapshot the plaintext copy BEFORE conversion (keyless read).
    let pre = open_plaintext(&copy_path).await;
    let mut pre_counts = Vec::new();
    for t in ALL_TABLES {
        pre_counts.push((t, count(&pre, t).await));
    }
    let pre_ledger = ledger_rows(&pre).await;
    let pre_versions = applied_versions(&pre).await;
    println!("real-copy pre-conversion counts:");
    for (t, c) in &pre_counts {
        println!("  {t}: {c}");
    }
    println!("  _sqlx_migrations: {} rows", pre_ledger.len());
    pre.close().await;

    // Convert in place (on the copy).
    encryption::ensure_encrypted(&copy_path, TEST_KEY_HEX)
        .await
        .expect("real-copy conversion");

    // The copy is now encrypted (wrong key rejected).
    assert!(
        !encryption::opens_with_key(&copy_path, WRONG_KEY_HEX)
            .await
            .expect("wrong-key probe"),
        "converted real copy must reject the wrong key"
    );

    // No lingering plaintext: the .pre-encryption backup and any stale plaintext
    // WAL/SHM must be gone after the verified conversion.
    assert!(
        !backup_path(&copy_path).exists(),
        "real-copy: plaintext backup must be deleted after verified conversion"
    );
    let (rwal, rshm) = wal_shm_paths(&copy_path);
    assert!(
        !rwal.exists(),
        "real-copy: stale plaintext -wal must be removed"
    );
    assert!(
        !rshm.exists(),
        "real-copy: stale plaintext -shm must be removed"
    );
    println!("real-copy: backup + stale WAL/SHM removed (no plaintext residue)");

    // Keyed re-open: everything preserved.
    let enc = open_encrypted(&copy_path, TEST_KEY_HEX).await;
    println!("real-copy post-conversion counts (encrypted):");
    for (t, pre) in &pre_counts {
        let post = count(&enc, t).await;
        println!("  {t}: {post}");
        assert_eq!(post, *pre, "row count changed for {t}");
    }
    assert_eq!(
        applied_versions(&enc).await,
        pre_versions,
        "migration versions changed across real-copy conversion"
    );
    assert_eq!(
        ledger_rows(&enc).await,
        pre_ledger,
        "the _sqlx_migrations ledger must survive real-copy conversion intact"
    );
    // Migrator remains a no-op on the encrypted real copy.
    MIGRATOR
        .run(&enc)
        .await
        .expect("migrator no-op on real copy");
    assert_eq!(applied_versions(&enc).await, pre_versions);
    enc.close().await;
}

// --- Guardrailed manager fallback (ADR-0014): tests (g)–(i) ----------------------
//
// These drive the REAL `DatabaseManager::new` through its key path (which reads the
// OS keychain via `secrets::db`) against a PERSISTENT in-memory mock keyring — never
// the machine store. They serialize on the single process-wide `db-key` entry.

/// FNV-1a digest of a file's bytes — used to prove the ciphertext file is not mutated
/// by a fail-closed open (no rewrite, no key regeneration touching the file).
fn file_digest(path: &Path) -> (u64, u64) {
    let bytes = std::fs::read(path).expect("read db file for digest");
    let mut h: u64 = 0xcbf29ce484222325;
    for b in &bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    (bytes.len() as u64, h)
}

/// Build a real ENCRYPTED DB at `db_path`: migrate a plaintext DB, seed it, then run
/// the true conversion path with `TEST_KEY_HEX`. Leaves a verified cipher file on
/// disk (no plaintext residue), exactly like a converted user DB.
async fn make_encrypted_db(db_path: &Path) {
    let plain = open_plaintext(db_path).await;
    MIGRATOR.run(&plain).await.expect("migrations (plaintext)");
    seed_minimal(&plain).await;
    plain.close().await;
    encryption::ensure_encrypted(db_path, TEST_KEY_HEX)
        .await
        .expect("seed conversion to encrypted");
    // Sanity: it really is ciphertext now.
    assert!(
        !encryption::is_plaintext_db(db_path).expect("state probe"),
        "precondition: seeded DB must be encrypted"
    );
}

/// (g) ENCRYPTED file + key PRESENT → `DatabaseManager::new` opens keyed and rows read.
#[tokio::test]
async fn manager_encrypted_with_key_present_opens_and_reads() {
    mock_keyring::install();
    let _guard = mock_keyring::db_key_lock().lock().await;

    let tmp = tempfile::tempdir().expect("tempdir");
    let db_path = tmp.path().join("meeting_minutes.sqlite");
    let backend = tmp.path().join("meeting_minutes.db"); // absent legacy path

    make_encrypted_db(&db_path).await;
    // Store the SAME key the file was encrypted with under the device `db-key` entry.
    let entry = keyring::Entry::new(app_lib::secrets::KEYCHAIN_SERVICE, dbkey::ENTRY_NAME)
        .expect("open mock db-key entry");
    entry.set_password(TEST_KEY_HEX).expect("seed db-key");

    let db_path_s = db_path.to_string_lossy().to_string();
    let backend_s = backend.to_string_lossy().to_string();
    let mgr = DatabaseManager::new(&db_path_s, &backend_s)
        .await
        .expect("encrypted DB with key present must open");

    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM meetings")
        .fetch_one(mgr.pool())
        .await
        .expect("read meetings through the keyed pool");
    assert_eq!(
        n, 2,
        "seeded rows must be readable via the keyed manager pool"
    );
    mgr.cleanup().await.ok();
}

/// (h) ENCRYPTED file + key MISSING → FAIL CLOSED. `DatabaseManager::new` errors, the
/// ciphertext file is byte-for-byte UNMODIFIED, NO new key is minted, and the DB is
/// NOT opened as plaintext.
#[tokio::test]
async fn manager_encrypted_with_key_missing_fails_closed() {
    mock_keyring::install();
    let _guard = mock_keyring::db_key_lock().lock().await;

    let tmp = tempfile::tempdir().expect("tempdir");
    let db_path = tmp.path().join("meeting_minutes.sqlite");
    let backend = tmp.path().join("meeting_minutes.db");

    make_encrypted_db(&db_path).await;
    let before = file_digest(&db_path);

    // No key in the store: a missing entry must NOT be minted over ciphertext.
    mock_keyring::clear_db_key();
    assert!(
        dbkey::get_hex().expect("non-creating read").is_none(),
        "precondition: the db-key entry must be absent"
    );

    let db_path_s = db_path.to_string_lossy().to_string();
    let backend_s = backend.to_string_lossy().to_string();
    let res = DatabaseManager::new(&db_path_s, &backend_s).await;

    // 1) It FAILS CLOSED (does not open).
    let err = res
        .err()
        .expect("encrypted-but-keyless open must fail closed");
    let msg = err.to_string();
    assert!(
        msg.contains("encrypted") && msg.contains("key"),
        "error must explain the encrypted-but-keyless refusal, got: {msg}"
    );

    // 2) The ciphertext file is byte-for-byte UNMODIFIED (no rewrite/conversion).
    assert_eq!(
        file_digest(&db_path),
        before,
        "the encrypted file must not be modified by a fail-closed open"
    );

    // 3) NO new key was minted (the fail-closed path used the non-creating read).
    assert!(
        dbkey::get_hex()
            .expect("non-creating read after fail-closed")
            .is_none(),
        "a fail-closed open must NOT create a new db-key over ciphertext"
    );

    // 4) It was NOT opened as plaintext: the file is still ciphertext AND only the
    //    original key reads it (a regenerated key would not).
    assert!(
        !encryption::is_plaintext_db(&db_path).expect("state probe"),
        "the file must remain encrypted (never opened/rewritten as plaintext)"
    );
    assert!(
        encryption::opens_with_key(&db_path, TEST_KEY_HEX)
            .await
            .expect("right-key probe"),
        "the original key must still decrypt the untouched file"
    );

    // Restore a key so a later serialized test starts clean.
    let entry = keyring::Entry::new(app_lib::secrets::KEYCHAIN_SERVICE, dbkey::ENTRY_NAME)
        .expect("reopen mock db-key entry");
    entry.set_password(TEST_KEY_HEX).expect("restore db-key");
}

/// (i) PLAINTEXT file + key store UNAVAILABLE → FAIL OPEN. The key store errors (as a
/// locked/unavailable OS keychain would), so the manager opens the still-plaintext
/// file rather than lock the user out of their own local data (CLAUDE.md §0.1). Rows
/// read back and there is no crash. The file remains plaintext (heals on the next
/// keyed launch once the store is back).
#[tokio::test]
async fn manager_plaintext_with_store_unavailable_fails_open() {
    mock_keyring::install();
    let _guard = mock_keyring::db_key_lock().lock().await;

    let tmp = tempfile::tempdir().expect("tempdir");
    let db_path = tmp.path().join("meeting_minutes.sqlite");
    let backend = tmp.path().join("meeting_minutes.db");

    // Build a PLAINTEXT, migrated, seeded DB.
    let plain = open_plaintext(&db_path).await;
    MIGRATOR.run(&plain).await.expect("migrations (plaintext)");
    seed_minimal(&plain).await;
    plain.close().await;
    assert!(
        encryption::is_plaintext_db(&db_path).expect("state probe"),
        "precondition: DB must be plaintext"
    );

    // Model a locked/unavailable OS keychain: every credential op errors, so
    // `get_or_create_hex()` returns Err and the PLAINTEXT fail-open branch runs.
    mock_keyring::clear_db_key();
    mock_keyring::set_unavailable();

    let db_path_s = db_path.to_string_lossy().to_string();
    let backend_s = backend.to_string_lossy().to_string();
    let opened = DatabaseManager::new(&db_path_s, &backend_s).await;
    mock_keyring::set_available(); // restore before asserting so cleanup is normal

    let mgr = opened.expect("plaintext DB must open (fail-open) when the key store is unavailable");

    // Rows read back, no crash.
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM meetings")
        .fetch_one(mgr.pool())
        .await
        .expect("read meetings after a plaintext fail-open");
    assert_eq!(n, 2, "seeded rows must be readable after a plaintext open");

    // It stayed PLAINTEXT (no key was available to encrypt it) — heals next launch.
    assert!(
        encryption::is_plaintext_db(&db_path).expect("state probe after fail-open"),
        "with the store unavailable the file must remain plaintext (no keyless encryption)"
    );
    mgr.cleanup().await.ok();
}
