//! One-time, at-startup conversion of a **plaintext** SQLite database to a
//! **SQLCipher-encrypted** one (BACKLOG B3, `docs/SECURITY_PRIVACY.md`
//! "Encryption", ADR-0014).
//!
//! ## Where this runs
//! [`ensure_encrypted`] is called by [`crate::database::manager::DatabaseManager`]
//! **before** the keyed pool is opened and **before** `sqlx::migrate!` runs. After
//! it returns `Ok(())` the file at `db_path` is guaranteed to be either:
//!   * a SQLCipher database that opens with the device key, or
//!   * absent (fresh install — the keyed `create_if_missing` open then makes a new
//!     encrypted file), or
//!   * a legacy plaintext temp/test DB opened with **no** key (a keyless SQLCipher
//!     build still reads plaintext, so existing keyless tests keep working).
//!
//! ## Conversion algorithm (SQLCipher `sqlcipher_export`)
//! When a readable *plaintext* main DB is detected we:
//!   1. if a plaintext `<db>-wal` exists, open the plaintext DB and
//!      `PRAGMA wal_checkpoint(TRUNCATE)` so every committed row folds into the main
//!      file (avoids missing WAL-only rows, and a stale plaintext `-wal` colliding
//!      with the new encrypted DB's WAL under the same name),
//!   2. open the **encrypted destination** `<db>.encrypting` as the *main*
//!      connection (`create_if_missing` + the `key` pragma) — on this build
//!      `ATTACH`ing a brand-new file cannot create it (`SQLITE_CANTOPEN`), so the
//!      encrypted side must be the main connection, not the attached one,
//!      `ATTACH DATABASE '<db>' AS plaintext KEY ''` — the existing plaintext source,
//!      empty key,
//!   3. `SELECT sqlcipher_export('main', 'plaintext')` — copies **all** schema +
//!      rows verbatim from the plaintext source into the encrypted main DB,
//!      including the `_sqlx_migrations` ledger (the migration-idempotency proof
//!      depends on it surviving),
//!   4. `DETACH DATABASE plaintext`, close the connection,
//!   5. **atomically** move the original aside to `<db>.pre-encryption` (a temporary
//!      safety backup) and move the new cipher file into `<db>`,
//!   6. **verify** the promoted cipher file opens with the key (a cheap
//!      `SELECT count(*) FROM sqlite_master`); only then **delete** the
//!      `<db>.pre-encryption` backup (best-effort scrub-then-unlink) and any stale
//!      plaintext `<db>-wal` / `<db>-shm` siblings. If verification fails, the backup
//!      is restored and we error (fail closed, recoverable).
//!
//! The keyed open + migrations then proceed on the encrypted file. Because sync is
//! not involved and no rows are rewritten, every `created_at`/`updated_at`/`rev`
//! and every `workspace_id` is preserved byte-for-byte.
//!
//! ## Local-first / fail-closed
//! The key comes from the OS keychain ([`crate::secrets::db`]). If the store is
//! unavailable we return an error and the DB is **not** opened (no unencrypted
//! fallback). Conversion never deletes the original until the encrypted copy is in
//! place **and proven readable with the key**; on any mid-flight failure the
//! plaintext file is left intact (or restored from the backup) and the stray partial
//! cipher file is cleaned up, so a retry on next launch is safe.
//!
//! ## No lingering plaintext (security review, ADR-0014)
//! Once the encrypted DB is verified, **no** full-database plaintext remains on
//! disk: the `<db>.pre-encryption` backup is deleted and the stale plaintext WAL/SHM
//! are removed. The consequence is that the key in the OS keychain is now the *only*
//! way to read the data — **losing the `db-key` entry makes the DB unrecoverable**
//! (the intended at-rest posture; a key export/rekey affordance is future work).

use anyhow::{anyhow, Context, Result};
use sqlx::sqlite::{SqliteConnectOptions, SqliteConnection, SqliteJournalMode};
use sqlx::{ConnectOptions, Connection};
use std::path::{Path, PathBuf};

