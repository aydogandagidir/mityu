//! Running the `diarize-helper` sidecar and reading what it says.
//!
//! One invocation per meeting, then the process exits — ADR-0034 makes
//! diarization a whole-file pass over a finished recording, so there is no
//! keep-alive to manage and nothing to leak if it crashes.
//!
//! Binary resolution mirrors `summary::summary_engine::sidecar`: an environment
//! override for development, then next to our own executable with the Tauri
//! target-triple suffix, which is how `externalBin` lays sidecars down.

use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use serde::Deserialize;

/// What the helper prints on stdout with `--format json`.
///
/// Deserialised into our own type rather than shared with the helper crate: the
/// helper is a separate process with its own release cycle, so this is a wire
/// contract, and a shared struct would hide the day the two drift apart.
#[derive(Debug, Deserialize)]
struct HelperOutput {
    speakers: i32,
    turns: Vec<HelperTurn>,
}

#[derive(Debug, Deserialize)]
struct HelperTurn {
    start_ms: i64,
    end_ms: i64,
    speaker_label: String,
    confidence: Option<f64>,
}

/// One completed diarization pass.
#[derive(Debug, Clone, PartialEq)]
pub struct DiarizationOutcome {
    pub speakers: i32,
    pub turns: Vec<crate::database::repositories::speaker_turn::SpeakerTurn>,
}

fn target_triple() -> &'static str {
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    {
        "x86_64-pc-windows-msvc"
    }
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        "x86_64-unknown-linux-gnu"
    }
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    {
        "aarch64-unknown-linux-gnu"
    }
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    {
        "x86_64-apple-darwin"
    }
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        "aarch64-apple-darwin"
    }
}

/// Locate the sidecar, or say clearly that it is absent.
///
/// Absence is a legitimate state — a build that has not produced the helper yet
/// — so the error names both places that were searched rather than reporting a
/// generic failure the user cannot act on.
pub fn resolve_binary() -> Result<PathBuf> {
    if let Ok(env_path) = std::env::var("MITYU_DIARIZE_HELPER") {
        if !env_path.is_empty() {
            let path = PathBuf::from(env_path);
            if path.is_file() {
                return Ok(path);
            }
            bail!("MITYU_DIARIZE_HELPER points at a file that does not exist: {path:?}");
        }
    }

    let exe = std::env::current_exe().context("locate our own executable")?;
    let dir = exe
        .parent()
        .ok_or_else(|| anyhow!("executable has no parent directory"))?;
    let suffix = if cfg!(windows) { ".exe" } else { "" };
    let candidates = [
        dir.join(format!("diarize-helper-{}{suffix}", target_triple())),
        dir.join(format!("diarize-helper{suffix}")),
    ];
    for candidate in &candidates {
        if candidate.is_file() {
            return Ok(candidate.clone());
        }
    }
    bail!(
        "diarize-helper not found. Looked at {} and {}. Set MITYU_DIARIZE_HELPER for a \
         development build.",
        candidates[0].display(),
        candidates[1].display()
    )
}

/// Parse the helper's stdout.
///
/// Split out so the wire contract can be tested without running the binary —
/// which is the half most likely to break silently when either side changes.
pub fn parse_output(stdout: &str) -> Result<DiarizationOutcome> {
    let parsed: HelperOutput =
        serde_json::from_str(stdout).context("diarize-helper produced output we cannot read")?;

    let mut turns = Vec::with_capacity(parsed.turns.len());
    for t in parsed.turns {
        // Re-checked on this side of the process boundary. The helper already
        // refuses to emit these, but "the other process validated it" is not a
        // guarantee once the two ship separately, and a nonsense turn would be
        // stored and then shown as fact.
        if t.end_ms <= t.start_ms {
            bail!(
                "diarize-helper returned a turn that ends before it starts ({} -> {})",
                t.start_ms,
                t.end_ms
            );
        }
        if t.speaker_label.trim().is_empty() {
            bail!("diarize-helper returned a turn with no speaker label");
        }
        turns.push(crate::database::repositories::speaker_turn::SpeakerTurn {
            start_ms: t.start_ms,
            end_ms: t.end_ms,
            speaker_label: t.speaker_label,
            confidence: t.confidence,
        });
    }
    Ok(DiarizationOutcome {
        speakers: parsed.speakers,
        turns,
    })
}

