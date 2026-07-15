use log::{debug, error};
use once_cell::sync::Lazy;
use std::path::PathBuf;
use which::which;

#[cfg(not(windows))]
const EXECUTABLE_NAME: &str = "ffmpeg";

#[cfg(windows)]
const EXECUTABLE_NAME: &str = "ffmpeg.exe";

static FFMPEG_PATH: Lazy<Option<PathBuf>> = Lazy::new(find_ffmpeg_path_internal);

pub fn find_ffmpeg_path() -> Option<PathBuf> {
    FFMPEG_PATH.as_ref().map(|p| p.clone())
}

fn find_ffmpeg_path_internal() -> Option<PathBuf> {
    debug!("Starting search for ffmpeg executable");

    // ============================================================
    // PRIORITY 1: Bundled Binary (Production)
    // ============================================================
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_folder) = exe_path.parent() {
            let bundled = exe_folder.join(EXECUTABLE_NAME);
            if bundled.exists() && bundled.is_file() {
                debug!("Found bundled ffmpeg: {:?}", bundled);
                return Some(bundled);
            }

            #[cfg(target_os = "macos")]
            {
                let bundled = exe_folder.join("../Resources").join(EXECUTABLE_NAME);
                if bundled.exists() && bundled.is_file() {
                    debug!("Found bundled ffmpeg resource: {:?}", bundled);
                    return Some(bundled);
                }
            }

            #[cfg(target_os = "linux")]
            {
                let bundled = exe_folder.join("lib").join(EXECUTABLE_NAME);
                if bundled.exists() && bundled.is_file() {
                    debug!("Found bundled ffmpeg library: {:?}", bundled);
                    return Some(bundled);
                }
            }
        }
    }

    // Release builds must use the binary whose archive digest and build flags
    // were verified at build time. Falling back to PATH or the working directory
    // would let an unrelated GPL/nonfree binary silently replace it.
    if !cfg!(debug_assertions) {
        error!(
            "bundled ffmpeg executable not found; reinstall Mityu to restore the verified offline binary"
        );
        return None;
    }

    // ============================================================
    // PRIORITY 2: Fallback to Existing Logic
    // ============================================================

    // Check if `ffmpeg` is in the PATH environment variable
    if let Ok(path) = which(EXECUTABLE_NAME) {
        debug!("Found ffmpeg in PATH: {:?}", path);
        return Some(path);
    }
    debug!("ffmpeg not found in PATH");

    // Check in $HOME/.local/bin on macOS
    #[cfg(target_os = "macos")]
    {
        if let Ok(home) = std::env::var("HOME") {
            let local_bin = PathBuf::from(home).join(".local").join("bin");
            debug!("Checking $HOME/.local/bin: {:?}", local_bin);
            let ffmpeg_in_local_bin = local_bin.join(EXECUTABLE_NAME);
            if ffmpeg_in_local_bin.exists() {
                debug!(
                    "Found ffmpeg in $HOME/.local/bin: {:?}",
                    ffmpeg_in_local_bin
                );
                return Some(ffmpeg_in_local_bin);
            }
            debug!("ffmpeg not found in $HOME/.local/bin");
        }
    }

    // Check in current working directory
    if let Ok(cwd) = std::env::current_dir() {
        debug!("Current working directory: {:?}", cwd);
        let ffmpeg_in_cwd = cwd.join(EXECUTABLE_NAME);
        if ffmpeg_in_cwd.is_file() && ffmpeg_in_cwd.exists() {
            debug!(
                "Found ffmpeg in current working directory: {:?}",
                ffmpeg_in_cwd
            );
            return Some(ffmpeg_in_cwd);
        }
        debug!("ffmpeg not found in current working directory");
    }

    // Production bundles FFmpeg at build time. Never download an executable at
    // runtime: that would break offline guarantees and bypass the pinned archive
    // digest and license-policy checks in build/ffmpeg.rs. PATH/local fallbacks
    // above are limited to debug builds.
    error!("ffmpeg executable not found; reinstall Mityu to restore the bundled offline binary");
    None
}