/// SQLite/SQLCipher file magic: the first 16 bytes of a *plaintext* SQLite file are
/// the ASCII header `"SQLite format 3\0"`. A SQLCipher-encrypted file starts with
/// the (encrypted) salt instead, so this header is absent — which is exactly how we
/// tell the two apart without a key.
const SQLITE_HEADER: &[u8; 16] = b"SQLite format 3\x00";

/// Suffix for the temporary pre-conversion plaintext backup (deleted after the
/// encrypted DB is verified readable).
const BACKUP_SUFFIX: &str = "pre-encryption";
/// Suffix for the in-progress encrypted file (promoted to the real path on success).
const ENCRYPTING_SUFFIX: &str = "encrypting";

/// Whether the file at `path` is a **plaintext** SQLite database (carries the
/// `"SQLite format 3\0"` header).
///
/// This is the single, shared on-disk state probe used both by [`ensure_encrypted`]
/// (to decide whether a one-time conversion is needed) and by
/// [`crate::database::manager::DatabaseManager`] (to decide fail-open vs fail-closed
/// on the key path — ADR-0014). It returns:
///   * `Ok(false)` for a **missing**, empty, short (< 16 byte), or **encrypted** file
///     (a SQLCipher file starts with an encrypted salt, not the header);
///   * `Ok(true)` only for a readable plaintext SQLite DB.
///
/// IO errors other than not-found are surfaced. It never opens a keyed/SQLCipher
/// connection — it only inspects the first 16 bytes — so it is safe to call before
/// any key is fetched. Callers distinguish `absent` from `encrypted` (both `false`
/// here) via [`Path::exists`].
///
/// `pub` (not `pub(crate)`) so the encryption integration tests can classify a temp
/// file's on-disk state with the exact same probe the manager uses, alongside the
/// already-public [`opens_with_key`] — rather than duplicate the header logic.
pub fn is_plaintext_db(path: &Path) -> Result<bool> {
    has_plaintext_header(path)
}

/// Does `path` begin with the plaintext `"SQLite format 3\0"` header?
///
/// `Ok(false)` for a missing/empty file or an encrypted file; `Ok(true)` only for a
/// readable plaintext SQLite DB. IO errors (other than not-found) are surfaced.
fn has_plaintext_header(path: &Path) -> Result<bool> {
    use std::io::Read;
    let mut file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(e) => {
            return Err(e)
                .with_context(|| format!("failed to open {} for header probe", path.display()))
        }
    };
    let mut header = [0u8; 16];
    match file.read_exact(&mut header) {
        Ok(()) => Ok(&header == SQLITE_HEADER),
        // A file shorter than 16 bytes cannot be a populated plaintext DB.
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => Ok(false),
        Err(e) => Err(e).with_context(|| format!("failed to read header of {}", path.display())),
    }
}

/// Append a `.suffix` sibling path (e.g. `foo.sqlite` → `foo.sqlite.pre-encryption`).
fn sibling(path: &Path, suffix: &str) -> PathBuf {
    let mut s = path.as_os_str().to_os_string();
    s.push(".");
    s.push(suffix);
    PathBuf::from(s)
}

/// The SQLite WAL and SHM sidecar paths for a DB (e.g. `foo.sqlite` →
/// `foo.sqlite-wal`, `foo.sqlite-shm`). Note the hyphen (SQLite's own convention),
/// unlike [`sibling`]'s dot.
fn wal_shm_paths(db_path: &Path) -> (PathBuf, PathBuf) {
    let mut wal = db_path.as_os_str().to_os_string();
    wal.push("-wal");
    let mut shm = db_path.as_os_str().to_os_string();
    shm.push("-shm");
    (PathBuf::from(wal), PathBuf::from(shm))
}