/// Run the sidecar over one prepared WAV.
pub async fn run(
    binary: &Path,
    wav: &Path,
    segmentation: &Path,
    embedding: &Path,
) -> Result<DiarizationOutcome> {
    let mut command = tokio::process::Command::new(binary);
    command
        .arg("--audio")
        .arg(wav)
        .arg("--segmentation")
        .arg(segmentation)
        .arg("--embedding")
        .arg(embedding)
        .arg("--format")
        .arg("json");
    #[cfg(windows)]
    {
        // No console window for a background pass. `creation_flags` is tokio's
        // own method here, so the std `CommandExt` trait is not needed.
        command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }

    let output = command
        .output()
        .await
        .with_context(|| format!("run {}", binary.display()))?;

    if !output.status.success() {
        // The helper writes its reason to stderr (unreadable audio, missing
        // model, wrong sample rate). Surfacing it beats "exited with code 1".
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "diarize-helper failed ({}): {}",
            output.status,
            stderr.trim()
        );
    }
    parse_output(&String::from_utf8_lossy(&output.stdout))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_helpers_documented_output() {
        let out = parse_output(
            r#"{"speakers":2,"turns":[
                 {"start_ms":1583,"end_ms":3406,"speaker_label":"Speaker 1","confidence":null},
                 {"start_ms":9346,"end_ms":11472,"speaker_label":"Speaker 2","confidence":null}]}"#,
        )
        .expect("parsed");
        assert_eq!(out.speakers, 2);
        assert_eq!(out.turns.len(), 2);
        assert_eq!(out.turns[0].start_ms, 1583);
        assert_eq!(out.turns[1].speaker_label, "Speaker 2");
        assert!(out.turns.iter().all(|t| t.confidence.is_none()));
    }

    /// "Ran and separated nothing" is a real outcome and must parse, because the
    /// caller still stamps `diarized_at` for it — that stamp is the only thing
    /// distinguishing it from "never ran".
    #[test]
    fn an_empty_result_is_valid_not_an_error() {
        let out = parse_output(r#"{"speakers":0,"turns":[]}"#).expect("parsed");
        assert_eq!(out.speakers, 0);
        assert!(out.turns.is_empty());
    }

    /// The other process validates these too. Trusting that is how a contract
    /// drifts silently once the two ship on different cycles.
    #[test]
    fn nonsense_turns_are_refused_on_this_side_of_the_boundary() {
        assert!(parse_output(
            r#"{"speakers":1,"turns":[{"start_ms":500,"end_ms":100,"speaker_label":"Speaker 1","confidence":null}]}"#
        )
        .is_err());
        assert!(parse_output(
            r#"{"speakers":1,"turns":[{"start_ms":0,"end_ms":100,"speaker_label":"  ","confidence":null}]}"#
        )
        .is_err());
    }

    #[test]
    fn unreadable_output_is_an_error_not_an_empty_result() {
        assert!(parse_output("").is_err());
        assert!(parse_output("not json at all").is_err());
        // A silently-empty parse here would look exactly like "no speakers".
        assert!(parse_output(r#"{"speakers":2}"#).is_err());
    }

    #[test]
    fn a_bad_env_override_is_reported_rather_than_ignored() {
        std::env::set_var(
            "MITYU_DIARIZE_HELPER",
            "/definitely/not/here/diarize-helper",
        );
        let err = resolve_binary().expect_err("must refuse");
        assert!(err.to_string().contains("does not exist"), "{err}");
        std::env::remove_var("MITYU_DIARIZE_HELPER");
    }
}

/// Guards for the sidecar's third-party notice.
///
/// The notice makes legal claims about a binary built from a pinned archive.
/// Every claim in it can go stale silently -- a pin bump, a rename, a resource
/// list that forgets to ship the file -- and a stale legal notice is worse than
/// none, because it asserts something untrue with apparent authority.
#[cfg(test)]
mod notice {
    const NOTICE: &str = include_str!("../../resources/SIDECAR-NOTICES.txt");
    const PINS: &str = include_str!("../../../../tools/diarization/fetch-sherpa-archive.py");
    const TAURI_CONF: &str = include_str!("../../tauri.conf.json");

    /// The archive hash the notice quotes must be the one actually fetched.
    ///
    /// This is the drift that matters most: bump the pin, forget the notice,
    /// and Mityu ships a document attesting to the provenance of a binary it
    /// no longer builds.
    #[test]
    fn the_notice_quotes_the_archive_that_is_actually_pinned() {
        let sha = NOTICE
            .lines()
            .map(str::trim)
            .find(|l| l.len() == 64 && l.chars().all(|c| c.is_ascii_hexdigit()))
            .expect("SIDECAR-NOTICES.txt quotes no 64-hex archive digest");
        assert!(
            PINS.contains(sha),
            "SIDECAR-NOTICES.txt quotes archive sha256 {sha}, which no longer \
             appears in fetch-sherpa-archive.py -- the notice describes a build \
             we do not make"
        );
    }

    /// ...and the archive filename, so a same-hash rename is caught too.
    #[test]
    fn the_notice_names_the_archive_that_is_actually_pinned() {
        let archive = "sherpa-onnx-v1.13.4-win-x64-static-MT-Release-no-tts-lib.tar.bz2";
        assert!(
            NOTICE.contains(archive),
            "the notice does not name {archive}"
        );
        // The pin file builds the name from an f-string, so match the distinctive tail.
        assert!(
            PINS.contains("win-x64-static-MT-Release-no-tts-lib.tar.bz2"),
            "fetch-sherpa-archive.py no longer pins the no-tts Windows archive"
        );
    }

    /// A notice that is not bundled is not a notice.
    ///
    /// Apache-2.0 4(a) requires giving recipients a copy of the licence, and
    /// MIT/BSD require reproducing their text in binary distributions. Writing
    /// the file discharges nothing if `tauri.conf.json` does not ship it.
    #[test]
    fn the_notice_and_the_licence_text_are_actually_shipped() {
        for resource in [
            "resources/SIDECAR-NOTICES.txt",
            "resources/COPYING.Apache-2.0.txt",
        ] {
            assert!(
                TAURI_CONF.contains(resource),
                "{resource} exists but tauri.conf.json does not bundle it, so no \
                 user ever receives it"
            );
        }
    }

    /// Every permissive licence the binary carries has to appear by name, and
    /// the GPL claim has to stay attached to the reason it is true.
    #[test]
    fn every_linked_licence_is_named() {
        for licence in ["Apache-2.0", "MIT", "BSD-3-Clause"] {
            assert!(
                NOTICE.contains(licence),
                "the notice never mentions {licence}"
            );
        }
        // Each component, verified from its own upstream repository.
        for component in [
            "sherpa-onnx",
            "kaldi-decoder",
            "kaldifst",
            "OpenFST",
            "kaldi-native-fbank",
            "simple-sentencepiece",
            "ONNX Runtime",
            "KISS FFT",
        ] {
            assert!(
                NOTICE.contains(component),
                "the notice omits {component}, which is statically linked in"
            );
        }
        // The no-GPL claim is only true because of the no-tts build; keeping the
        // two together stops the claim outliving its justification.
        assert!(NOTICE.contains("no-tts"));
        assert!(NOTICE.contains("GPL-3.0"));
    }
}
