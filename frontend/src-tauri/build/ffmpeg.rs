// ============================================================================
// FFmpeg Binary Bundling
// ============================================================================
// Download and bundle FFmpeg binaries at build-time to eliminate runtime download delays

/// Download and bundle FFmpeg binary for current target platform
/// Checks cache first, downloads only if missing or corrupted
pub fn ensure_ffmpeg_binary() {
    let target = std::env::var("TARGET")
        .or_else(|_| std::env::var("HOST"))
        .expect("Neither TARGET nor HOST environment variable set");

    println!(
        "cargo:warning=🎬 Checking FFmpeg binary for target: {}",
        target
    );

    let binary_name = if target.contains("windows") {
        format!("ffmpeg-{}.exe", target)
    } else {
        format!("ffmpeg-{}", target)
    };

    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
        .expect("CARGO_MANIFEST_DIR environment variable not set");
    let binaries_dir = std::path::PathBuf::from(&manifest_dir).join("binaries");
    let binary_path = binaries_dir.join(&binary_name);

    // Cache check: Skip download if binary exists and works
    if binary_path.exists() {
        println!(
            "cargo:warning=🔍 Found cached FFmpeg binary: {}",
            binary_name
        );
        if verify_ffmpeg_binary(&binary_path, &target) {
            println!(
                "cargo:warning=✅ FFmpeg binary already cached and verified: {}",
                binary_name
            );
            return;
        } else {
            println!("cargo:warning=⚠️  Cached FFmpeg binary appears corrupted, re-downloading...");
            let _ = std::fs::remove_file(&binary_path);
        }
    }

    println!(
        "cargo:warning=📥 FFmpeg binary not found, downloading for {}",
        target
    );

    // Create binaries directory if it doesn't exist
    if !binaries_dir.exists() {
        std::fs::create_dir_all(&binaries_dir).expect("Failed to create binaries directory");
    }

    // Download and extract
    match download_and_extract_ffmpeg(&target, &binary_path) {
        Ok(()) => {
            println!(
                "cargo:warning=✅ FFmpeg binary downloaded successfully: {}",
                binary_name
            );

            // Verify downloaded binary works
            if !verify_ffmpeg_binary(&binary_path, &target) {
                panic!("⚠️  Downloaded FFmpeg binary verification failed!");
            }
        }
        Err(e) => {
            panic!("⚠️  Failed to download FFmpeg: {}", e);
        }
    }
}

/// Download FFmpeg from platform-specific URL and extract to target location
fn download_and_extract_ffmpeg(
    target: &str,
    output_path: &std::path::PathBuf,
) -> Result<(), String> {
    use sha2::{Digest, Sha256};
    use std::io::Write;

    println!(
        "cargo:warning=🌐 Fetching FFmpeg download URL for {}",
        target
    );

    // Get platform-specific download URL
    let url = get_ffmpeg_url_for_target(target)?;

    println!("cargo:warning=⬇️  Downloading from: {}", url);

    // Download with timeout (using reqwest from build-dependencies)
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(600)) // 10 min timeout for large downloads
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let response = client
        .get(&url)
        .send()
        .map_err(|e| format!("Failed to download: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("HTTP error: {}", response.status()));
    }

    let total_size = response.content_length().unwrap_or(0);
    println!(
        "cargo:warning=📦 Download size: {:.1} MB",
        total_size as f64 / 1_048_576.0
    );

    // Download to temp file
    let temp_dir = std::env::temp_dir();
    let archive_filename = url.split('/').next_back().unwrap_or("ffmpeg-archive");
    let archive_path = temp_dir.join(format!("ffmpeg-build-{}-{}", target, archive_filename));

    {
        let mut file = std::fs::File::create(&archive_path)
            .map_err(|e| format!("Failed to create temp file: {}", e))?;

        let content = response
            .bytes()
            .map_err(|e| format!("Failed to read response: {}", e))?;

        if let Some(expected_sha256) = expected_ffmpeg_archive_sha256(target) {
            let actual_sha256 = format!("{:x}", Sha256::digest(&content));
            if actual_sha256 != expected_sha256 {
                return Err(format!(
                    "FFmpeg archive SHA-256 mismatch for {target}: expected {expected_sha256}, got {actual_sha256}"
                ));
            }
            println!(
                "cargo:warning=FFmpeg archive SHA-256 verified for {}",
                target
            );
        } else {
            println!(
                "cargo:warning=FFmpeg archive has no approved checksum for {}; this target is not production-release eligible",
                target
            );
        }

        file.write_all(&content)
            .map_err(|e| format!("Failed to write archive: {}", e))?;
    }

    println!("cargo:warning=📦 Downloaded to: {:?}", archive_path);
    println!("cargo:warning=📂 Extracting FFmpeg binary...");

    // Extract binary (platform-specific)
    extract_ffmpeg_from_archive(&archive_path, target, output_path)?;

    // Cleanup archive
    let _ = std::fs::remove_file(&archive_path);

    println!("cargo:warning=✨ Extraction complete");

    Ok(())
}

