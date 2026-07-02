use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode};
use sqlx::{Result, Sqlite, SqlitePool, Transaction};
use std::fs;
use std::path::Path;
use tauri::Manager;

#[derive(Clone)]
pub struct DatabaseManager {
    pool: SqlitePool,
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
        // ADR-0014). The 256-bit DB key lives in the OS keychain; fetch/create it
        // and FAIL CLOSED — a locked/unavailable store must never open the DB
        // unencrypted (map the anyhow error onto sqlx::Error to keep the signature).
        // `Zeroizing<String>` scrubs the key from memory once it drops (below, right
        // after the pool is opened) so it does not linger in freed heap.
        let key_hex = crate::secrets::db::get_or_create_hex().map_err(|e| {
            sqlx::Error::Configuration(format!("database encryption key unavailable: {e:#}").into())
        })?;

        // One-time plaintext -> SQLCipher conversion, BEFORE the keyed open and
        // BEFORE migrations. No-op when the file is missing (fresh install) or
        // already encrypted. Preserves the _sqlx_migrations ledger and every row.
        crate::database::encryption::ensure_encrypted(Path::new(tauri_db_path), &key_hex)
            .await
            .map_err(|e| {
                sqlx::Error::Configuration(
                    format!("database encryption conversion failed: {e:#}").into(),
                )
            })?;

        // Keyed pool. `PRAGMA key` is a reserved slot that sqlx executes FIRST
        // (before journal_mode/foreign_keys/any statement), so the cipher is active
        // for the whole connection. WAL mode is preserved explicitly so the
        // checkpoint/recovery paths below keep working; a fresh file is created
        // already-encrypted via create_if_missing.
        let options = SqliteConnectOptions::new()
            .filename(tauri_db_path)
            .create_if_missing(true)
            .pragma("key", crate::secrets::db::pragma_key_value(&key_hex))
            .journal_mode(SqliteJournalMode::Wal)
            .foreign_keys(true);
        let pool = SqlitePool::connect_with(options).await?;
        // Key material is no longer needed once the pool holds an open keyed
        // connection; drop it now to zeroize it promptly (defense in depth).
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

        Ok(DatabaseManager { pool })
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
