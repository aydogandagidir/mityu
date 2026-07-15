//! Verifiable deletion for Mityu-managed local meeting data (ADR-0026).
//!
//! SQLite core tables and FTS5 require separate secure-delete controls. A
//! successful maintenance cycle also truncates WAL and vacuums free pages. File
//! overwrites are defense in depth, not a forensic-erasure guarantee on SSD,
//! copy-on-write filesystems, snapshots, backups, swap, or exported copies.

use crate::context::AuthContext;
use crate::database::repositories::meeting::delete_meeting_with_connection;
use anyhow::{anyhow, bail, Context, Result};
use chrono::Utc;
use serde::Deserialize;
use sqlx::{Acquire, Row, SqliteConnection, SqlitePool};
use std::fs::{self, OpenOptions};
use std::io::{Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

const WIPE_BUFFER_BYTES: usize = 1024 * 1024;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ArtifactEraseReport {
    pub managed_files_removed: u64,
    pub managed_directories_removed: u64,
    pub retained_user_entries: u64,
}

/// Configure and verify both SQLite secure-deletion layers on one connection.
async fn verify_secure_delete_controls(conn: &mut SqliteConnection) -> Result<()> {
    sqlx::query("PRAGMA secure_delete = ON")
        .execute(&mut *conn)
        .await
        .context("enable SQLite secure_delete")?;

    let core_enabled: i64 = sqlx::query_scalar("PRAGMA secure_delete")
        .fetch_one(&mut *conn)
        .await
        .context("read SQLite secure_delete")?;
    if core_enabled != 1 {
        bail!("SQLite secure_delete is not enabled");
    }

    let fts_enabled: Option<i64> = sqlx::query_scalar(
        "SELECT CAST(v AS INTEGER) FROM transcript_search_fts_config \
         WHERE k = 'secure-delete'",
    )
    .fetch_optional(&mut *conn)
    .await
    .context("read FTS5 secure-delete configuration")?;
    if fts_enabled != Some(1) {
        bail!("FTS5 secure-delete is not enabled");
    }

    Ok(())
}

/// Delete tenant-scoped database records and atomically mark physical privacy
/// maintenance as pending. The marker contains no meeting id or content.
pub async fn delete_database_records(
    pool: &SqlitePool,
    ctx: &AuthContext,
    meeting_id: &str,
) -> Result<bool> {
    if meeting_id.trim().is_empty() {
        bail!("meeting_id cannot be empty");
    }

    let mut conn = pool
        .acquire()
        .await
        .context("acquire deletion connection")?;
    // This operation temporarily disables SQLite FK actions so a malformed
    // foreign-workspace child cannot be cascaded away with the caller's parent.
    // Never return that connection to the pool: cancellation, error and success
    // all close it, and the pool replaces it with normal foreign_keys=ON options.
    conn.close_on_drop();
    verify_secure_delete_controls(&mut conn).await?;
    sqlx::query("PRAGMA foreign_keys = OFF")
        .execute(&mut *conn)
        .await
        .context("disable foreign-key cascades on disposable deletion connection")?;
    let foreign_keys_enabled: i64 = sqlx::query_scalar("PRAGMA foreign_keys")
        .fetch_one(&mut *conn)
        .await
        .context("verify disposable deletion connection foreign-key mode")?;
    if foreign_keys_enabled != 0 {
        bail!("foreign-key cascades remain enabled on deletion connection");
    }
    let mut tx = conn.begin().await.context("begin meeting deletion")?;

    let deleted = delete_meeting_with_connection(&mut tx, ctx, meeting_id)
        .await
        .context("delete tenant-scoped meeting records")?;
    if !deleted {
        tx.rollback()
            .await
            .context("rollback absent meeting deletion")?;
        return Ok(false);
    }

    sqlx::query(
        "UPDATE local_privacy_maintenance \
         SET required = 1, completed_at = NULL WHERE singleton = 1",
    )
    .execute(&mut *tx)
    .await
    .context("mark privacy maintenance pending")?;

    tx.commit().await.context("commit meeting deletion")?;
    Ok(true)
}

pub async fn privacy_maintenance_required(pool: &SqlitePool) -> Result<bool> {
    let required: i64 =
        sqlx::query_scalar("SELECT required FROM local_privacy_maintenance WHERE singleton = 1")
            .fetch_one(pool)
            .await
            .context("read privacy maintenance marker")?;
    Ok(required == 1)
}

async fn checked_wal_truncate(conn: &mut SqliteConnection) -> Result<()> {
    let journal_mode: String = sqlx::query_scalar("PRAGMA journal_mode")
        .fetch_one(&mut *conn)
        .await
        .context("read SQLite journal mode")?;
    if !journal_mode.eq_ignore_ascii_case("wal") {
        // SQLite reports (0, -1, -1) for wal_checkpoint outside WAL mode.
        // There is no WAL file to scrub in that case; secure_delete + VACUUM
        // still covers the main database and rollback-journal lifecycle.
        return Ok(());
    }

    let row = sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
        .fetch_one(&mut *conn)
        .await
        .context("truncate SQLite WAL")?;
    let busy: i64 = row.try_get(0).context("read WAL busy result")?;
    let log_frames: i64 = row.try_get(1).context("read WAL frame result")?;
    let checkpointed: i64 = row.try_get(2).context("read WAL checkpoint result")?;

    if busy != 0 || log_frames != 0 || checkpointed != 0 {
        bail!(
            "WAL truncate was incomplete (busy={busy}, log={log_frames}, checkpointed={checkpointed})"
        );
    }
    Ok(())
}

/// Finish a pending SQLite/FTS deletion cycle. `required` is cleared only after
/// FTS optimization, checked WAL truncation, VACUUM, and free-page validation.
pub async fn complete_privacy_maintenance(pool: &SqlitePool) -> Result<()> {
    let mut conn = pool
        .acquire()
        .await
        .context("acquire maintenance connection")?;
    verify_secure_delete_controls(&mut conn).await?;

    sqlx::query("INSERT INTO transcript_search_fts(transcript_search_fts) VALUES('optimize')")
        .execute(&mut *conn)
        .await
        .context("optimize FTS5 secure-delete segments")?;
    checked_wal_truncate(&mut conn).await?;

    sqlx::query("VACUUM")
        .execute(&mut *conn)
        .await
        .context("vacuum SQLite free pages")?;
    checked_wal_truncate(&mut conn).await?;

    let free_pages: i64 = sqlx::query_scalar("PRAGMA freelist_count")
        .fetch_one(&mut *conn)
        .await
        .context("read SQLite freelist_count")?;
    if free_pages != 0 {
        bail!("SQLite VACUUM left {free_pages} free page(s)");
    }

    sqlx::query(
        "UPDATE local_privacy_maintenance \
         SET required = 0, completed_at = ? WHERE singleton = 1",
    )
    .bind(Utc::now().to_rfc3339())
    .execute(&mut *conn)
    .await
    .context("mark privacy maintenance complete")?;

    if let Err(error) = checked_wal_truncate(&mut conn).await {
        // The sensitive-content maintenance already completed, but preserve the
        // retry signal if the final marker checkpoint could not be proven.
        let _ = sqlx::query(
            "UPDATE local_privacy_maintenance \
             SET required = 1, completed_at = NULL WHERE singleton = 1",
        )
        .execute(&mut *conn)
        .await;
        return Err(error);
    }

    Ok(())
}

pub async fn ensure_folder_not_shared(
    pool: &SqlitePool,
    ctx: &AuthContext,
    meeting_id: &str,
    folder_path: &str,
) -> Result<()> {
    let has_other_reference = crate::database::repositories::meeting::MeetingsRepository::recording_folder_has_other_same_workspace_reference(
        pool,
        ctx,
        meeting_id,
        folder_path,
    )
    .await
    .context("check for a shared recording folder")?;

    if has_other_reference {
        bail!(
            "recording folder is referenced by another meeting; managed artifacts were not deleted"
        );
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
struct RecordingOwnershipMarker {
    #[serde(default)]
    workspace_id: Option<String>,
}

/// Prove that a recording folder belongs to the caller before touching any
/// managed artifact. New recordings always carry this marker. A pre-v1.0.4
/// folder with no marker (or no `workspace_id` field) is accepted only for the
/// single implicit `local` workspace; every other workspace fails closed.
fn validate_recording_workspace_ownership(target: &Path, ctx: &AuthContext) -> Result<()> {
    let marker_path = target.join("metadata.json");
    let marker_metadata = match fs::symlink_metadata(&marker_path) {
        Ok(metadata) => Some(metadata),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => {
            return Err(error).with_context(|| {
                format!(
                    "read recording ownership marker metadata: {}",
                    marker_path.display()
                )
            })
        }
    };

    let Some(marker_metadata) = marker_metadata else {
        if ctx.tenant_id.as_str() == crate::context::LOCAL_WORKSPACE_ID {
            return Ok(());
        }
        bail!("recording folder has no workspace ownership marker");
    };

    if is_link_or_reparse_point(&marker_metadata) || !marker_metadata.is_file() {
        bail!("recording ownership marker must be a regular file");
    }

    let marker_json = fs::read_to_string(&marker_path)
        .with_context(|| format!("read recording ownership marker: {}", marker_path.display()))?;
    let marker: RecordingOwnershipMarker =
        serde_json::from_str(&marker_json).with_context(|| {
            format!(
                "parse recording ownership marker: {}",
                marker_path.display()
            )
        })?;

    match marker.workspace_id.as_deref() {
        Some(workspace_id) if workspace_id == ctx.tenant_id.as_str() => Ok(()),
        Some(_) => bail!("recording folder belongs to a different workspace"),
        None if ctx.tenant_id.as_str() == crate::context::LOCAL_WORKSPACE_ID => Ok(()),
        None => bail!("legacy recording folder is restricted to the local workspace"),
    }
}

fn is_managed_top_level_file(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
        return false;
    };
    let lower = name.to_ascii_lowercase();

    if matches!(
        lower.as_str(),
        "metadata.json" | "transcripts.json" | ".metadata.json.tmp" | ".transcripts.json.tmp"
    ) || lower.starts_with(".metadata.json.")
    {
        return true;
    }

    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase);
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase);

    if stem.as_deref() == Some("audio") {
        return extension
            .as_deref()
            .is_some_and(|value| crate::audio::constants::AUDIO_EXTENSIONS.contains(&value));
    }

    let legacy_transcript = stem
        .as_deref()
        .and_then(|value| value.strip_prefix("transcript_"))
        .is_some_and(is_legacy_transcript_timestamp)
        && matches!(extension.as_deref(), Some("txt" | "json"));
    let legacy_recording = stem
        .as_deref()
        .and_then(|value| value.strip_prefix("recording_"))
        .is_some_and(is_legacy_recording_timestamp)
        && extension
            .as_deref()
            .is_some_and(|value| crate::audio::constants::AUDIO_EXTENSIONS.contains(&value));

    legacy_transcript || legacy_recording
}

