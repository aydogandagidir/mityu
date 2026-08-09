//! Trusted manifest, verification and acquisition for the diarization models.
//!
//! ADR-0034 requires these to be pinned and integrity-checked "like Parakeet",
//! and this follows that module's shape deliberately: a `(exact byte length,
//! SHA-256)` manifest, verification through `utils::verify_file_integrity`, and
//! an artifact **absent from the manifest is an error, never a warning**
//! (`parakeet_engine.rs:383` uses the same wording for the same reason).
//!
//! ## What is pinned, and why the digest is the pin
//!
//! Parakeet pins a Hugging Face git revision. These come from GitHub *release*
//! assets, and a release tag can be moved to point at different bytes — so the
//! tag is only an address and the SHA-256 is the actual pin. Both artifacts are
//! checked by exact size and digest before anything uses them.
//!
//! ## The archive, and why its contents are pinned separately
//!
//! The segmentation model ships inside a `.tar.bz2`. Verifying the archive
//! proves what was downloaded; it says nothing about what is on disk a month
//! later. So the extracted files carry their own pins and are re-verified on
//! every status check, exactly as `validate_model_directory` does for Parakeet.
//!
//! `bzip2` and `tar` cost nothing new here: `sherpa-onnx-sys` already pulls both
//! into the lock file, so this adds no crate to the dependency graph.
//!
//! ## Licences travel with the model
//!
//! The segmentation archive ships the upstream MIT licence with its original
//! `Copyright (c) 2022 CNRS` line, and MIT requires that notice to accompany
//! copies — so it is extracted and kept beside the model rather than discarded.
//! It is not separately pinned because it arrives inside the verified archive.
//! CAM++ is published as a bare `.onnx` with no licence file, so its Apache-2.0
//! attribution is ours to supply at ship time (BACKLOG H7).

use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};

/// One file this feature expects to find on disk, and what it must be.
#[derive(Debug, Clone, Copy)]
pub struct ArtifactPin {
    pub filename: &'static str,
    pub size: u64,
    pub sha256: &'static str,
}

/// Where the bytes come from, and what to take out of them.
struct Source {
    url: &'static str,
    /// Pin on the downloaded bytes, checked before anything is extracted or
    /// moved into place.
    size: u64,
    sha256: &'static str,
    /// `(path inside the archive, name on disk)`. Empty means the download IS
    /// the artifact and is stored under `on_disk`.
    members: &'static [(&'static str, &'static str)],
    /// Name on disk when the download is not an archive.
    on_disk: Option<&'static str>,
}

const SEGMENTATION_ONNX: &str = "segmentation.onnx";
const EMBEDDING_ONNX: &str = "embedding.onnx";
/// The upstream MIT licence that ships inside the segmentation archive.
pub const SEGMENTATION_LICENSE: &str = "segmentation.LICENSE";

/// Every file that must be present and correct for diarization to run.
///
/// The licence is deliberately absent: it is a notice, not an input, and it
/// arrives inside the archive this manifest already verifies.
const ON_DISK: &[ArtifactPin] = &[
    ArtifactPin {
        // pyannote segmentation-3.0, MIT (Copyright (c) 2022 CNRS), as
        // republished by sherpa-onnx. Verified 2026-08-09.
        filename: SEGMENTATION_ONNX,
        size: 5_992_913,
        sha256: "220ad67ca923bef2fa91f2390c786097bf305bceb5e261d4af67b38e938e1079",
    },
    ArtifactPin {
        // 3D-Speaker CAM++ (zh-cn common), Apache-2.0. Chosen over NeMo TitaNet
        // on licence grounds (no attribution-in-product obligation) and size
        // (28 MB vs 101 MB) — ADR-0034.
        filename: EMBEDDING_ONNX,
        size: 28_281_138,
        sha256: "f682b514c05d947ee3fa91cd6ec6c5c7543479a128373fa29b1faedccd21fd11",
    },
];

const SOURCES: &[Source] = &[
    Source {
        url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/speaker-segmentation-models/sherpa-onnx-pyannote-segmentation-3-0.tar.bz2",
        size: 6_958_444,
        sha256: "24615ee884c897d9d2ba09bb4d30da6bb1b15e685065962db5b02e76e4996488",
        members: &[
            ("sherpa-onnx-pyannote-segmentation-3-0/model.onnx", SEGMENTATION_ONNX),
            ("sherpa-onnx-pyannote-segmentation-3-0/LICENSE", SEGMENTATION_LICENSE),
        ],
        on_disk: None,
    },
    Source {
        url: "https://github.com/k2-fsa/sherpa-onnx/releases/download/speaker-recongition-models/3dspeaker_speech_campplus_sv_zh-cn_16k-common.onnx",
        size: 28_281_138,
        sha256: "f682b514c05d947ee3fa91cd6ec6c5c7543479a128373fa29b1faedccd21fd11",
        members: &[],
        on_disk: Some(EMBEDDING_ONNX),
    },
];