/// Fold any committed WAL rows of the *plaintext* DB back into the main file, then
/// close, so (a) no committed row is left only in the WAL and thus missed by the
/// export, and (b) no stale plaintext `-wal` survives to collide with the new
/// encrypted DB's WAL under the same name. Best-effort: a checkpoint failure is
/// logged and tolerated (the subsequent export still reads the WAL via the attached
/// connection); only a hard open failure is surfaced.
async fn checkpoint_plaintext_wal(db_path: &Path) -> Result<()> {
    let (wal, _shm) = wal_shm_paths(db_path);
    if !wal.exists() {
        return Ok(());
    }
    let mut conn: SqliteConnection = SqliteConnectOptions::new()
        .filename(db_path)
        .create_if_missing(false)
        .connect()
        .await
        .with_context(|| {
            format!(
                "failed to open plaintext database {} to checkpoint its WAL",
                db_path.display()
            )
        })?;
    match sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
        .execute(&mut conn)
        .await
    {
        Ok(_) => log::info!(
            "Checkpointed plaintext WAL into {} before encryption",
            db_path.display()
        ),
        Err(e) => log::warn!(
            "Non-fatal: could not checkpoint the plaintext WAL for {} ({}); the export \
             still reads committed WAL rows via the attached connection",
            db_path.display(),
            e
        ),
    }
    let _ = conn.close().await;
    Ok(())
}

/// Best-effort secure delete of a (plaintext) file: overwrite its bytes with zeros,
/// flush, then unlink. On any error we still attempt the plain unlink. Never fatal.
fn scrub_and_remove(path: &Path) {
    use std::io::Write as _;
    if let Ok(meta) = std::fs::metadata(path) {
        let len = meta.len();
        if let Ok(mut f) = std::fs::OpenOptions::new().write(true).open(path) {
            let zeros = vec![0u8; 64 * 1024];
            let mut remaining = len;
            while remaining > 0 {
                let n = remaining.min(zeros.len() as u64) as usize;
                if f.write_all(&zeros[..n]).is_err() {
                    break;
                }
                remaining -= n as u64;
            }
            let _ = f.flush();
            let _ = f.sync_all();
        }
    }
    let _ = std::fs::remove_file(path);
}