fn has_digit_pattern(value: &str, separators: &[(usize, u8)], expected_len: usize) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == expected_len
        && bytes.iter().enumerate().all(|(index, byte)| {
            separators
                .iter()
                .find(|(separator_index, _)| *separator_index == index)
                .map_or_else(|| byte.is_ascii_digit(), |(_, expected)| byte == expected)
        })
}

fn is_legacy_transcript_timestamp(value: &str) -> bool {
    has_digit_pattern(
        value,
        &[(4, b'-'), (7, b'-'), (10, b'_'), (13, b'-'), (16, b'-')],
        19,
    )
}

fn is_legacy_recording_timestamp(value: &str) -> bool {
    has_digit_pattern(value, &[(8, b'_')], 15)
}

fn is_link_or_reparse_point(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }

    // Windows directory junctions and other reparse points are not always
    // reported as symbolic links. Treat all of them as links so recursive
    // cleanup never follows a managed-folder entry into another location.
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return true;
        }
    }

    false
}

fn open_managed_artifact_without_following_windows_reparse(path: &Path) -> Result<fs::File> {
    let mut options = OpenOptions::new();
    options.write(true);

    // Keep the path-level link guard effective across the metadata/open gap on
    // Windows. Opening the reparse point itself lets the handle-level check
    // below reject it instead of following a link that replaced the file.
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
        options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }

    options
        .open(path)
        .with_context(|| format!("open managed artifact for overwrite: {}", path.display()))
}

