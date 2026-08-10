//! Orchestration: is this meeting diarizable, and if so, do it.
//!
//! Nothing here runs during capture. ADR-0034 makes this a post-hoc pass over a
//! finished recording, so it can fail without touching a recording in progress
//! (`CLAUDE.md` §4).

use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use sqlx::SqlitePool;

use crate::context::AuthContext;
use crate::database::repositories::speaker_turn::{SpeakerTurn, SpeakerTurnsRepository};
use crate::diarization::models;

/// Whether this meeting can be diarized, and what has already happened.
///
/// Four states, because collapsing any two of them makes the UI lie. In
/// particular an empty result and a pass that never ran are different facts:
/// the first is an answer, the second is an offer.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase", tag = "status")]
pub enum Availability {
    /// No saved audio, so a pass can never run for this meeting. Offering a
    /// button here would be offering something that cannot work.
    NoAudio,
    /// Audio present, models present, never run.
    Ready,
    /// Models are not on disk yet.
    ModelsMissing,
    /// A pass completed. `turns` may be zero, which means it separated nothing.
    Done { diarized_at: String, turns: usize },
}

/// Where a meeting's recording lives, if it kept one.
///
/// Saved audio is OPTIONAL: `RecordingSaver::start_accumulation(false)` discards
/// every chunk, so "transcripts only" is a supported mode that leaves no file.
/// ADR-0027's retention goal also means audio can disappear later, which is why
/// this is probed rather than stored — a flag would go stale on deletion.
pub fn meeting_audio(folder_path: Option<&str>) -> Option<PathBuf> {
    let folder = PathBuf::from(folder_path?);
    if !folder.is_dir() {
        return None;
    }
    crate::audio::retranscription::find_audio_file(&folder).ok()
}

/// Report the four states for one meeting.
pub async fn availability(
    pool: &SqlitePool,
    ctx: &AuthContext,
    meeting_id: &str,
    folder_path: Option<&str>,
    models_dir: &Path,
) -> Result<Availability> {
    if let Some(at) = SpeakerTurnsRepository::diarized_at(pool, ctx, meeting_id).await? {
        let turns = SpeakerTurnsRepository::list_for_meeting(pool, ctx, meeting_id)
            .await?
            .len();
        return Ok(Availability::Done {
            diarized_at: at,
            turns,
        });
    }
    if meeting_audio(folder_path).is_none() {
        return Ok(Availability::NoAudio);
    }
    match models::status(models_dir).await {
        models::ModelStatus::Available(_) => Ok(Availability::Ready),
        // Corrupted is reported as missing on purpose: from the caller's side
        // the remedy is identical (acquire them), and the detail belongs in the
        // log rather than in a status the UI switches on.
        _ => Ok(Availability::ModelsMissing),
    }
}

/// Decode the meeting's audio to what the sidecar accepts and write it beside
/// the caller's temp directory.
///
/// Uses the app's own decoder — the same path retranscription takes — rather
/// than shelling out: it already produces 16 kHz mono with a proper sinc
/// resampler, and the helper refuses anything else instead of resampling.
pub fn prepare_wav(audio: &Path, dest: &Path) -> Result<f64> {
    let decoded = crate::audio::decoder::decode_audio_file(audio)
        .with_context(|| format!("decode {}", audio.display()))?;
    let samples = decoded.to_whisper_format();
    if samples.is_empty() {
        bail!("{} decoded to no audio", audio.display());
    }

    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 16_000,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::create(dest, spec)
        .with_context(|| format!("create {}", dest.display()))?;
    for s in &samples {
        writer.write_sample(*s)?;
    }
    writer.finalize().context("finalize prepared wav")?;
    Ok(samples.len() as f64 / 16_000.0)
}

