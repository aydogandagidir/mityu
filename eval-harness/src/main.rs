//! Phase-0 transcription evaluation harness (docs/PHASE0_VALIDATION.md).
//!
//! Measures the app's OWN engines — whisper via `whisper-rs` and Parakeet via
//! `ort` — by linking the Tauri core crate (`mityu`, lib name `app_lib`)
//! directly. No external whisper CLI / pip package is involved, and no Tauri
//! app is started.
//!
//! Flow: `prep` (raw → 16 kHz mono WAV) → `draft` (machine transcript for the
//! human to correct into `.ref.txt`) → `run` (metrics + report). The GO /
//! CONDITIONAL / NO-GO verdict is always made by a human.

mod engines;
mod metrics;
mod prep;
mod report;
mod wav;

use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand, ValueEnum};

use crate::engines::{
    build_vocab_prompt, load_jargon, resolve_whisper, ParakeetRunner, WhisperRunner,
    DEFAULT_PARAKEET_MODEL, DEFAULT_WHISPER_MODEL, DEFAULT_WHISPER_TURBO_MODEL,
    PARAKEET_WINDOW_SECS,
};
use crate::report::{write_reports, Row, RunMeta};

pub const BUCKETS: [&str; 4] = ["quiet", "field", "multi", "jargon"];

#[derive(Parser)]
#[command(
    name = "eval-harness",
    about = "Phase-0 transcription eval — runs the app's own whisper/Parakeet engines",
    version
)]
struct Cli {
    /// Repo kökü (varsayılan: bu crate'in üst dizini = workspace kökü)
    #[arg(long, global = true)]
    root: Option<PathBuf>,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Copy, Clone, PartialEq, Eq, ValueEnum)]
pub enum EngineKind {
    Whisper,
    Parakeet,
}

#[derive(Subcommand)]
enum Cmd {
    /// eval/raw/<kova>/*.{m4a,mp3,wav,mp4} → eval/<kova>/<id>.wav (16 kHz mono s16, uygulamanın ffmpeg sidecar'ı)
    Prep {
        /// Var olan .wav çıktılarının üzerine yaz
        #[arg(long)]
        force: bool,
    },
    /// .ref.txt'si olmayan klipler için <id>.draft.txt taslak transkript üret
    Draft {
        #[arg(long, value_enum, default_value = "whisper")]
        engine: EngineKind,
        /// Whisper: katalog adı (ör. large-v3) veya ggml-*.bin dosya yolu; Parakeet: model adı
        #[arg(long)]
        model: Option<String>,
        /// Var olan .draft.txt dosyalarını yeniden üret
        #[arg(long)]
        force: bool,
    },
    /// .ref.txt'si olan klipleri konfigürasyonlarla değerlendir; eval/report.{json,md} yaz
    Run {
        /// Virgülle ayrık konfig listesi
        #[arg(
            long,
            value_delimiter = ',',
            default_value = "whisper_large_v3,whisper_large_v3_vocab,whisper_large_v3_turbo,whisper_large_v3_turbo_vocab,parakeet,parakeet_vocab"
        )]
        configs: Vec<String>,
        /// Erken sinyal: kova başına ilk N klip
        #[arg(long)]
        quick: Option<usize>,
        /// YALNIZ whisper_large_v3(_vocab) konfigleri için model: katalog adı (varsayılan large-v3) veya ggml-*.bin yolu
        #[arg(long)]
        model: Option<String>,
        /// YALNIZ whisper_large_v3_turbo(_vocab) konfigleri için model: katalog adı (varsayılan large-v3-turbo) veya ggml-*.bin yolu
        #[arg(long)]
        model_turbo: Option<String>,
        /// Parakeet model adı (uygulamanın indirdiği model yeniden kullanılır)
        #[arg(long, default_value = DEFAULT_PARAKEET_MODEL)]
        parakeet_model: String,
    },
}

#[derive(Copy, Clone, PartialEq, Eq)]
enum RefFilter {
    HasRef,
    MissingRef,
}

struct Clip {
    bucket: String,
    id: String,
    wav: PathBuf,
    ref_path: PathBuf,
    lang: Option<String>,
}

/// Which whisper model a whisper config resolves (per-config model resolution).
#[derive(Copy, Clone, PartialEq, Eq)]
enum WhisperSlot {
    LargeV3,
    Turbo,
}