#[cfg(unix)]
fn verified_opened_file_length_and_link_count(
    path: &Path,
    preflight: &fs::Metadata,
    file: &fs::File,
) -> Result<(u64, u64)> {
    use std::os::unix::fs::MetadataExt;

    let opened = file
        .metadata()
        .with_context(|| format!("read opened managed artifact metadata: {}", path.display()))?;
    if !opened.file_type().is_file() {
        bail!(
            "opened managed artifact is not a regular file: {}",
            path.display()
        );
    }

    // fstat-style metadata belongs to the open descriptor. Comparing its
    // device/inode with the earlier lstat prevents a replacement (including a
    // symlink followed by open) from being wiped after the path check.
    if preflight.dev() != opened.dev() || preflight.ino() != opened.ino() {
        bail!(
            "managed artifact changed while it was being opened; refusing overwrite: {}",
            path.display()
        );
    }

    Ok((opened.len(), opened.nlink()))
}

#[cfg(windows)]
fn verified_opened_file_length_and_link_count(
    path: &Path,
    _preflight: &fs::Metadata,
    file: &fs::File,
) -> Result<(u64, u64)> {
    use std::ffi::c_void;
    use std::mem::MaybeUninit;
    use std::os::windows::io::AsRawHandle;

    #[repr(C)]
    struct ByHandleFileInformation {
        file_attributes: u32,
        creation_time: [u32; 2],
        last_access_time: [u32; 2],
        last_write_time: [u32; 2],
        volume_serial_number: u32,
        file_size_high: u32,
        file_size_low: u32,
        number_of_links: u32,
        file_index_high: u32,
        file_index_low: u32,
    }

    #[link(name = "Kernel32")]
    extern "system" {
        fn GetFileInformationByHandle(
            file: *mut c_void,
            information: *mut ByHandleFileInformation,
        ) -> i32;
    }

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;
    let mut information = MaybeUninit::<ByHandleFileInformation>::uninit();
    // SAFETY: `file` owns a valid Windows handle for the duration of the call,
    // and `information` points to writable storage of the exact Win32 layout.
    let succeeded =
        unsafe { GetFileInformationByHandle(file.as_raw_handle(), information.as_mut_ptr()) };
    if succeeded == 0 {
        return Err(std::io::Error::last_os_error())
            .with_context(|| format!("read opened managed artifact metadata: {}", path.display()));
    }

    // SAFETY: a nonzero result guarantees that Win32 initialized the structure.
    let information = unsafe { information.assume_init() };
    if information.file_attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        bail!(
            "opened managed artifact is a reparse point; refusing overwrite: {}",
            path.display()
        );
    }

    let length =
        (u64::from(information.file_size_high) << 32) | u64::from(information.file_size_low);
    Ok((length, u64::from(information.number_of_links)))
}

