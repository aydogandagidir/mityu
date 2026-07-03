use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode};
use sqlx::{Result, Sqlite, SqlitePool, Transaction};
use std::fs;
use std::path::Path;
use tauri::Manager;

#[derive(Clone)]
pub struct DatabaseManager {
    pool: SqlitePool,
    /// Records which branch of the ADR-0014 at-rest encryption decision actually ran
    /// for this open, so the UI can warn on a plaintext fallback (follow-up to
    /// ADR-0014). `true` = the pool was opened with the SQLCipher key (data encrypted
    /// at rest); `false` = a fail-open plaintext branch was taken because the keychain
    /// key was unavailable on a plaintext/fresh DB. This does NOT influence the open
    /// logic; it only mirrors the outcome recorded in `DatabaseManager::new`.
    at_rest_encrypted: bool,
}

impl DatabaseManager {
    pub async fn new(tauri_db_path: &str, backend_db_path: &str) -> Result<Self> {
        if let Some(parent_dir) = Path::new(tauri_db_path).parent() {
            if !parent_dir.exists() {
                fs::create_dir_all(parent_dir).map_err(sqlx::Error::Io)?;
            }
        }

        // Legacy import stays plaintext on disk here; the SQLCipher conversion
        // below encrypts whatever plaintext file we end up with (copied legacy or a
        // pre-existing plaintext .sqlite) before the keyed pool opens.
        if !Path::new(tauri_db_path).exists() && Path::new(backend_db_path).exists() {
            log::info!(
                "Copying database from {} to {}",
                backend_db_path,
                tauri_db_path
            );
            fs::copy(backend_db_path, tauri_db_path).map_err(sqlx::Error::Io)?;
        }

        // At-rest encryption (BACKLOG B3, docs/SECURITY_PRIVACY.md "Encryption",
        // ADR-0014) with a GUARDRAILED local-first fallback. The 256-bit DB key lives
        // in the OS keychain (`secrets::db`). The posture depends on the on-disk state
        // of the file, decided BEFORE the key is touched:
        //
        //   * ENCRYPTED file  → FAIL CLOSED. Read the key with the NON-creating
        //     `get_hex()` (a missing entry must NEVER mint a fresh key over live
        //     ciphertext — that would strand the data behind a key that cannot decrypt
        //     it). If the key is present we open keyed as normal; if it is missing or
        //     the store errored we return an error and DO NOT open — never a keyless
        //     open of ciphertext, never a regenerated key. Local-first is preserved by
        //     "retry next launch / restore the key", not by exposing plaintext.
        //
        //   * PLAINTEXT or ABSENT file → fail-open is permitted (CLAUDE.md §0.1: the
        //     capture→transcript→summary→store path must stay functional). Use
        //     `get_or_create_hex()` (mint-on-first-run is correct here), then convert
        //     via `ensure_encrypted`. If the key store is unavailable, or the one-time
        //     conversion fails (e.g. a SQLCipher-less runtime where `sqlcipher_export`
        //     is missing), we log LOUDLY and open the file UNENCRYPTED, retried next
        //     launch. This is safe because the file was plaintext anyway — no
        //     confidentiality is lost relative to the pre-B3 state, and no ciphertext
        //     is ever opened keyless.
        //
        // Net accepted tradeoff: brand-new plaintext data is possible ONLY on a
        // machine that was never successfully encrypted (first run with a broken
        // keychain); it heals on the next keyed launch. An in-app downgrade WARNING is
        // tracked as follow-up (ADR-0014). `Zeroizing<String>` scrubs the key from
        // memory on drop (below, right after the pool opens).
        let db_path = Path::new(tauri_db_path);
        let is_plaintext = crate::database::encryption::is_plaintext_db(db_path)
            .map_err(|e| sqlx::Error::Io(std::io::Error::other(format!("{e:#}"))))?;
        let on_disk_encrypted = db_path.exists() && !is_plaintext;

        // `key_hex` holds the key material only if a keyed open is actually happening;
        // `None` means "open plaintext" (fail-open branch only, never for ciphertext).
        let key_hex: Option<zeroize::Zeroizing<String>> = if on_disk_encrypted {
            // FAIL CLOSED path: non-creating read; a missing/errored key must abort.
            match crate::secrets::db::get_hex() {
                Ok(Some(key)) => {
                    // `ensure_encrypted` is a no-op on an already-encrypted file; call
                    // it for symmetry, then open keyed.
                    crate::database::encryption::ensure_encrypted(db_path, &key)
                        .await
                        .map_err(|e| {
                            sqlx::Error::Configuration(
                                format!("database encryption conversion failed: {e:#}").into(),
                            )
                        })?;
                    Some(key)
                }
                Ok(None) | Err(_) => {
                    // Encrypted on disk but no usable key: DO NOT open, DO NOT
                    // regenerate. This is NOT corruption — return a distinct
                    // Configuration error so the recovery path in
                    // `new_from_app_handle` does not misclassify it and delete the WAL.
                    return Err(sqlx::Error::Configuration(
                        "local database is encrypted but its key is unavailable (the OS keychain \
                         is locked, or the 'db-key' entry is missing); refusing to open — retry \
                         once the keychain is unlocked, or restore the key"
                            .into(),
                    ));
                }
            }
        } else {
            // FAIL OPEN path (plaintext or absent file only). Mint-on-first-run is
            // correct here; a store/conversion failure degrades to a plaintext open.
            match crate::secrets::db::get_or_create_hex() {
                Ok(key) => match crate::database::encryption::ensure_encrypted(db_path, &key).await
                {
                    Ok(()) => Some(key),
                    Err(e) => {
                        // The file is still plaintext (conversion failed); open it
                        // UNENCRYPTED for now. Do NOT claim it is encrypted/safe.
                        log::error!(
                            "At-rest DB encryption conversion FAILED ({e:#}); opening the local \
                             database UNENCRYPTED (plaintext at rest) for now — will retry the \
                             conversion on the next launch. No data is lost; the file was already \
                             plaintext."
                        );
                        None
                    }
                },
                Err(e) => {
                    // Keychain unavailable on a fresh/plaintext file: open plaintext,
                    // retry next launch. Accurate wording — this is UNENCRYPTED.
                    log::error!(
                        "DB encryption key unavailable ({e:#}); opening the local database \
                         UNENCRYPTED (plaintext at rest) for now — will retry acquiring the key \
                         and encrypting on the next launch."
                    );
                    None
                }
            }
        };

        // Keyed pool ONLY when a key is in hand (encrypted file with a good key, or a
        // freshly converted/created file); otherwise a plaintext open on the fail-open
        // branch so a first-run encryption hiccup never locks the user out of their
        // own local data. A keyless open of an ENCRYPTED file cannot happen here — that
        // branch returned above. `PRAGMA key` is a reserved slot sqlx executes FIRST
        // when present. WAL mode is preserved explicitly; a fresh file is created
        // already-encrypted (or plaintext) via create_if_missing.
        let mut options = SqliteConnectOptions::new()
            .filename(tauri_db_path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .foreign_keys(true);
        if let Some(k) = &key_hex {
            options = options.pragma("key", crate::secrets::db::pragma_key_value(k));
        }
        // Record the encryption outcome BEFORE `key_hex` is dropped. A key in hand
        // means the pool is opening keyed (encrypted file + good key, or a
        // freshly created/converted file); `None` is only ever the fail-open plaintext
        // branch (both keyless-open paths above logged loudly). This is purely a
        // read-out of the decision already made — it does not alter the open logic.
        let at_rest_encrypted = key_hex.is_some();

        let pool = SqlitePool::connect_with(options).await?;
        // Key material is no longer needed once the pool holds an open connection;
        // drop it now to zeroize it promptly (defense in depth).
        drop(key_hex);

        sqlx::migrate!("./migrations").run(&pool).await?;

        // One-time, idempotent migration of any legacy plaintext BYOK API keys out
        // of the settings/transcript_settings columns and into the OS credential
        // store (CLAUDE.md §0.7/§3, docs/SECURITY_PRIVACY.md "Secrets"). Runs on
        // every init path (fresh, legacy-import, recovery) and fully offline.
        // NON-FATAL: a locked/unavailable keychain must never block opening the DB
        // or offline operation (local-first invariant) — unmigrated columns are
        // retried lazily on next read.
        match crate::database::repositories::setting::SettingsRepository::migrate_plaintext_keys_to_keychain(&pool)
            .await
        {
            Ok(0) => {}
            Ok(n) => log::info!(
                "Migrated {} legacy plaintext API key(s) into the OS credential store",
                n
            ),
            Err(e) => log::warn!(
                "Non-fatal: could not migrate plaintext API keys into the OS credential store ({}); will retry lazily on next read",
                e
            ),
        }

        Ok(DatabaseManager {
            pool,
            at_rest_encrypted,
        })
    }

    // NOTE: So for the first time users they needs to start the application
    // after they can just delete the existing .sqlite file and then copy the existing .db file to
    // the current app dir, So the system detects legacy db and copy it and starts with that data
    // (Newly created .sqlite with the copied content from .db)
    pub async fn new_from_app_handle(app_handle: &tauri::AppHandle) -> Result<Self> {
        // Resolve the app's data directory
        let app_data_dir = app_handle
            .path()
            .app_data_dir()
            .expect("failed to get app data dir");
        if !app_data_dir.exists() {
            fs::create_dir_all(&app_data_dir).map_err(|e| sqlx::Error::Io(e))?;
        }

        // Define database paths
        let tauri_db_path = app_data_dir
            .join("meeting_minutes.sqlite")
            .to_string_lossy()
            .to_string();
        // Legacy backend DB path (for auto-migration if exists)
        let backend_db_path = app_data_dir
            .join("meeting_minutes.db")
            .to_string_lossy()
            .to_string();

        // WAL file paths for defensive cleanup
        let wal_path = app_data_dir.join("meeting_minutes.sqlite-wal");
        let shm_path = app_data_dir.join("meeting_minutes.sqlite-shm");

        log::info!("Tauri DB path: {}", tauri_db_path);
        log::info!("Legacy backend DB path: {}", backend_db_path);

        // Try to open database with defensive WAL handling
        match Self::new(&tauri_db_path, &backend_db_path).await {
            Ok(db_manager) => {
                log::info!("Database opened successfully");
                Ok(db_manager)
            }
            Err(e) => {
                // Check if error is due to corrupted WAL file
                let error_msg = e.to_string();
                if error_msg.contains("malformed") || error_msg.contains("corrupt") {
                    log::warn!("Database appears corrupted, likely due to orphaned WAL file. Attempting recovery...");
                    log::warn!("Error details: {}", error_msg);

                    // Delete potentially corrupted WAL/SHM files
                    if wal_path.exists() {
                        match fs::remove_file(&wal_path) {
                            Ok(_) => log::info!("Removed orphaned WAL file: {:?}", wal_path),
                            Err(e) => log::warn!("Failed to remove WAL file: {}", e),
                        }
                    }
                    if shm_path.exists() {
                        match fs::remove_file(&shm_path) {
                            Ok(_) => log::info!("Removed orphaned SHM file: {:?}", shm_path),
                            Err(e) => log::warn!("Failed to remove SHM file: {}", e),
                        }
                    }

                    // Retry connection without WAL files
                    log::info!("Retrying database connection after WAL cleanup...");
                    match Self::new(&tauri_db_path, &backend_db_path).await {
                        Ok(db_manager) => {
                            log::info!("Database opened successfully after WAL recovery");
                            Ok(db_manager)
                        }
                        Err(retry_err) => {
                            log::error!(
                                "Database connection failed even after WAL cleanup: {}",
                                retry_err
                            );
                            Err(retry_err)
                        }
                    }
                } else {
                    // Not a WAL-related error, propagate original error
                    log::error!("Database connection failed: {}", error_msg);
                    Err(e)
                }
            }
        }
    }

    /// Check if this is the first launch (sqlite database doesn't exist yet)
    pub async fn is_first_launch(app_handle: &tauri::AppHandle) -> Result<bool> {
        let app_data_dir = app_handle
            .path()
            .app_data_dir()
            .expect("failed to get app data dir");

        let tauri_db_path = app_data_dir.join("meeting_minutes.sqlite");

        Ok(!tauri_db_path.exists())
    }

    /// Import a legacy database from the specified path and initialize
    pub async fn import_legacy_database(
        app_handle: &tauri::AppHandle,
        legacy_db_path: &str,
    ) -> Result<Self> {
        let app_data_dir = app_handle
            .path()
            .app_data_dir()
            .expect("failed to get app data dir");

        if !app_data_dir.exists() {
            fs::create_dir_all(&app_data_dir).map_err(|e| sqlx::Error::Io(e))?;
        }

        // Copy legacy database to app data directory as meeting_minutes.db
        let target_legacy_path = app_data_dir.join("meeting_minutes.db");
        log::info!(
            "Copying legacy database from {} to {}",
            legacy_db_path,
            target_legacy_path.display()
        );

        fs::copy(legacy_db_path, &target_legacy_path).map_err(|e| sqlx::Error::Io(e))?;

        // Now use the standard initialization which will detect and migrate the legacy db
        Self::new_from_app_handle(app_handle).await
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Whether the local database was opened ENCRYPTED at rest (SQLCipher key applied)
    /// for this session. `false` means the ADR-0014 fail-open plaintext branch was
    /// taken (keychain key unavailable on a plaintext/fresh DB), so the file is
    /// plaintext at rest and the UI should warn. Reflects the decision made in `new`;
    /// it does not re-check the file on disk.
    pub fn is_at_rest_encrypted(&self) -> bool {
        self.at_rest_encrypted
    }

    pub async fn with_transaction<T, F, Fut>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&mut Transaction<'_, Sqlite>) -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        let mut tx = self.pool.begin().await?;
        let result = f(&mut tx).await;

        match result {
            Ok(val) => {
                tx.commit().await?;
                Ok(val)
            }
            Err(err) => {
                tx.rollback().await?;
                Err(err)
            }
        }
    }

    /// Cleanup database connection and checkpoint WAL
    /// This should be called on application shutdown to ensure:
    /// - All WAL changes are written to the main database file
    /// - The .wal and .shm files are deleted
    /// - Connection pool is gracefully closed
    pub async fn cleanup(&self) -> Result<()> {
        log::info!("Starting database cleanup...");

        // Force checkpoint of WAL to main database file and remove WAL file
        // TRUNCATE mode: checkpoints all pages AND deletes the WAL file
        match sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
            .execute(&self.pool)
            .await
        {
            Ok(_) => log::info!("WAL checkpoint completed successfully"),
            Err(e) => log::warn!("WAL checkpoint failed (non-fatal): {}", e),
        }

        // Close the connection pool gracefully
        self.pool.close().await;
        log::info!("Database connection pool closed");

        Ok(())
    }
}