#[derive(Clone)]
struct RunConfig {
    name: String,
    engine: EngineKind,
    /// `Some` for whisper configs; `None` for parakeet.
    slot: Option<WhisperSlot>,
    vocab: bool,
    note: Option<String>,
}

enum Runner {
    Whisper(WhisperRunner),
    Parakeet(ParakeetRunner),
}

fn default_root() -> PathBuf {
    // eval-harness/ lives directly under the repo root — compile-time anchor.
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    manifest
        .parent()
        .map_or_else(|| manifest.to_path_buf(), Path::to_path_buf)
}

fn read_lang(dir: &Path, id: &str) -> Option<String> {
    let text = std::fs::read_to_string(dir.join(format!("{id}.lang.txt"))).ok()?;
    let lang = text.trim().to_lowercase();
    if lang.is_empty() {
        return None;
    }
    if lang != "tr" && lang != "en" {
        eprintln!(
            "Uyarı: {id}.lang.txt beklenmedik dil kodu '{lang}' (tr/en önerilir) — yine de whisper'a iletilecek"
        );
    }
    Some(lang)
}

fn collect_clips(eval_dir: &Path, filter: RefFilter, quick: Option<usize>) -> Result<Vec<Clip>> {
    let mut clips = Vec::new();
    for bucket in BUCKETS {
        let dir = eval_dir.join(bucket);
        if !dir.is_dir() {
            continue;
        }
        let mut wavs: Vec<PathBuf> = std::fs::read_dir(&dir)
            .with_context(|| format!("dizin okunamadı: {}", dir.display()))?
            .filter_map(std::result::Result::ok)
            .map(|e| e.path())
            .filter(|p| {
                p.is_file()
                    && p.extension()
                        .and_then(|s| s.to_str())
                        .is_some_and(|e| e.eq_ignore_ascii_case("wav"))
            })
            .collect();
        wavs.sort();
        let mut in_bucket = 0usize;
        for wav in wavs {
            if quick.is_some_and(|n| in_bucket >= n) {
                break;
            }
            let Some(id) = wav.file_stem().and_then(|s| s.to_str()).map(String::from) else {
                continue;
            };
            let ref_path = dir.join(format!("{id}.ref.txt"));
            let keep = match filter {
                RefFilter::HasRef => ref_path.is_file(),
                RefFilter::MissingRef => !ref_path.is_file(),
            };
            if !keep {
                continue;
            }
            let lang = read_lang(&dir, &id);
            clips.push(Clip {
                bucket: bucket.to_string(),
                id,
                wav,
                ref_path,
                lang,
            });
            in_bucket += 1;
        }
    }
    Ok(clips)
}

fn parse_configs(names: &[String]) -> Result<Vec<RunConfig>> {
    let mut out = Vec::new();
    for raw in names {
        let name = raw.trim();
        if name.is_empty() {
            continue;
        }
        let cfg = match name {
            "whisper_large_v3" => RunConfig {
                name: name.into(),
                engine: EngineKind::Whisper,
                slot: Some(WhisperSlot::LargeV3),
                vocab: false,
                note: None,
            },
            "whisper_large_v3_vocab" => RunConfig {
                name: name.into(),
                engine: EngineKind::Whisper,
                slot: Some(WhisperSlot::LargeV3),
                vocab: true,
                note: None,
            },
            "whisper_large_v3_turbo" => RunConfig {
                name: name.into(),
                engine: EngineKind::Whisper,
                slot: Some(WhisperSlot::Turbo),
                vocab: false,
                note: None,
            },
            "whisper_large_v3_turbo_vocab" => RunConfig {
                name: name.into(),
                engine: EngineKind::Whisper,
                slot: Some(WhisperSlot::Turbo),
                vocab: true,
                note: None,
            },
            "parakeet" => RunConfig {
                name: name.into(),
                engine: EngineKind::Parakeet,
                slot: None,
                vocab: false,
                note: None,
            },
            "parakeet_vocab" => RunConfig {
                name: name.into(),
                engine: EngineKind::Parakeet,
                slot: None,
                vocab: false,
                note: Some(
                    "Parakeet ort entegrasyonunda hotword/vocab biasing YOK — düz parakeet olarak koşuldu"
                        .into(),
                ),
            },
            other => bail!(
                "bilinmeyen konfig '{other}' (geçerli: whisper_large_v3, whisper_large_v3_vocab, \
                 whisper_large_v3_turbo, whisper_large_v3_turbo_vocab, parakeet, parakeet_vocab)"
            ),
        };
        out.push(cfg);
    }
    if out.is_empty() {
        bail!("en az bir konfig gerekli");
    }
    Ok(out)
}