fn overwrite_and_remove_file(path: &Path) -> Result<()> {
    let preflight = fs::symlink_metadata(path)
        .with_context(|| format!("read managed artifact metadata: {}", path.display()))?;
    if is_link_or_reparse_point(&preflight) || !preflight.file_type().is_file() {
        bail!("managed artifact is not a regular file: {}", path.display());
    }

    let mut file = open_managed_artifact_without_following_windows_reparse(path)?;
    let (length, link_count) = verified_opened_file_length_and_link_count(path, &preflight, &file)?;
    if link_count != 1 {
        // Overwriting one directory entry would also overwrite every external
        // hard link to the same inode/file record. Fail closed and leave all
        // links untouched; the user can separate the file before retrying.
        bail!(
            "managed artifact has {link_count} hard links; refusing overwrite: {}",
            path.display()
        );
    }

    file.seek(SeekFrom::Start(0))
        .with_context(|| format!("seek managed artifact: {}", path.display()))?;

    let zeros = vec![0_u8; WIPE_BUFFER_BYTES];
    let mut remaining = length;
    while remaining > 0 {
        let chunk = remaining.min(zeros.len() as u64) as usize;
        file.write_all(&zeros[..chunk])
            .with_context(|| format!("overwrite managed artifact: {}", path.display()))?;
        remaining -= chunk as u64;
    }
    file.sync_all()
        .with_context(|| format!("sync managed artifact overwrite: {}", path.display()))?;
    file.set_len(0)
        .with_context(|| format!("truncate managed artifact: {}", path.display()))?;
    file.sync_all()
        .with_context(|| format!("sync managed artifact truncation: {}", path.display()))?;
    drop(file);
    fs::remove_file(path)
        .with_context(|| format!("unlink managed artifact: {}", path.display()))?;
    Ok(())
}

