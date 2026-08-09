//! `diarize-helper` — post-hoc speaker diarization, out of process.
//!
//! Audio in, anonymous speaker turns out. It does exactly one thing and holds no
//! state: ADR-0034 makes diarization a whole-file pass over a finished
//! recording, so this is invoked once per meeting and exits.
//!
//! It never touches the capture path (`CLAUDE.md` §4), never reads the database,
//! and never reaches the network — the models are files the caller supplies.
//!
//! Output is JSON in exactly the shape `speaker_turns` stores, or RTTM, which
//! `eval-harness der` scores directly. That second format is not a convenience:
//! it means the engine's output can be measured by the instrument built for it
//! without a conversion step in between that could itself be wrong.

mod turns;

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use clap::{Parser, ValueEnum};
use sherpa_onnx::{
    FastClusteringConfig, OfflineSpeakerDiarization, OfflineSpeakerDiarizationConfig,
    OfflineSpeakerSegmentationModelConfig, OfflineSpeakerSegmentationPyannoteModelConfig,
    SpeakerEmbeddingExtractorConfig,
};

#[derive(Copy, Clone, PartialEq, Eq, ValueEnum)]
enum Format {
    /// `{"speakers": N, "turns": [{start_ms, end_ms, speaker_label, confidence}]}`
    Json,
    /// NIST RTTM — feeds straight into `eval-harness der`.
    Rttm,
}

#[derive(Parser)]
#[command(
    name = "diarize-helper",
    about = "Post-hoc speaker diarization (ADR-0034/0035). Anonymous labels only.",
    version
)]
struct Cli {
    /// 16 kHz mono WAV. The caller resamples; this tool refuses anything else
    /// rather than resampling silently.
    #[arg(long)]
    audio: PathBuf,
    /// pyannote segmentation ONNX model.
    #[arg(long)]
    segmentation: PathBuf,
    /// Speaker embedding ONNX model (CAM++ by default — ADR-0034).
    #[arg(long)]
    embedding: PathBuf,
    /// Number of speakers, when it is known. Omit when it is not: the engine
    /// then clusters by `--threshold` instead of being told an answer.
    #[arg(long)]
    num_speakers: Option<i32>,
    /// Clustering threshold used when the speaker count is unknown.
    #[arg(long, default_value_t = 0.5)]
    threshold: f32,
    #[arg(long, default_value_t = 1)]
    threads: i32,
    #[arg(long, value_enum, default_value = "json")]
    format: Format,
    /// Recording id written into RTTM output.
    #[arg(long, default_value = "meeting")]
    file_id: String,
}

fn require_file(path: &Path, what: &str) -> Result<String> {
    if !path.is_file() {
        bail!("{what} not found: {}", path.display());
    }
    Ok(path.to_string_lossy().into_owned())
}

/// Read a mono 16 kHz WAV into the `f32` samples the engine expects.
///
/// Refuses a mismatched rate or channel count instead of resampling or
/// downmixing. A silent conversion here would change *when* speech happened and
/// therefore every timestamp downstream — and the caller already owns an FFmpeg
/// sidecar that does this correctly.
fn read_mono_wav(path: &Path, expected_rate: i32) -> Result<Vec<f32>> {
    let mut reader =
        hound::WavReader::open(path).with_context(|| format!("open audio: {}", path.display()))?;
    let spec = reader.spec();

    if spec.channels != 1 {
        bail!(
            "audio must be mono, got {} channels: {}. Downmixing here would hide a \
             capture-side mistake; convert it first.",
            spec.channels,
            path.display()
        );
    }
    if spec.sample_rate as i32 != expected_rate {
        bail!(
            "audio must be {expected_rate} Hz (the model's rate), got {} Hz: {}. \
             Resampling here would shift every timestamp; convert it first.",
            spec.sample_rate,
            path.display()
        );
    }

    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader.samples::<f32>().collect::<Result<_, _>>()?,
        hound::SampleFormat::Int => {
            let scale = 1.0_f32 / (1_i64 << (spec.bits_per_sample - 1)) as f32;
            reader
                .samples::<i32>()
                .map(|s| s.map(|v| v as f32 * scale))
                .collect::<Result<_, _>>()?
        }
    };
    if samples.is_empty() {
        bail!("audio contains no samples: {}", path.display());
    }
    Ok(samples)
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let segmentation = require_file(&cli.segmentation, "segmentation model")?;
    let embedding = require_file(&cli.embedding, "embedding model")?;
    if !cli.audio.is_file() {
        bail!("audio not found: {}", cli.audio.display());
    }

    let config = OfflineSpeakerDiarizationConfig {
        segmentation: OfflineSpeakerSegmentationModelConfig {
            pyannote: OfflineSpeakerSegmentationPyannoteModelConfig {
                model: Some(segmentation),
            },
            num_threads: cli.threads,
            ..OfflineSpeakerSegmentationModelConfig::default()
        },
        embedding: SpeakerEmbeddingExtractorConfig {
            model: Some(embedding),
            num_threads: cli.threads,
            ..SpeakerEmbeddingExtractorConfig::default()
        },
        clustering: FastClusteringConfig {
            // -1 means "unknown, cluster by threshold". Defaulting to a fixed
            // count would impose a speaker count on every meeting, and a
            // two-person conversation is the typical recording.
            num_clusters: cli.num_speakers.unwrap_or(-1),
            threshold: cli.threshold,
        },
        ..OfflineSpeakerDiarizationConfig::default()
    };

    let diarizer = OfflineSpeakerDiarization::create(&config)
        .context("create diarizer (check the model paths and that both files are ONNX)")?;

    let samples = read_mono_wav(&cli.audio, diarizer.sample_rate())?;

    let result = diarizer
        .process(&samples)
        .context("diarization failed on this audio")?;

    let segments: Vec<(f32, f32, i32)> = result
        .sort_by_start_time()
        .into_iter()
        .map(|s| (s.start, s.end, s.speaker))
        .collect();
    let turns = turns::to_speaker_turns(&segments)?;

    match cli.format {
        Format::Json => {
            let doc = serde_json::json!({
                "speakers": result.num_speakers(),
                "turns": turns,
            });
            println!("{}", serde_json::to_string_pretty(&doc)?);
        }
        Format::Rttm => print!("{}", turns::to_rttm(&cli.file_id, &turns)),
    }
    Ok(())
}