/// Run one diarization pass and persist it.
///
/// Persists even when the pass separated nothing: the stamp is what
/// distinguishes "ran, inconclusive" from "never ran", and without it the UI
/// would keep offering a pass that has already been tried.
pub async fn diarize_meeting(
    pool: &SqlitePool,
    ctx: &AuthContext,
    meeting_id: &str,
    folder_path: Option<&str>,
    models_dir: &Path,
) -> Result<usize> {
    let audio = meeting_audio(folder_path).ok_or_else(|| {
        anyhow!("this meeting has no saved audio, so speakers cannot be identified")
    })?;
    let paths = match models::status(models_dir).await {
        models::ModelStatus::Available(p) => p,
        models::ModelStatus::Missing => bail!("diarization models are not downloaded yet"),
        models::ModelStatus::Corrupted { filename, reason } => {
            bail!("diarization model {filename} failed verification: {reason}")
        }
    };
    let binary = crate::diarization::sidecar::resolve_binary()?;

    let temp = tempfile::Builder::new()
        .prefix("mityu-diarize-")
        .suffix(".wav")
        .tempfile()
        .context("create temporary audio file")?;
    let wav_path = temp.path().to_path_buf();

    let audio_for_blocking = audio.clone();
    let wav_for_blocking = wav_path.clone();
    tokio::task::spawn_blocking(move || prepare_wav(&audio_for_blocking, &wav_for_blocking))
        .await
        .context("audio preparation task panicked")??;

    let outcome =
        crate::diarization::sidecar::run(&binary, &wav_path, &paths.segmentation, &paths.embedding)
            .await?;

    let turns: Vec<SpeakerTurn> = outcome.turns;
    SpeakerTurnsRepository::replace_for_meeting(pool, ctx, meeting_id, &turns).await?;
    Ok(turns.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A transcripts-only meeting has no folder at all.
    #[test]
    fn no_folder_means_no_audio() {
        assert!(meeting_audio(None).is_none());
        assert!(meeting_audio(Some("")).is_none());
        assert!(meeting_audio(Some("C:/definitely/not/a/real/folder")).is_none());
    }

    /// A folder that exists but kept no audio is the "transcripts only" case,
    /// and it must be distinguishable from a folder we simply cannot find.
    #[test]
    fn a_folder_without_audio_is_still_no_audio() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("transcripts.json"), b"{}").expect("write");
        assert!(meeting_audio(dir.path().to_str()).is_none());
    }

    /// The exact JSON the frontend receives. Asserted rather than assumed
    /// because serde's `rename_all` on an enum renames the VARIANTS but leaves
    /// struct-variant fields alone -- so the tag is `modelsMissing` while the
    /// field beside it is `diarized_at`. A UI written to the plausible-looking
    /// `diarizedAt` would render `undefined` and still typecheck.
    #[test]
    fn the_wire_shape_the_ui_is_written_against() {
        assert_eq!(
            serde_json::to_string(&Availability::Done {
                diarized_at: "2026-08-09T10:00:00Z".into(),
                turns: 3,
            })
            .expect("serialize"),
            r#"{"status":"done","diarized_at":"2026-08-09T10:00:00Z","turns":3}"#
        );
        assert_eq!(
            serde_json::to_string(&Availability::NoAudio).expect("serialize"),
            r#"{"status":"noAudio"}"#
        );
        assert_eq!(
            serde_json::to_string(&Availability::Ready).expect("serialize"),
            r#"{"status":"ready"}"#
        );
        assert_eq!(
            serde_json::to_string(&Availability::ModelsMissing).expect("serialize"),
            r#"{"status":"modelsMissing"}"#
        );
    }

    #[test]
    fn a_folder_with_audio_resolves_to_it() {
        let dir = tempfile::tempdir().expect("tempdir");
        let audio = dir.path().join("audio.mp4");
        std::fs::write(&audio, b"not really audio, but the probe is by name").expect("write");
        assert_eq!(meeting_audio(dir.path().to_str()), Some(audio));
    }
}