fn unlink_symlink(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(file_error) => fs::remove_dir(path).map_err(|dir_error| {
            anyhow!(
                "could not unlink symlink {} (file: {}; directory: {})",
                path.display(),
                file_error,
                dir_error
            )
        }),
    }
}

fn scrub_checkpoint_tree(path: &Path, report: &mut ArtifactEraseReport) -> Result<()> {
    for entry in fs::read_dir(path)
        .with_context(|| format!("read checkpoint directory: {}", path.display()))?
    {
        let entry = entry.context("read checkpoint entry")?;
        let child = entry.path();
        let metadata = fs::symlink_metadata(&child)
            .with_context(|| format!("read checkpoint entry metadata: {}", child.display()))?;

        if is_link_or_reparse_point(&metadata) {
            unlink_symlink(&child)?;
            report.managed_files_removed += 1;
        } else if metadata.is_dir() {
            scrub_checkpoint_tree(&child, report)?;
            fs::remove_dir(&child)
                .with_context(|| format!("remove checkpoint directory: {}", child.display()))?;
            report.managed_directories_removed += 1;
        } else if metadata.is_file() {
            overwrite_and_remove_file(&child)?;
            report.managed_files_removed += 1;
        } else {
            bail!("unsupported checkpoint artifact type: {}", child.display());
        }
    }
    Ok(())
}

/// Scrub only Mityu-managed artifacts in a meeting folder. Unknown top-level
/// entries are retained so deleting a meeting cannot destroy unrelated files a
/// user placed in that directory. Symlinks are unlinked and never followed.
pub fn erase_recording_folder(
    target: &Path,
    allowed_recording_roots: &[PathBuf],
    ctx: &AuthContext,
) -> Result<ArtifactEraseReport> {
    if !target.exists() {
        return Ok(ArtifactEraseReport::default());
    }
    let canonical_target = validate_managed_recording_folder(target, allowed_recording_roots)?;
    validate_recording_workspace_ownership(&canonical_target, ctx)?;

    let mut report = ArtifactEraseReport::default();
    for entry in fs::read_dir(&canonical_target)
        .with_context(|| format!("read recording folder: {}", canonical_target.display()))?
    {
        let entry = entry.context("read recording-folder entry")?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)
            .with_context(|| format!("read recording entry metadata: {}", path.display()))?;

        let managed_name = is_managed_top_level_file(&path);
        let checkpoint_root = entry.file_name() == ".checkpoints";

        if is_link_or_reparse_point(&metadata) && (managed_name || checkpoint_root) {
            unlink_symlink(&path)?;
            report.managed_files_removed += 1;
        } else if is_link_or_reparse_point(&metadata) {
            report.retained_user_entries += 1;
        } else if metadata.is_dir() && checkpoint_root {
            scrub_checkpoint_tree(&path, &mut report)?;
            fs::remove_dir(&path)
                .with_context(|| format!("remove checkpoint root: {}", path.display()))?;
            report.managed_directories_removed += 1;
        } else if metadata.is_file() && managed_name {
            overwrite_and_remove_file(&path)?;
            report.managed_files_removed += 1;
        } else {
            report.retained_user_entries += 1;
        }
    }

    if report.retained_user_entries == 0 {
        fs::remove_dir(&canonical_target).with_context(|| {
            format!(
                "remove empty recording folder: {}",
                canonical_target.display()
            )
        })?;
        report.managed_directories_removed += 1;
    }

    Ok(report)
}