/// SHA-256 digests published by GitHub for Mityu's pinned LGPL FFmpeg assets.
/// macOS deliberately has no approved digest because its current upstream
/// archive has not passed the license/provenance gate and is excluded from the
/// v1.0.4 production release matrix.
fn expected_ffmpeg_archive_sha256(target: &str) -> Option<&'static str> {
    if target.contains("windows") {
        Some("e2757eb478954028a18be862cc5927f585524f0f000d69e36fe35283aba157db")
    } else if target.contains("linux") && (target.contains("aarch64") || target.contains("arm")) {
        Some("4830e419054f198d5b38f77a33310366d2825673f0dce1d82c8541fa4144749c")
    } else if target.contains("linux") {
        Some("90236926a76974e230f85917e4962e39307a23140e661ccd1ee85f3cda0145f2")
    } else {
        None
    }
}

/// Get FFmpeg download URL for specific target triple
fn get_ffmpeg_url_for_target(target: &str) -> Result<String, String> {
    // Windows and Linux: self-hosted LGPL-only static builds (unmodified
    // re-uploads of BtbN/FFmpeg-Builds' pinned n8.1 release), pinned to our
    // own GitHub release so this doesn't depend on a third party's servers
    // or a `:latest` tag that can move under a reproducible build. Mityu
    // only ever transcodes raw audio to AAC/MP4 (no `-c:v`, no video codec
    // ever used), so LGPL is sufficient — the previous gyan.dev-derived
    // build was `--enable-gpl`, which would put a GPL source-redistribution
    // obligation on this project as a commercial distributor for
    // functionality it doesn't use. See docs/DECISIONS.md ADR-0021.
    const MITYU_FFMPEG_RELEASE: &str =
        "https://github.com/aydogandagidir/mityu/releases/download/ffmpeg-deps-8.1-lgpl";

    let url = if target.contains("windows") {
        format!("{}/win64-lgpl.zip", MITYU_FFMPEG_RELEASE)
    } else if target.contains("apple") {
        // macOS: not yet mirrored — no equivalently-licensed (LGPL-only)
        // static macOS build has been sourced/verified. Still on the
        // original third-party mirror pending a separate fix.
        if target.contains("aarch64") {
            // Apple Silicon (M1/M2/M3)
            "https://github.com/Zackriya-Solutions/ffmpeg-binaries/releases/download/0.0.1/ffmpeg80arm.zip".to_string()
        } else {
            // Intel Mac
            "https://github.com/Zackriya-Solutions/ffmpeg-binaries/releases/download/0.0.1/ffmpeg-8.0.1.zip".to_string()
        }
    } else if target.contains("linux") {
        if target.contains("aarch64") || target.contains("arm") {
            format!("{}/linuxarm64-lgpl.tar.xz", MITYU_FFMPEG_RELEASE)
        } else {
            format!("{}/linux64-lgpl.tar.xz", MITYU_FFMPEG_RELEASE)
        }
    } else {
        return Err(format!("Unsupported target platform: {}", target));
    };

    Ok(url)
}