/// Resolved, verified paths the sidecar is pointed at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelPaths {
    pub segmentation: PathBuf,
    pub embedding: PathBuf,
}

/// What the models directory currently holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelStatus {
    /// Every artifact present and matching the manifest.
    Available(ModelPaths),
    /// Nothing downloaded yet.
    Missing,
    /// Present but wrong. Named so the UI can say which file and why, rather
    /// than "something went wrong".
    Corrupted { filename: String, reason: String },
}

/// `<app data>/models/diarization`, matching where Parakeet's models live.
pub fn models_dir(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("models").join("diarization")
}

fn pin(filename: &str) -> Result<&'static ArtifactPin> {
    ON_DISK
        .iter()
        .find(|p| p.filename == filename)
        .ok_or_else(|| anyhow!("{filename} is absent from the trusted manifest"))
}

/// Verify every file in `pins` inside `dir`.
///
/// Split out from [`status`] so it can be tested with small synthetic pins: the
/// real artifacts are 34 MB and downloading them in a unit test would make the
/// test suite depend on the network.
pub async fn verify_all(dir: &Path, pins: &[ArtifactPin]) -> ModelStatus {
    let mut any_present = false;
    for p in pins {
        let path = dir.join(p.filename);
        if !path.exists() {
            continue;
        }
        any_present = true;
        if let Err(e) = crate::utils::verify_file_integrity(&path, p.size, p.sha256).await {
            return ModelStatus::Corrupted {
                filename: p.filename.to_string(),
                reason: format!("{e:#}"),
            };
        }
    }
    if !any_present {
        return ModelStatus::Missing;
    }
    // Everything present verified; anything missing keeps this incomplete.
    for p in pins {
        if !dir.join(p.filename).exists() {
            return ModelStatus::Corrupted {
                filename: p.filename.to_string(),
                reason: "expected model file is missing from a partly-populated directory"
                    .to_string(),
            };
        }
    }
    ModelStatus::Available(ModelPaths {
        segmentation: dir.join(SEGMENTATION_ONNX),
        embedding: dir.join(EMBEDDING_ONNX),
    })
}

/// Current status of the real manifest.
///
/// Re-verifies on every call rather than trusting a previous success: a model
/// file can be truncated, replaced or partly written between runs, and a wrong
/// model would not fail loudly — it would produce plausible, wrong speakers.
pub async fn status(dir: &Path) -> ModelStatus {
    verify_all(dir, ON_DISK).await
}

/// Download whatever is missing, verify it, and return the resolved paths.
///
/// `progress` is called with `(downloaded_bytes, total_bytes)` per source, so a
/// caller can show something during a 34 MB fetch. Nothing is moved into place
/// until it has been verified.
pub async fn ensure<F>(dir: &Path, mut progress: F) -> Result<ModelPaths>
where
    F: FnMut(&str, u64, u64),
{
    if let ModelStatus::Available(paths) = status(dir).await {
        return Ok(paths);
    }
    tokio::fs::create_dir_all(dir)
        .await
        .with_context(|| format!("create diarization model directory: {}", dir.display()))?;

    for source in SOURCES {
        if source_satisfied(dir, source).await {
            continue;
        }
        acquire(dir, source, &mut progress).await?;
    }

    match status(dir).await {
        ModelStatus::Available(paths) => Ok(paths),
        ModelStatus::Corrupted { filename, reason } => {
            bail!("diarization model {filename} failed verification after download: {reason}")
        }
        ModelStatus::Missing => {
            bail!("diarization models are still missing after a download that reported success")
        }
    }
}

async fn source_satisfied(dir: &Path, source: &Source) -> bool {
    let names: Vec<&str> = match source.on_disk {
        Some(name) => vec![name],
        None => source.members.iter().map(|(_, name)| *name).collect(),
    };
    for name in names {
        // The licence is not in the manifest, so presence is all we can check.
        let Ok(p) = pin(name) else {
            if !dir.join(name).exists() {
                return false;
            }
            continue;
        };
        let path = dir.join(p.filename);
        if !path.exists()
            || crate::utils::verify_file_integrity(&path, p.size, p.sha256)
                .await
                .is_err()
        {
            return false;
        }
    }
    true
}

