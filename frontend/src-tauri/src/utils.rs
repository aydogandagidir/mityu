pub fn format_timestamp(seconds: f64) -> String {
    let total_seconds = seconds as u64;
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let secs = total_seconds % 60;
    format!("{:02}:{:02}:{:02}", hours, minutes, secs)
}

/// Verify a downloaded artifact against an immutable manifest entry. Model
/// files are executable inputs to native inference runtimes, so neither an
/// approximate size check nor a mutable upstream URL is sufficient.
pub async fn verify_file_integrity(
    path: &std::path::Path,
    expected_size: u64,
    expected_sha256: &str,
) -> anyhow::Result<()> {
    use anyhow::{bail, Context};
    use sha2::{Digest, Sha256};
    use tokio::io::AsyncReadExt;

    if expected_sha256.len() != 64 || !expected_sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        bail!("invalid trusted SHA-256 manifest entry");
    }

    let metadata = tokio::fs::metadata(path)
        .await
        .context("read downloaded artifact metadata")?;
    if metadata.len() != expected_size {
        bail!(
            "downloaded artifact size mismatch: expected {expected_size}, got {}",
            metadata.len()
        );
    }

    let mut file = tokio::fs::File::open(path)
        .await
        .context("open downloaded artifact for verification")?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; 1024 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .await
            .context("read downloaded artifact for verification")?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }

    let actual = format!("{:x}", hasher.finalize());
    if !actual.eq_ignore_ascii_case(expected_sha256) {
        bail!("downloaded artifact SHA-256 mismatch");
    }
    Ok(())
}

#[cfg(test)]
mod integrity_tests {
    use super::verify_file_integrity;
    use sha2::{Digest, Sha256};

    #[tokio::test]
    async fn verifies_exact_size_and_digest() {
        let temp = tempfile::NamedTempFile::new().expect("temporary file");
        std::fs::write(temp.path(), b"mityu-model").expect("write fixture");
        let digest = format!("{:x}", Sha256::digest(b"mityu-model"));

        verify_file_integrity(temp.path(), 11, &digest)
            .await
            .expect("valid artifact");
        assert!(verify_file_integrity(temp.path(), 10, &digest)
            .await
            .is_err());
        assert!(verify_file_integrity(temp.path(), 11, &"0".repeat(64))
            .await
            .is_err());
    }
}

/// Opens macOS System Settings to a specific privacy preference pane
#[cfg(target_os = "macos")]
#[tauri::command]
pub async fn open_system_settings(preference_pane: String) -> Result<(), String> {
    use std::process::Command;

    // Construct the URL for System Settings
    let url = format!(
        "x-apple.systempreferences:com.apple.preference.security?{}",
        preference_pane
    );

    // Use the 'open' command on macOS to open the URL
    Command::new("open")
        .arg(&url)
        .spawn()
        .map_err(|e| format!("Failed to open system settings: {}", e))?;

    Ok(())
}