async fn cmd_draft(
    root: &Path,
    engine: EngineKind,
    model: Option<&str>,
    force: bool,
) -> Result<()> {
    let eval_dir = root.join("eval");
    let clips = collect_clips(&eval_dir, RefFilter::MissingRef, None)?;
    if clips.is_empty() {
        println!(
            "Taslak bekleyen klip yok: ya tüm .wav'ların .ref.txt'si var ya da hiç .wav yok (önce `eval-harness prep`)."
        );
        return Ok(());
    }
    let runner = match engine {
        EngineKind::Whisper => Runner::Whisper(WhisperRunner::load(root, model).await?),
        EngineKind::Parakeet => Runner::Parakeet(
            ParakeetRunner::load(root, model.unwrap_or(DEFAULT_PARAKEET_MODEL)).await?,
        ),
    };
    let mut written = 0usize;
    for clip in &clips {
        let draft_path = eval_dir
            .join(&clip.bucket)
            .join(format!("{}.draft.txt", clip.id));
        if draft_path.is_file() && !force {
            println!("atlandı (draft var): {}/{}", clip.bucket, clip.id);
            continue;
        }
        let samples = wav::read_wav_16k_mono_s16(&clip.wav)?;
        let secs = samples.len() as f64 / f64::from(wav::SAMPLE_RATE);
        println!(
            "taslak üretiliyor: {}/{} ({secs:.1}s)...",
            clip.bucket, clip.id
        );
        let started = Instant::now();
        let text = match &runner {
            Runner::Whisper(w) => w.transcribe(samples, clip.lang.as_deref(), None).await?,
            Runner::Parakeet(p) => p.transcribe(&samples).await?,
        };
        std::fs::write(&draft_path, &text)
            .with_context(|| format!("yazılamadı: {}", draft_path.display()))?;
        println!(
            "  → {} ({:.1}s sürdü)",
            draft_path.display(),
            started.elapsed().as_secs_f64()
        );
        written += 1;
    }
    println!(
        "\n{written} taslak yazıldı. Şimdi her taslağı elle düzeltip aynı dizine <id>.ref.txt \
         olarak kaydedin (insan doğrulaması şart — bkz. eval/README.md)."
    );
    Ok(())
}

fn run_notes(
    cfgs: &[RunConfig],
    prompt_info: Option<&(String, usize)>,
    jargon_total: usize,
    need_parakeet: bool,
) -> Vec<String> {
    let mut notes: Vec<String> = Vec::new();
    if let Some((_, used)) = prompt_info {
        notes.push(format!(
            "Whisper vocab prompt: eval/jargon.txt'den {used}/{jargon_total} terim \
             (~600 karakter sınırı; whisper initial-prompt ≈224 token)"
        ));
        notes.push(
            "Whisper initial-prompt yalnızca ilk 30s penceresini doğrudan koşullar; sonraki \
             pencereler önceki çıktıyı bağlam alır (whisper.cpp davranışı)"
                .to_string(),
        );
    }
    if cfgs.iter().any(|c| c.name == "parakeet_vocab") {
        notes.push(
            "parakeet_vocab: uygulamanın ort tabanlı Parakeet entegrasyonu hotword/vocab \
             biasing desteklemiyor — düz parakeet olarak koşuldu"
                .to_string(),
        );
    }
    if need_parakeet {
        notes.push(format!(
            "Parakeet girdisi {PARAKEET_WINDOW_SECS}s pencerelere bölünerek verildi \
             (uygulamadaki akış kullanımına paralel); pencere sınırlarında küçük WER etkisi olabilir"
        ));
    }
    if cfg!(debug_assertions) {
        notes.push(
            "Harness debug profilde derlendi; whisper.cpp C çekirdekleri her koşulda Release \
             (whisper-rs-sys) ve onnxruntime önceden derlenmiş kütüphane — RTF göstergeseldir"
                .to_string(),
        );
    }
    notes.push("Diyarizasyon Phase 0'da nitel değerlendirilir (harness kapsamı dışı)".to_string());
    notes
}

/// Shared per-run inputs for `eval_config`.
struct EvalCtx<'a> {
    eval_dir: &'a Path,
    clips: &'a [Clip],
    jargon_folded: &'a [String],
    vocab_prompt: Option<&'a str>,
}