async fn acquire<F>(dir: &Path, source: &Source, progress: &mut F) -> Result<()>
where
    F: FnMut(&str, u64, u64),
{
    let label = source.url.rsplit('/').next().unwrap_or("model");
    let staged = dir.join(format!(".{label}.part"));

    download(source.url, &staged, source.size, label, progress).await?;
    // Verified BEFORE it is unpacked or moved into place: an unverified archive
    // must never be handed to an extractor, and an unverified model must never
    // reach the engine.
    crate::utils::verify_file_integrity(&staged, source.size, source.sha256)
        .await
        .with_context(|| format!("verify downloaded {label}"))?;

    match source.on_disk {
        Some(name) => {
            tokio::fs::rename(&staged, dir.join(name))
                .await
                .with_context(|| format!("install {name}"))?;
        }
        None => {
            let staged_for_blocking = staged.clone();
            let dir_for_blocking = dir.to_path_buf();
            let members: Vec<(String, String)> = source
                .members
                .iter()
                .map(|(a, b)| ((*a).to_string(), (*b).to_string()))
                .collect();
            tokio::task::spawn_blocking(move || {
                extract_members(&staged_for_blocking, &dir_for_blocking, &members)
            })
            .await
            .context("extraction task panicked")??;
            let _ = tokio::fs::remove_file(&staged).await;
        }
    }
    Ok(())
}

/// Extract exactly the named members, and fail if any is absent.
///
/// Deliberately not a blanket unpack: the archive also carries Python scripts we
/// have no use for, and unpacking whatever an archive happens to contain is how
/// a path-traversal entry gets written outside the directory.
fn extract_members(archive: &Path, dir: &Path, members: &[(String, String)]) -> Result<()> {
    use std::io::Read;

    let file = std::fs::File::open(archive)
        .with_context(|| format!("open archive {}", archive.display()))?;
    let mut tar = tar::Archive::new(bzip2::read::BzDecoder::new(file));

    let mut found = vec![false; members.len()];
    for entry in tar.entries().context("read archive entries")? {
        let mut entry = entry.context("read archive entry")?;
        let path = entry.path().context("archive entry path")?.to_path_buf();
        let name = path.to_string_lossy().replace('\\', "/");
        let Some(idx) = members.iter().position(|(inside, _)| *inside == name) else {
            continue;
        };
        let mut bytes = Vec::new();
        entry
            .read_to_end(&mut bytes)
            .context("read archive member")?;
        // Written under a name WE choose, so a crafted entry path cannot escape
        // the directory.
        std::fs::write(dir.join(&members[idx].1), &bytes)
            .with_context(|| format!("write {}", members[idx].1))?;
        found[idx] = true;
    }

    for (i, ok) in found.iter().enumerate() {
        if !ok {
            bail!(
                "archive {} does not contain the expected member {}",
                archive.display(),
                members[i].0
            );
        }
    }
    Ok(())
}