/// Extract FFmpeg binary from downloaded archive (handles ZIP and TAR.XZ)
fn extract_ffmpeg_from_archive(
    archive_path: &std::path::Path,
    target: &str,
    output_path: &std::path::PathBuf,
) -> Result<(), String> {
    let extract_dir = std::env::temp_dir().join(format!("ffmpeg-extract-{}", target));

    // Clean old extraction directory
    let _ = std::fs::remove_dir_all(&extract_dir);
    std::fs::create_dir_all(&extract_dir)
        .map_err(|e| format!("Failed to create extract dir: {}", e))?;

    // Determine archive format from extension
    let archive_str = archive_path.to_string_lossy();

    if archive_str.ends_with(".zip") {
        extract_zip(archive_path, &extract_dir)?;
    } else if archive_str.ends_with(".tar.xz") || archive_str.ends_with(".txz") {
        extract_tar_xz(archive_path, &extract_dir)?;
    } else {
        return Err(format!("Unsupported archive format: {}", archive_str));
    }

    // Find extracted FFmpeg binary (platform-specific locations)
    let ffmpeg_binary = find_ffmpeg_in_extracted_dir(&extract_dir, target)?;

    println!("cargo:warning=📋 Found FFmpeg at: {:?}", ffmpeg_binary);

    // Copy to target location
    std::fs::copy(&ffmpeg_binary, output_path)
        .map_err(|e| format!("Failed to copy binary to binaries/: {}", e))?;

    // Set executable permissions on Unix systems
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(output_path)
            .map_err(|e| format!("Failed to get metadata: {}", e))?
            .permissions();
        perms.set_mode(0o755); // rwxr-xr-x
        std::fs::set_permissions(output_path, perms)
            .map_err(|e| format!("Failed to set executable permissions: {}", e))?;
        println!("cargo:warning=🔐 Set executable permissions");
    }

    // Cleanup extraction directory
    let _ = std::fs::remove_dir_all(&extract_dir);

    Ok(())
}

/// Extract ZIP archive (Windows, macOS)
fn extract_zip(
    archive_path: &std::path::Path,
    extract_dir: &std::path::Path,
) -> Result<(), String> {
    let file =
        std::fs::File::open(archive_path).map_err(|e| format!("Failed to open ZIP: {}", e))?;

    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| format!("Failed to read ZIP archive: {}", e))?;

    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| format!("Failed to read ZIP entry {}: {}", i, e))?;

        // Use enclosed_name() to prevent Zip Slip path traversal attacks
        let outpath = match file.enclosed_name() {
            Some(name) => extract_dir.join(name),
            None => {
                // Skip entries with path traversal sequences (e.g., "../")
                println!(
                    "cargo:warning=⚠️  Skipping suspicious ZIP entry: {}",
                    file.name()
                );
                continue;
            }
        };

        if file.is_dir() {
            // Directory
            std::fs::create_dir_all(&outpath)
                .map_err(|e| format!("Failed to create directory: {}", e))?;
        } else {
            // File
            if let Some(parent) = outpath.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| format!("Failed to create parent directory: {}", e))?;
            }

            let mut outfile = std::fs::File::create(&outpath)
                .map_err(|e| format!("Failed to create output file: {}", e))?;

            std::io::copy(&mut file, &mut outfile)
                .map_err(|e| format!("Failed to extract file: {}", e))?;
        }

        // Set Unix permissions if available
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Some(mode) = file.unix_mode() {
                std::fs::set_permissions(&outpath, std::fs::Permissions::from_mode(mode)).ok();
            }
        }
    }

    Ok(())
}

/// Extract TAR.XZ archive (Linux)
fn extract_tar_xz(
    archive_path: &std::path::Path,
    extract_dir: &std::path::Path,
) -> Result<(), String> {
    let file =
        std::fs::File::open(archive_path).map_err(|e| format!("Failed to open TAR.XZ: {}", e))?;

    // Decompress XZ
    let decompressor = xz2::read::XzDecoder::new(file);

    // Extract TAR
    let mut archive = tar::Archive::new(decompressor);
    archive
        .unpack(extract_dir)
        .map_err(|e| format!("Failed to extract TAR: {}", e))?;

    Ok(())
}

