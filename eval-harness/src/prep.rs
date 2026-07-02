//! `eval-harness prep` — convert eval/raw/<bucket>/* recordings to the
//! canonical eval WAV shape (16 kHz mono s16) using the app's ffmpeg sidecar.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};

const RAW_EXTS: [&str; 4] = ["m4a", "mp3", "wav", "mp4"];

/// Locate the app's bundled ffmpeg sidecar; fall back to `ffmpeg` on PATH.
pub fn ffmpeg_path(repo_root: &Path) -> PathBuf {
    let bin_dir = repo_root
        .join("frontend")
        .join("src-tauri")
        .join("binaries");
    let names: &[&str] = if cfg!(target_os = "windows") {
        &["ffmpeg-x86_64-pc-windows-msvc.exe"]
    } else if cfg!(target_os = "macos") {
        &["ffmpeg-aarch64-apple-darwin", "ffmpeg-x86_64-apple-darwin"]
    } else {
        &["ffmpeg-x86_64-unknown-linux-gnu"]
    };
    for name in names {
        let p = bin_dir.join(name);
        if p.is_file() {
            return p;
        }
    }
    eprintln!(
        "Uyarı: ffmpeg sidecar bulunamadı ({}), PATH'teki 'ffmpeg' denenecek",
        bin_dir.display()
    );
    PathBuf::from("ffmpeg")
}

pub fn run_prep(repo_root: &Path, buckets: &[&str], force: bool) -> Result<()> {
    let eval_dir = repo_root.join("eval");
    let raw_root = eval_dir.join("raw");
    if !raw_root.is_dir() {
        bail!(
            "eval/raw bulunamadı: {} — klipleri eval/raw/<kova>/ altına koyun (bkz. eval/README.md)",
            raw_root.display()
        );
    }
    let ffmpeg = ffmpeg_path(repo_root);

    let mut converted = 0usize;
    let mut skipped = 0usize;
    let mut total_raw = 0usize;
    let mut failures: Vec<String> = Vec::new();

    for bucket in buckets {
        let src_dir = raw_root.join(bucket);
        if !src_dir.is_dir() {
            continue;
        }
        let dst_dir = eval_dir.join(bucket);
        std::fs::create_dir_all(&dst_dir)
            .with_context(|| format!("dizin oluşturulamadı: {}", dst_dir.display()))?;

        let mut sources: Vec<PathBuf> = std::fs::read_dir(&src_dir)
            .with_context(|| format!("dizin okunamadı: {}", src_dir.display()))?
            .filter_map(std::result::Result::ok)
            .map(|e| e.path())
            .filter(|p| {
                p.is_file()
                    && p.extension()
                        .and_then(|s| s.to_str())
                        .is_some_and(|ext| RAW_EXTS.iter().any(|e| ext.eq_ignore_ascii_case(e)))
            })
            .collect();
        sources.sort();

        let mut seen_ids: HashSet<String> = HashSet::new();
        for src in sources {
            total_raw += 1;
            let Some(id) = src.file_stem().and_then(|s| s.to_str()).map(String::from) else {
                failures.push(format!("{}: dosya adı okunamadı", src.display()));
                continue;
            };
            if !seen_ids.insert(id.clone()) {
                eprintln!(
                    "Uyarı: {bucket}/{id} için birden fazla kaynak dosya var — {} atlandı",
                    src.display()
                );
                continue;
            }
            let dst = dst_dir.join(format!("{id}.wav"));
            if dst.is_file() && !force {
                skipped += 1;
                continue;
            }
            println!("prep: {} → {}", src.display(), dst.display());
            let output = Command::new(&ffmpeg)
                .args(["-hide_banner", "-loglevel", "error", "-y", "-i"])
                .arg(&src)
                .args(["-vn", "-ac", "1", "-ar", "16000", "-c:a", "pcm_s16le"])
                .arg(&dst)
                .output()
                .with_context(|| format!("ffmpeg çalıştırılamadı: {}", ffmpeg.display()))?;
            if output.status.success() {
                converted += 1;
            } else {
                let _ = std::fs::remove_file(&dst); // don't leave partial output behind
                failures.push(format!(
                    "{}: {}",
                    src.display(),
                    String::from_utf8_lossy(&output.stderr).trim()
                ));
            }
        }
    }

    println!(
        "prep bitti: {converted} dönüştürüldü, {skipped} atlandı (mevcut; --force ile yenile), {} hata",
        failures.len()
    );
    if total_raw == 0 {
        println!(
            "eval/raw/<kova>/ altında klip yok. Kayıt talimatı: eval/README.md \
             (kova başına ≥5 klip, 2-10 dk; m4a/mp3/wav/mp4)"
        );
    }
    if !failures.is_empty() {
        bail!(
            "{} dönüşüm başarısız:\n{}",
            failures.len(),
            failures.join("\n")
        );
    }
    Ok(())
}