async fn download<F>(
    url: &str,
    dest: &Path,
    expected_size: u64,
    label: &str,
    progress: &mut F,
) -> Result<()>
where
    F: FnMut(&str, u64, u64),
{
    use futures_util::StreamExt;
    use tokio::io::AsyncWriteExt;

    let response = reqwest::Client::new()
        .get(url)
        .send()
        .await
        .with_context(|| format!("request {url}"))?;
    if !response.status().is_success() {
        bail!("download {url} failed with HTTP {}", response.status());
    }

    let mut file = tokio::fs::File::create(dest)
        .await
        .with_context(|| format!("create {}", dest.display()))?;
    let mut stream = response.bytes_stream();
    let mut written: u64 = 0;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.with_context(|| format!("read body of {url}"))?;
        file.write_all(&chunk).await.context("write model bytes")?;
        written += chunk.len() as u64;
        progress(label, written, expected_size);
    }
    file.flush().await.context("flush model file")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    fn write(dir: &Path, name: &str, bytes: &[u8]) -> ArtifactPin {
        std::fs::write(dir.join(name), bytes).expect("write fixture");
        let digest = format!("{:x}", Sha256::digest(bytes));
        // Leaked so the pin can be `&'static`, which only happens in tests.
        ArtifactPin {
            filename: Box::leak(name.to_string().into_boxed_str()),
            size: bytes.len() as u64,
            sha256: Box::leak(digest.into_boxed_str()),
        }
    }

    #[tokio::test]
    async fn an_empty_directory_is_missing_not_corrupted() {
        let dir = tempfile::tempdir().expect("tempdir");
        let pins = [ArtifactPin {
            filename: "a.onnx",
            size: 3,
            sha256: "0000000000000000000000000000000000000000000000000000000000000000",
        }];
        assert_eq!(verify_all(dir.path(), &pins).await, ModelStatus::Missing);
    }

    #[tokio::test]
    async fn matching_files_resolve_to_paths() {
        let dir = tempfile::tempdir().expect("tempdir");
        let a = write(dir.path(), SEGMENTATION_ONNX, b"segmentation bytes");
        let b = write(dir.path(), EMBEDDING_ONNX, b"embedding bytes");
        match verify_all(dir.path(), &[a, b]).await {
            ModelStatus::Available(paths) => {
                assert_eq!(paths.segmentation, dir.path().join(SEGMENTATION_ONNX));
                assert_eq!(paths.embedding, dir.path().join(EMBEDDING_ONNX));
            }
            other => panic!("expected Available, got {other:?}"),
        }
    }

    /// The failure mode that matters: a wrong model does not crash, it produces
    /// plausible and wrong speakers. So it must be caught before it is used.
    #[tokio::test]
    async fn a_single_flipped_byte_is_reported_as_corrupted() {
        let dir = tempfile::tempdir().expect("tempdir");
        let a = write(dir.path(), SEGMENTATION_ONNX, b"segmentation bytes");
        let b = write(dir.path(), EMBEDDING_ONNX, b"embedding bytes");
        // Same length, different content.
        std::fs::write(dir.path().join(EMBEDDING_ONNX), b"embeddinh bytes").expect("corrupt");
        match verify_all(dir.path(), &[a, b]).await {
            ModelStatus::Corrupted { filename, .. } => assert_eq!(filename, EMBEDDING_ONNX),
            other => panic!("expected Corrupted, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn a_half_populated_directory_is_not_reported_available() {
        let dir = tempfile::tempdir().expect("tempdir");
        let a = write(dir.path(), SEGMENTATION_ONNX, b"segmentation bytes");
        let b = ArtifactPin {
            filename: EMBEDDING_ONNX,
            size: 5,
            sha256: "0000000000000000000000000000000000000000000000000000000000000000",
        };
        match verify_all(dir.path(), &[a, b]).await {
            ModelStatus::Corrupted { filename, reason } => {
                assert_eq!(filename, EMBEDDING_ONNX);
                assert!(reason.contains("missing"), "{reason}");
            }
            other => panic!("expected Corrupted, got {other:?}"),
        }
    }

    /// Mirrors Parakeet's rule: an artifact nobody pinned must be an error, not
    /// a silent pass.
    #[test]
    fn an_unpinned_artifact_is_an_error() {
        assert!(pin("something-nobody-pinned.onnx").is_err());
        assert!(pin(SEGMENTATION_ONNX).is_ok());
        assert!(pin(EMBEDDING_ONNX).is_ok());
    }

    /// The real acquisition, end to end. `#[ignore]` because it downloads 34 MB:
    /// a test suite that needs the network is a test suite that fails for
    /// reasons unrelated to the code. Run it deliberately when the pins change:
    ///
    /// ```text
    /// cargo test -p mityu --lib diarization -- --ignored --nocapture
    /// ```
    ///
    /// It is the only thing that proves the pins describe the bytes GitHub
    /// actually serves — a manifest can be internally consistent and still
    /// wrong.
    #[tokio::test]
    #[ignore = "downloads 34 MB from GitHub"]
    async fn real_models_download_verify_and_extract() {
        let dir = tempfile::tempdir().expect("tempdir");
        let paths = ensure(dir.path(), |label, done, total| {
            if total > 0 && done == total {
                println!("  fetched {label} ({done} bytes)");
            }
        })
        .await
        .expect("acquire models");

        assert!(paths.segmentation.is_file());
        assert!(paths.embedding.is_file());
        assert_eq!(status(dir.path()).await, ModelStatus::Available(paths));

        // MIT requires the notice to travel with the copy, so the archive's
        // licence must survive extraction.
        let licence = std::fs::read_to_string(dir.path().join(SEGMENTATION_LICENSE))
            .expect("segmentation licence extracted");
        assert!(licence.contains("MIT License"), "{licence}");
        assert!(
            licence.contains("CNRS"),
            "upstream copyright line preserved"
        );

        // A second call must be a no-op, not a re-download.
        let again = ensure(dir.path(), |_, _, _| {
            panic!("re-downloaded an already-valid model")
        })
        .await
        .expect("second call is a no-op");
        assert!(again.segmentation.is_file());
    }

    #[test]
    fn every_manifest_entry_is_a_well_formed_pin() {
        for p in ON_DISK {
            assert_eq!(p.sha256.len(), 64, "{} digest length", p.filename);
            assert!(
                p.sha256.bytes().all(|b| b.is_ascii_hexdigit()),
                "{} digest is not hex",
                p.filename
            );
            assert!(p.size > 0, "{} has no byte length", p.filename);
        }
        for s in SOURCES {
            assert_eq!(s.sha256.len(), 64);
            assert!(s.size > 0);
            assert!(s.url.starts_with("https://"), "model source must be https");
            // Exactly one of the two: a source is EITHER an archive we take
            // members out of, OR a single file stored under `on_disk`. Both set
            // would be ambiguous; neither would produce nothing.
            assert!(
                s.on_disk.is_some() ^ !s.members.is_empty(),
                "source {} must be either an archive with members or a single file",
                s.url
            );
        }
    }
}