#[derive(Copy, Clone)]
enum EngineRef<'a> {
    Whisper(&'a WhisperRunner),
    Parakeet(&'a ParakeetRunner),
}

/// Run one config over all clips: transcribe, score, write hypothesis files, push rows.
async fn eval_config(
    engine: EngineRef<'_>,
    cfg: &RunConfig,
    ctx: &EvalCtx<'_>,
    rows: &mut Vec<Row>,
) -> Result<()> {
    println!("\n=== Konfig: {} ===", cfg.name);
    for clip in ctx.clips {
        let samples = wav::read_wav_16k_mono_s16(&clip.wav)?;
        let audio_secs = samples.len() as f64 / f64::from(wav::SAMPLE_RATE);
        let started = Instant::now();
        let hyp = match engine {
            EngineRef::Whisper(w) => {
                let prompt = if cfg.vocab { ctx.vocab_prompt } else { None };
                w.transcribe(samples, clip.lang.as_deref(), prompt).await?
            }
            EngineRef::Parakeet(p) => p.transcribe(&samples).await?,
        };
        let wall_secs = started.elapsed().as_secs_f64();
        let rtf = if audio_secs > 0.0 {
            wall_secs / audio_secs
        } else {
            0.0
        };
        let ref_text = std::fs::read_to_string(&clip.ref_path)
            .with_context(|| format!("referans okunamadı: {}", clip.ref_path.display()))?;
        let s = metrics::score(&ref_text, &hyp, ctx.jargon_folded);
        let hyp_file = ctx
            .eval_dir
            .join(&clip.bucket)
            .join(format!("{}.{}.hyp.txt", clip.id, cfg.name));
        std::fs::write(&hyp_file, &hyp)
            .with_context(|| format!("hipotez yazılamadı: {}", hyp_file.display()))?;
        println!(
            "[{}] {}/{}: WER {:.3} (fold {:.3}) CER {:.3} terim {} — {:.0}s ses, {:.0}s duvar, RTF {:.2}",
            cfg.name,
            clip.bucket,
            clip.id,
            s.wer,
            s.wer_folded,
            s.cer,
            s.term_recall
                .map_or_else(|| "n/a".to_string(), |r| format!("{r:.2}")),
            audio_secs,
            wall_secs,
            rtf
        );
        rows.push(Row {
            clip: clip.id.clone(),
            bucket: clip.bucket.clone(),
            config: cfg.name.clone(),
            lang: clip.lang.clone(),
            audio_secs,
            wall_secs,
            rtf,
            wer: s.wer,
            wer_folded: s.wer_folded,
            cer: s.cer,
            cer_folded: s.cer_folded,
            term_recall: s.term_recall,
            note: cfg.note.clone(),
            hyp_file: hyp_file.display().to_string(),
        });
    }
    Ok(())
}

async fn cmd_run(
    root: &Path,
    config_names: &[String],
    quick: Option<usize>,
    model: Option<&str>,
    model_turbo: Option<&str>,
    parakeet_model: &str,
) -> Result<()> {
    let eval_dir = root.join("eval");
    let cfgs = parse_configs(config_names)?;
    let clips = collect_clips(&eval_dir, RefFilter::HasRef, quick)?;
    if clips.is_empty() {
        bail!(
            "Değerlendirilecek klip yok: eval/<kova>/<id>.wav + <id>.ref.txt çiftleri gerekli.\n\
             Akış: kayıtları eval/raw/<kova>/ altına koy → `eval-harness prep` → `eval-harness draft` \
             → taslakları düzeltip .ref.txt yap → `eval-harness run`"
        );
    }

    let jargon = load_jargon(&eval_dir)?;
    if jargon.is_empty() {
        eprintln!(
            "Uyarı: eval/jargon.txt boş/yok — terim yakalama ve vocab konfigleri sınırlı olur"
        );
    }
    let jargon_folded: Vec<String> = jargon
        .iter()
        .map(|t| metrics::normalize(t).folded)
        .filter(|t| !t.is_empty())
        .collect();
    let prompt_info = build_vocab_prompt(&jargon);
    let vocab_prompt = prompt_info.as_ref().map(|(p, _)| p.clone());

    let need_parakeet = cfgs.iter().any(|c| c.engine == EngineKind::Parakeet);
    let mut notes = run_notes(&cfgs, prompt_info.as_ref(), jargon.len(), need_parakeet);

    let ctx = EvalCtx {
        eval_dir: &eval_dir,
        clips: &clips,
        jargon_folded: &jargon_folded,
        vocab_prompt: vocab_prompt.as_deref(),
    };

    let mut rows: Vec<Row> = Vec::new();
    let mut whisper_models: Vec<String> = Vec::new();
    let mut parakeet_model_loaded: Option<String> = None;

    // Whisper configs run grouped per model slot; only one whisper context is
    // alive at a time (a large model costs multiple GB of RAM). large-v3 is
    // required (hard fail with the download instruction); turbo is optional —
    // if unavailable, its configs are skipped with a note in the report.
    let slots: [(WhisperSlot, Option<&str>, &str, bool); 2] = [
        (WhisperSlot::LargeV3, model, DEFAULT_WHISPER_MODEL, true),
        (
            WhisperSlot::Turbo,
            model_turbo,
            DEFAULT_WHISPER_TURBO_MODEL,
            false,
        ),
    ];
    for (slot, override_arg, default_name, required) in slots {
        let slot_cfgs: Vec<&RunConfig> = cfgs.iter().filter(|c| c.slot == Some(slot)).collect();
        if slot_cfgs.is_empty() {
            continue;
        }
        let load_result = match resolve_whisper(root, override_arg, default_name) {
            Ok(resolved) => WhisperRunner::load_resolved(resolved).await,
            Err(e) => Err(e),
        };
        match load_result {
            Ok(runner) => {
                whisper_models.push(runner.model_name.clone());
                for cfg in slot_cfgs {
                    eval_config(EngineRef::Whisper(&runner), cfg, &ctx, &mut rows).await?;
                }
                // runner drops here → whisper context freed before the next slot loads
            }
            Err(e) => {
                if required {
                    return Err(e);
                }
                let names = slot_cfgs
                    .iter()
                    .map(|c| c.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                let brief = e
                    .to_string()
                    .lines()
                    .next()
                    .unwrap_or("model kullanılamadı")
                    .to_string();
                eprintln!("\nUyarı: {names} atlanıyor —\n{e}\n");
                notes.push(format!(
                    "ATLANDI ({names}): {brief} Mityu → Settings → Transcription'dan \
                     '{default_name}' indirmesi tamamlanınca bu konfigleri yeniden koşun."
                ));
            }
        }
    }

    if need_parakeet {
        let runner = ParakeetRunner::load(root, parakeet_model).await?;
        parakeet_model_loaded = Some(runner.model_name.clone());
        for cfg in cfgs.iter().filter(|c| c.engine == EngineKind::Parakeet) {
            eval_config(EngineRef::Parakeet(&runner), cfg, &ctx, &mut rows).await?;
        }
    }

    if rows.is_empty() {
        bail!(
            "hiçbir konfig koşulamadı — rapor yazılmadı. Notlar:\n{}",
            notes.join("\n")
        );
    }

    let meta = RunMeta {
        whisper_models,
        parakeet_model: parakeet_model_loaded,
        quick,
        notes,
    };
    let (json_path, md_path) = write_reports(&eval_dir, &rows, &meta)?;
    println!(
        "\nRapor yazıldı:\n  {}\n  {}",
        json_path.display(),
        md_path.display()
    );
    println!(
        "Karar (GO/CONDITIONAL/NO-GO) İNSAN tarafından verilir — raporun Verdict bölümünü doldurun \
         ve docs/DECISIONS.md'ye işleyin."
    );
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();
    let cli = Cli::parse();
    let root = cli.root.clone().unwrap_or_else(default_root);
    let eval_dir = root.join("eval");
    if !eval_dir.is_dir() {
        bail!(
            "eval/ dizini bulunamadı: {} — repo kökünü --root ile verin",
            eval_dir.display()
        );
    }
    match cli.cmd {
        Cmd::Prep { force } => prep::run_prep(&root, &BUCKETS, force),
        Cmd::Draft {
            engine,
            model,
            force,
        } => cmd_draft(&root, engine, model.as_deref(), force).await,
        Cmd::Run {
            configs,
            quick,
            model,
            model_turbo,
            parakeet_model,
        } => {
            cmd_run(
                &root,
                &configs,
                quick,
                model.as_deref(),
                model_turbo.as_deref(),
                &parakeet_model,
            )
            .await
        }
    }
}