/// The whole orchestration path, run for real.
///
/// Everything else about diarization is tested in pieces: the arithmetic, the
/// wire contract, the repository, the helper's CLI. None of that exercises the
/// path the app actually takes -- probe the audio, decode and resample it,
/// invoke the sidecar, parse it, and land the turns in the database under a
/// tenant. That seam is where the parts meet, and it had never been run.
///
/// `#[ignore]` because it needs the two model files and a built helper, which
/// CI does not have. Run it with:
///
///   MITYU_DIARIZE_HELPER=target/release/diarize-helper.exe \
///   MITYU_TEST_MODELS_DIR=C:\t\diarmodels \
///   cargo test -p mityu --lib diarize_end_to_end -- --ignored --nocapture
#[cfg(test)]
mod end_to_end {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn meeting_with_audio(folder: &str) -> (SqlitePool, AuthContext, String) {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("in-memory SQLite");
        // The real shape, from migration 20260808000000.
        sqlx::query(
            "CREATE TABLE meetings (id TEXT PRIMARY KEY, workspace_id TEXT NOT NULL, \
             folder_path TEXT, diarized_at TEXT, deleted_at TEXT)",
        )
        .execute(&pool)
        .await
        .expect("meetings");
        sqlx::query(
            "CREATE TABLE speaker_turns (id TEXT PRIMARY KEY, meeting_id TEXT NOT NULL, \
             workspace_id TEXT NOT NULL DEFAULT 'local', speaker_label TEXT NOT NULL, \
             start_ms INTEGER NOT NULL, end_ms INTEGER NOT NULL, confidence REAL, \
             created_at TEXT NOT NULL, updated_at TEXT NOT NULL)",
        )
        .execute(&pool)
        .await
        .expect("speaker_turns");
        let ctx = AuthContext::local();
        sqlx::query("INSERT INTO meetings (id, workspace_id, folder_path) VALUES (?, ?, ?)")
            .bind("m-e2e")
            .bind(ctx.tenant_id.as_str())
            .bind(folder)
            .execute(&pool)
            .await
            .expect("insert meeting");
        // The real Phase-1 resolution, not a hand-built struct: the workspace id
        // the repositories scope to must be the one the app actually uses.
        let ctx = AuthContext::local();
        (pool, ctx, "m-e2e".to_string())
    }

    #[tokio::test]
    #[ignore = "needs the diarization models and a built diarize-helper"]
    async fn diarize_end_to_end() {
        let models = std::env::var("MITYU_TEST_MODELS_DIR")
            .expect("set MITYU_TEST_MODELS_DIR to a directory holding the two .onnx files");
        let models_dir = std::path::PathBuf::from(&models);

        // The audio lives beside the models for this test; the app reads it from
        // the meeting's own folder, which is what `folder_path` points at here.
        let folder = models_dir.to_str().expect("utf-8 path").to_string();
        let (pool, ctx, meeting_id) = meeting_with_audio(&folder).await;

        // Before: audio present, models present, never run.
        let before = availability(&pool, &ctx, &meeting_id, Some(&folder), &models_dir)
            .await
            .expect("availability");
        assert_eq!(
            before,
            Availability::Ready,
            "expected Ready, got {before:?}"
        );

        let stored = diarize_meeting(&pool, &ctx, &meeting_id, Some(&folder), &models_dir)
            .await
            .expect("a diarization pass");
        assert!(stored > 0, "the pass stored no turns at all");

        // After: Done, with the stamp that separates "ran" from "never ran".
        let after = availability(&pool, &ctx, &meeting_id, Some(&folder), &models_dir)
            .await
            .expect("availability");
        match after {
            Availability::Done { turns, diarized_at } => {
                assert_eq!(turns, stored, "availability disagrees with what was stored");
                assert!(!diarized_at.is_empty(), "diarized_at was not stamped");
                println!("stored {turns} turns, stamped {diarized_at}");
            }
            other => panic!("expected Done, got {other:?}"),
        }

        // The turns themselves must be usable, not merely present.
        let rows = SpeakerTurnsRepository::list_for_meeting(&pool, &ctx, &meeting_id)
            .await
            .expect("read back");
        assert_eq!(rows.len(), stored);
        assert!(
            rows.iter().all(|t| t.end_ms > t.start_ms),
            "a stored turn ends before it starts"
        );
        assert!(
            rows.iter().all(|t| t.speaker_label.starts_with("Speaker ")),
            "a label is not the anonymous form ADR-0034 requires"
        );
        for w in rows.windows(2) {
            assert!(
                w[0].start_ms <= w[1].start_ms,
                "turns are not in time order"
            );
        }
        println!(
            "labels: {:?}",
            rows.iter()
                .map(|t| t.speaker_label.as_str())
                .collect::<std::collections::BTreeSet<_>>()
        );
    }
}