/// Ensure the database at `db_path` is SQLCipher-encrypted with `key_hex`, running a
/// one-time plaintext→encrypted conversion if (and only if) a plaintext DB is found.
///
/// Idempotent and safe to call on every startup:
///   * no file           → nothing to do (the keyed open creates a fresh cipher DB);
///   * already encrypted  → nothing to do;
///   * plaintext          → checkpoint its WAL, convert once, verify the cipher DB
///     opens with the key, then delete the temporary plaintext backup and stale WAL/SHM.
///
/// After success **no full-database plaintext remains on disk** and the OS-keychain
/// key is the only way to read the data (losing it ⇒ unrecoverable; see the module
/// docs and ADR-0014). `key_hex` must be 64 lowercase hex chars (see
/// [`crate::secrets::db`]).
pub async fn ensure_encrypted(db_path: &Path, key_hex: &str) -> Result<()> {
    if !has_plaintext_header(db_path)? {
        // Missing, empty, or already-encrypted: leave it to the keyed open.
        return Ok(());
    }

    log::info!(
        "Detected a plaintext database at {}; performing one-time SQLCipher encryption",
        db_path.display()
    );

    // Fold committed WAL rows into the plaintext main file first (avoids missing
    // WAL-only rows and a stale plaintext -wal colliding with the encrypted WAL).
    checkpoint_plaintext_wal(db_path).await?;

    let backup_path = sibling(db_path, BACKUP_SUFFIX);
    let encrypting_path = sibling(db_path, ENCRYPTING_SUFFIX);

    // A stale in-progress file from a previously interrupted run must not corrupt
    // this attempt.
    if encrypting_path.exists() {
        std::fs::remove_file(&encrypting_path).with_context(|| {
            format!(
                "failed to remove stale in-progress cipher file {}",
                encrypting_path.display()
            )
        })?;
    }

    // Perform the export into the fresh cipher file; on any failure clean up the
    // partial file and leave the plaintext original untouched for a safe retry.
    if let Err(e) = export_to_encrypted(db_path, &encrypting_path, key_hex).await {
        let _ = std::fs::remove_file(&encrypting_path);
        return Err(e);
    }

    // Atomic-ish swap: back up the plaintext original, then move the cipher file
    // into place. Order matters — the original is only ever removed by being
    // renamed to the backup, so the data is never without an on-disk copy.
    std::fs::rename(db_path, &backup_path).with_context(|| {
        format!(
            "failed to move the plaintext database {} aside to {}",
            db_path.display(),
            backup_path.display()
        )
    })?;
    if let Err(e) = std::fs::rename(&encrypting_path, db_path) {
        // Roll the original back so the app can still open its data next launch.
        let _ = std::fs::rename(&backup_path, db_path);
        let _ = std::fs::remove_file(&encrypting_path);
        return Err(e).with_context(|| {
            format!(
                "failed to promote the encrypted database into {}",
                db_path.display()
            )
        });
    }

    // VERIFY the promoted cipher file actually opens with the key BEFORE destroying
    // the plaintext backup. If it does not, restore the plaintext original from the
    // backup and fail closed (recoverable) — never leave the user without readable
    // data on a bad key/file.
    match opens_with_key(db_path, key_hex).await {
        Ok(true) => {}
        Ok(false) | Err(_) => {
            let _ = std::fs::remove_file(db_path);
            let _ = std::fs::rename(&backup_path, db_path);
            return Err(anyhow!(
                "the newly encrypted database at {} did not open with the key; restored the \
                 plaintext backup and aborted (fail closed)",
                db_path.display()
            ));
        }
    }

    // Verified readable: now there is a proven-good encrypted copy, so it is safe to
    // destroy the plaintext. Scrub+delete the backup and any stale plaintext WAL/SHM
    // siblings of the original — no full-database plaintext must survive on disk.
    scrub_and_remove(&backup_path);
    let (wal, shm) = wal_shm_paths(db_path);
    if wal.exists() {
        scrub_and_remove(&wal);
    }
    if shm.exists() {
        // SHM holds no row data (just WAL index), but remove the stale plaintext one
        // so it cannot shadow the encrypted DB's fresh SHM.
        let _ = std::fs::remove_file(&shm);
    }

    log::info!(
        "Database encrypted with SQLCipher and verified; plaintext backup and stale WAL/SHM \
         removed (the OS-keychain key is now required to read it)"
    );
    Ok(())
}

/// Copy the plaintext DB at `src` into a fresh SQLCipher DB at `dst` keyed with
/// `key_hex`, via `sqlcipher_export`, then detach. `dst` must not already exist.
///
/// Direction matters on this build: `ATTACH DATABASE '<new file>'` cannot *create*
/// a database file (it returns `SQLITE_CANTOPEN`), so we open the **encrypted
/// destination as the main connection** (which is opened with create permission and
/// the `key` pragma) and ATTACH the **already-existing plaintext source** with an
/// empty key (`KEY ''` ⇒ plaintext), then export `plaintext -> main`. This mirrors
/// how the live pool opens the encrypted file, so success here guarantees the pool
/// will open it too. Returns an error if the SQLCipher build is missing (the
/// `sqlcipher_export` function is unknown) — failing closed is correct.
async fn export_to_encrypted(src: &Path, dst: &Path, key_hex: &str) -> Result<()> {
    // Main connection = the encrypted destination (created here, keyed). Force the
    // rollback-journal (DELETE) mode so the cipher DB is a single self-contained
    // file — no `<dst>-wal`/`-shm` sidecars that the later single-file rename would
    // leave orphaned. The live pool re-opens `<db>` in WAL mode afterwards.
    let mut conn: SqliteConnection = SqliteConnectOptions::new()
        .filename(dst)
        .create_if_missing(true)
        .pragma("key", crate::secrets::db::pragma_key_value(key_hex))
        .journal_mode(SqliteJournalMode::Delete)
        .connect()
        .await
        .with_context(|| format!("failed to create the encrypted database {}", dst.display()))?;

    // ATTACH the plaintext source read-side with an empty key (plaintext). Paths are
    // single-quoted with any embedded quote doubled (SQL string-literal escaping).
    let src_literal = sql_string_literal(&src.to_string_lossy());
    let attach = format!("ATTACH DATABASE {src_literal} AS plaintext KEY ''");
    sqlx::query(&attach)
        .execute(&mut conn)
        .await
        .with_context(|| format!("failed to ATTACH the plaintext database {}", src.display()))?;

    // Copy every table (schema + data), including _sqlx_migrations, from the
    // plaintext source into the encrypted main DB.
    let exported: Result<()> = sqlx::query("SELECT sqlcipher_export('main', 'plaintext')")
        .execute(&mut conn)
        .await
        .map(|_| ())
        .context(
            "sqlcipher_export failed — the SQLite build may not be SQLCipher-enabled \
             (check the libsqlite3-sys 'bundled-sqlcipher-vendored-openssl' feature)",
        );

    // Always attempt to DETACH so the file handle is released before the swap, even
    // if the export failed.
    let _ = sqlx::query("DETACH DATABASE plaintext")
        .execute(&mut conn)
        .await;
    let _ = conn.close().await;

    exported?;

    // Sanity: the produced file must exist and must NOT carry the plaintext header.
    if !dst.exists() {
        return Err(anyhow!(
            "sqlcipher_export reported success but produced no file at {}",
            dst.display()
        ));
    }
    if has_plaintext_header(dst)? {
        return Err(anyhow!(
            "the exported database at {} is still plaintext — encryption did not take effect",
            dst.display()
        ));
    }
    Ok(())
}