/// Find FFmpeg binary in extracted directory (handles nested structures)
fn find_ffmpeg_in_extracted_dir(
    extract_dir: &std::path::Path,
    target: &str,
) -> Result<std::path::PathBuf, String> {
    let executable_name = if target.contains("windows") {
        "ffmpeg.exe"
    } else {
        "ffmpeg"
    };

    // Search patterns (in priority order)
    let search_patterns = [
        extract_dir.join(executable_name),             // Flat: ffmpeg
        extract_dir.join("bin").join(executable_name), // Nested: bin/ffmpeg
    ];

    // Try direct paths first
    for pattern in &search_patterns {
        if pattern.exists() && pattern.is_file() {
            return Ok(pattern.clone());
        }
    }

    // Recursive search for nested directories (e.g., ffmpeg-6.0-full_build/bin/ffmpeg.exe)
    for entry in
        std::fs::read_dir(extract_dir).map_err(|e| format!("Failed to read extract dir: {}", e))?
    {
        let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
        let path = entry.path();

        if path.is_dir() {
            // Check bin/ subdirectory
            let bin_path = path.join("bin").join(executable_name);
            if bin_path.exists() && bin_path.is_file() {
                return Ok(bin_path);
            }

            // Check root of subdirectory
            let root_path = path.join(executable_name);
            if root_path.exists() && root_path.is_file() {
                return Ok(root_path);
            }
        }
    }

    Err(format!(
        "FFmpeg binary '{}' not found in extracted archive",
        executable_name
    ))
}

/// Verify FFmpeg is functional and does not violate Mityu's LGPL-only policy.
/// This also invalidates stale local caches that predate the pinned archive.
fn verify_ffmpeg_binary(path: &std::path::Path, target: &str) -> bool {
    use sha2::{Digest, Sha256};

    // The production Windows executable extracted from the pinned archive is
    // pinned independently as well. This prevents a pre-populated local cache
    // from bypassing archive verification with a merely clean-looking banner.
    if target.contains("x86_64-pc-windows-msvc") {
        const EXPECTED_WINDOWS_BINARY_SHA256: &str =
            "dd757098407e2ac4920647a2f66f41a6e1006dcf373b0825023948ae1b96912a";
        let actual_sha256 = match std::fs::read(path) {
            Ok(bytes) => format!("{:x}", Sha256::digest(bytes)),
            Err(_) => return false,
        };

        if actual_sha256 != EXPECTED_WINDOWS_BINARY_SHA256 {
            println!(
                "cargo:warning=Rejected FFmpeg binary for {}: executable SHA-256 mismatch",
                target
            );
            return false;
        }
    }

    match std::process::Command::new(path).arg("-version").output() {
        Ok(output) => {
            if output.status.success() {
                let banner = format!(
                    "{}\n{}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                );
                let normalized = banner.to_ascii_lowercase();
                let forbidden_markers = [
                    "gyan.dev",
                    "--enable-gpl",
                    "--enable-nonfree",
                    "--enable-libx264",
                    "--enable-libx265",
                    "--enable-libxvid",
                    "--enable-libxavs2",
                ];

                if forbidden_markers
                    .iter()
                    .any(|marker| normalized.contains(marker))
                {
                    println!(
                        "cargo:warning=Rejected FFmpeg binary for {}: GPL/nonfree build marker detected",
                        target
                    );
                    return false;
                }

                if !normalized.contains("ffmpeg version ") {
                    println!(
                        "cargo:warning=Rejected FFmpeg binary for {}: missing version banner",
                        target
                    );
                    return false;
                }

                if (target.contains("windows") || target.contains("linux"))
                    && !normalized.contains("configuration:")
                {
                    println!(
                        "cargo:warning=Rejected FFmpeg binary for {}: missing build configuration",
                        target
                    );
                    return false;
                }

                if let Some(version_line) = banner.lines().find(|line| !line.trim().is_empty()) {
                    println!(
                        "cargo:warning=✅ FFmpeg verification passed: {}",
                        version_line
                    );
                }
                true
            } else {
                false
            }
        }
        Err(_) => false,
    }
}