/// Validate an existing meeting folder before any native filesystem operation.
/// The lexical and canonical checks are both required: canonical containment
/// blocks traversal outside the Mityu root, while walking every lexical path
/// component rejects symlinks and Windows junctions even when they resolve back
/// inside that root.
pub fn validate_managed_recording_folder(
    target: &Path,
    allowed_recording_roots: &[PathBuf],
) -> Result<PathBuf> {
    if !target.is_absolute() {
        bail!("recording folder must be an absolute path");
    }

    let target_metadata = fs::symlink_metadata(target)
        .with_context(|| format!("read recording folder metadata: {}", target.display()))?;
    if is_link_or_reparse_point(&target_metadata) || !target_metadata.is_dir() {
        bail!("recording folder must be a real directory, not a link or file");
    }
    let canonical_target = fs::canonicalize(target)
        .with_context(|| format!("canonicalize recording folder: {}", target.display()))?;

    let allowed = allowed_recording_roots.iter().any(|root| {
        if !root.is_absolute() || !root.exists() {
            return false;
        }

        let Ok(root_metadata) = fs::symlink_metadata(root) else {
            return false;
        };
        if is_link_or_reparse_point(&root_metadata) || !root_metadata.is_dir() {
            return false;
        }

        let Ok(relative) = target.strip_prefix(root) else {
            return false;
        };
        if relative.as_os_str().is_empty() {
            return false;
        }

        let mut cursor = root.clone();
        for component in relative.components() {
            let std::path::Component::Normal(part) = component else {
                return false;
            };
            cursor.push(part);
            let Ok(metadata) = fs::symlink_metadata(&cursor) else {
                return false;
            };
            if is_link_or_reparse_point(&metadata) {
                return false;
            }
        }

        fs::canonicalize(root)
            .map(|canonical_root| {
                canonical_target != canonical_root && canonical_target.starts_with(canonical_root)
            })
            .unwrap_or(false)
    });
    if !allowed {
        bail!("recording folder is outside the managed Mityu recording root or contains a link");
    }

    Ok(canonical_target)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuses_root_itself_and_paths_outside_allowed_root() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().join("recordings");
        let outside = temp.path().join("outside");
        fs::create_dir_all(&root).expect("root");
        fs::create_dir_all(&outside).expect("outside");

        let ctx = AuthContext::local();
        assert!(erase_recording_folder(&root, std::slice::from_ref(&root), &ctx).is_err());
        assert!(erase_recording_folder(&outside, &[root], &ctx).is_err());
    }

    #[test]
    fn removes_managed_artifacts_but_retains_unknown_user_file() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().join("recordings");
        let meeting = root.join("Meeting_2026-07-14_10-00");
        let checkpoints = meeting.join(".checkpoints");
        fs::create_dir_all(&checkpoints).expect("folders");
        fs::write(meeting.join("audio.mp4"), b"sensitive audio").expect("audio");
        fs::write(
            meeting.join("metadata.json"),
            br#"{"workspace_id":"local","note":"sensitive metadata"}"#,
        )
        .expect("metadata");
        fs::write(
            meeting.join(".metadata.json.interrupted-write.tmp"),
            b"sensitive temporary metadata",
        )
        .expect("temporary metadata");
        fs::write(checkpoints.join("chunk_0001.mp4"), b"sensitive checkpoint").expect("checkpoint");
        fs::write(meeting.join("keep.txt"), b"user file").expect("user file");
        for name in [
            "audio.notes",
            "recording_budget.xlsx",
            "transcript_contract.pdf",
            "system_backup.txt",
        ] {
            fs::write(meeting.join(name), b"user file with a misleading prefix")
                .expect("prefixed user file");
        }

        let report = erase_recording_folder(&meeting, &[root], &AuthContext::local())
            .expect("erase managed data");
        assert_eq!(report.managed_files_removed, 4);
        assert_eq!(report.retained_user_entries, 5);
        assert!(meeting.join("keep.txt").exists());
        assert!(meeting.join("audio.notes").exists());
        assert!(meeting.join("recording_budget.xlsx").exists());
        assert!(meeting.join("transcript_contract.pdf").exists());
        assert!(meeting.join("system_backup.txt").exists());
        assert!(!meeting.join("audio.mp4").exists());
        assert!(!meeting.join("metadata.json").exists());
        assert!(!meeting
            .join(".metadata.json.interrupted-write.tmp")
            .exists());
        assert!(!meeting.join(".checkpoints").exists());
    }

    #[test]
    fn foreign_workspace_marker_fails_closed_before_artifact_erasure() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().join("recordings");
        let meeting = root.join("workspace-other").join("Meeting");
        fs::create_dir_all(&meeting).expect("meeting folder");
        fs::write(
            meeting.join("metadata.json"),
            br#"{"workspace_id":"other-workspace"}"#,
        )
        .expect("ownership marker");
        fs::write(meeting.join("audio.mp4"), b"must survive").expect("audio");

        let error = erase_recording_folder(&meeting, &[root], &AuthContext::local())
            .expect_err("foreign marker must fail closed");
        assert!(error.to_string().contains("different workspace"));
        assert_eq!(
            fs::read(meeting.join("audio.mp4")).expect("audio survives"),
            b"must survive"
        );
        assert!(meeting.join("metadata.json").exists());
    }

    #[test]
    fn legacy_marker_without_workspace_is_accepted_only_for_local_context() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().join("recordings");
        let meeting = root.join("Legacy_Meeting");
        fs::create_dir_all(&meeting).expect("meeting folder");
        fs::write(meeting.join("metadata.json"), br#"{"version":"1.0"}"#).expect("legacy marker");
        fs::write(meeting.join("audio.mp4"), b"legacy audio").expect("legacy audio");

        let mut foreign = AuthContext::local();
        foreign.tenant_id = crate::context::TenantId::new("other-workspace");
        let error = erase_recording_folder(&meeting, std::slice::from_ref(&root), &foreign)
            .expect_err("legacy marker must not be claimed by a foreign workspace");
        assert!(error
            .to_string()
            .contains("restricted to the local workspace"));
        assert!(meeting.join("audio.mp4").exists());

        let report = erase_recording_folder(&meeting, &[root], &AuthContext::local())
            .expect("implicit local workspace may erase legacy recording");
        assert_eq!(report.managed_files_removed, 2);
        assert!(!meeting.exists());
    }

    #[test]
    fn refuses_managed_artifact_hard_link_without_modifying_external_content() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().join("recordings");
        let meeting = root.join("Meeting_2026-07-14_10-00");
        let outside = temp.path().join("outside");
        let external = outside.join("original-audio.mp4");
        let managed = meeting.join("audio.mp4");
        let sentinel = b"external content must not be overwritten";
        fs::create_dir_all(&meeting).expect("meeting folder");
        fs::create_dir_all(&outside).expect("outside folder");
        fs::write(&external, sentinel).expect("external file");
        fs::hard_link(&external, &managed).expect("managed hard link");

        let error = erase_recording_folder(&meeting, &[root], &AuthContext::local())
            .expect_err("multiple hard links must fail closed");

        assert!(error.to_string().contains("hard links"));
        assert_eq!(
            fs::read(&external).expect("external remains readable"),
            sentinel
        );
        assert_eq!(
            fs::read(&managed).expect("managed link remains readable"),
            sentinel
        );
    }

    #[test]
    fn refuses_checkpoint_hard_link_without_modifying_external_content() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().join("recordings");
        let meeting = root.join("Meeting_2026-07-14_10-00");
        let checkpoints = meeting.join(".checkpoints");
        let outside = temp.path().join("outside");
        let external = outside.join("original-checkpoint.mp4");
        let managed = checkpoints.join("chunk_0001.mp4");
        let sentinel = b"external checkpoint content must not be overwritten";
        fs::create_dir_all(&checkpoints).expect("checkpoint folder");
        fs::create_dir_all(&outside).expect("outside folder");
        fs::write(&external, sentinel).expect("external checkpoint");
        fs::hard_link(&external, &managed).expect("checkpoint hard link");

        let error = erase_recording_folder(&meeting, &[root], &AuthContext::local())
            .expect_err("checkpoint hard link must fail closed");

        assert!(error.to_string().contains("hard links"));
        assert_eq!(
            fs::read(&external).expect("external remains readable"),
            sentinel
        );
        assert_eq!(
            fs::read(&managed).expect("managed link remains readable"),
            sentinel
        );
    }

    #[cfg(windows)]
    #[test]
    fn unlinks_checkpoint_junction_without_following_its_target() {
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().join("recordings");
        let meeting = root.join("Meeting_2026-07-14_10-00");
        let outside = temp.path().join("outside");
        let junction = meeting.join(".checkpoints");
        fs::create_dir_all(&meeting).expect("meeting folder");
        fs::create_dir_all(&outside).expect("outside folder");
        fs::write(outside.join("must-survive.txt"), b"outside user data").expect("outside file");

        let status = std::process::Command::new("cmd.exe")
            .args(["/C", "mklink", "/J"])
            .arg(&junction)
            .arg(&outside)
            .status()
            .expect("run mklink");
        assert!(status.success(), "create test junction");

        let report = erase_recording_folder(&meeting, &[root], &AuthContext::local())
            .expect("unlink junction safely");
        assert_eq!(report.managed_files_removed, 1);
        assert!(outside.join("must-survive.txt").exists());
        assert!(!junction.exists());
    }
}