/// Render `s` as a single-quoted SQL string literal, doubling embedded single quotes.
fn sql_string_literal(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

/// Probe whether the database at `path` opens (and can read `sqlite_master`) with
/// `key_hex` under SQLCipher. Used by tests and by the manager's diagnostics; a
/// wrong key yields `Ok(false)` (SQLCipher reports the file as "not a database").
///
/// Applies `PRAGMA key` first via [`SqliteConnectOptions::pragma`] (sqlx runs the
/// reserved `key` slot before anything else), matching how the live pool opens.
pub async fn opens_with_key(path: &Path, key_hex: &str) -> Result<bool> {
    let opts = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(false)
        .pragma("key", crate::secrets::db::pragma_key_value(key_hex));
    let mut conn = match opts.connect().await {
        Ok(c) => c,
        Err(_) => return Ok(false),
    };
    let ok = sqlx::query_scalar::<_, i64>("SELECT count(*) FROM sqlite_master")
        .fetch_one(&mut conn)
        .await
        .is_ok();
    let _ = conn.close().await;
    Ok(ok)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sql_literal_escapes_quotes() {
        assert_eq!(sql_string_literal("plain"), "'plain'");
        assert_eq!(sql_string_literal("O'Brien"), "'O''Brien'");
        assert_eq!(
            sql_string_literal(r"C:\Users\a b\meeting_minutes.sqlite"),
            r"'C:\Users\a b\meeting_minutes.sqlite'"
        );
    }

    #[test]
    fn missing_file_has_no_plaintext_header() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(!has_plaintext_header(&tmp.path().join("nope.sqlite")).unwrap());
    }

    #[test]
    fn short_file_is_not_plaintext() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("tiny");
        std::fs::write(&p, b"SQLite").unwrap();
        assert!(!has_plaintext_header(&p).unwrap());
    }

    #[test]
    fn plaintext_header_detected() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("plain.sqlite");
        let mut bytes = SQLITE_HEADER.to_vec();
        bytes.extend_from_slice(&[0u8; 100]);
        std::fs::write(&p, bytes).unwrap();
        assert!(has_plaintext_header(&p).unwrap());
    }

    #[test]
    fn sibling_appends_suffix() {
        let p = Path::new("/data/meeting_minutes.sqlite");
        assert_eq!(
            sibling(p, BACKUP_SUFFIX),
            PathBuf::from("/data/meeting_minutes.sqlite.pre-encryption")
        );
    }
}
