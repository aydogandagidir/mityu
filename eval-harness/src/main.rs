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

mod diarization;
mod engines;
mod metrics;
mod prep;
mod report;
mod wav;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand, ValueEnum};

use crate::engines::{
    build_vocab_prompt, load_jargon_file, resolve_whisper, ParakeetRunner, WhisperRunner,
    DEFAULT_PARAKEET_MODEL, DEFAULT_WHISPER_MODEL, DEFAULT_WHISPER_TURBO_MODEL, MAX_PROMPT_CHARS,
    PARAKEET_WINDOW_SECS,
};
use crate::report::{write_reports, Row, RunMeta};

pub const BUCKETS: [&str; 4] = ["quiet", "field", "multi", "jargon"];
const MIN_GATE_CLIPS_PER_BUCKET: usize = 5;

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
        /// Kova başına en fazla ilk N klip (Phase-0 gate için N >= 5 olmalı)
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
    /// İki RTTM dosyasını karşılaştırıp DER (Diarization Error Rate) hesapla
    ///
    /// ADR-0034 adım (c)'nin ölçü aleti: bir diyarizasyon motoru seçilmeden önce
    /// iyiyi kötüden ayırabilmek gerekir. `eval/` dizinine ihtiyaç duymaz.
    Der {
        /// İnsan tarafından etiketlenmiş referans RTTM
        #[arg(long)]
        reference: PathBuf,
        /// Sistem çıktısı RTTM
        #[arg(long)]
        hypothesis: PathBuf,
        /// Birden fazla kayıt içeren RTTM'de puanlanacak dosya kimliği
        #[arg(long)]
        file_id: Option<String>,
        /// Referans sınırları etrafında puanlanmayan pay (saniye). NIST geleneği
        /// 0.25; yayınlanmış pyannote sayıları 0 ile hesaplanır.
        #[arg(long, default_value_t = 0.0)]
        collar: f64,
        /// Birden fazla referans konuşmacısının aynı anda konuştuğu bölgeleri
        /// puanlama dışı bırak. DER'i daima düşürür — bu şekilde hesaplanan bir
        /// sayı, örtüşme dahil hesaplanmış bir sayıyla KARŞILAŞTIRILAMAZ.
        #[arg(long)]
        skip_overlap: bool,
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
    /// Domain vocabulary set name from the `<id>.vocab.txt` sidecar. `None` →
    /// the `default` set (`eval/jargon.txt`).
    vocab: Option<String>,
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

/// Read the `<id>.vocab.txt` sidecar naming this clip's domain vocabulary set
/// (mirrors `<id>.lang.txt`). The file holds one set name, e.g. `legal`, which
/// resolves to `eval/jargon.legal.txt`. Unknown names are rejected later by
/// [`VocabSets::validate_clips`] — never silently downgraded to `default`,
/// because a typo would then measure the wrong domain's vocabulary.
/// A missing sidecar is the normal "use `default`" case and yields `Ok(None)`.
/// Every *other* failure — unreadable file, or bytes that are not UTF-8, which
/// is what a Windows PowerShell 5.1 `>` redirect produces (UTF-16LE) — is
/// returned as an error instead of being flattened to `None`. Flattening would
/// silently score the clip against `default`, the exact wrong-domain failure
/// this sidecar exists to prevent.
fn read_vocab_set(dir: &Path, id: &str) -> Result<Option<String>> {
    let path = dir.join(format!("{id}.vocab.txt"));
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) if e.kind() == std::io::ErrorKind::InvalidData => bail!(
            "{} UTF-8 değil ({e}). Vocab sidecar'ı düz UTF-8 olmalı — Windows PowerShell 5.1'de \
             `>` yönlendirmesi UTF-16 üretir; `Set-Content -Encoding utf8` kullanın veya dosyayı \
             editörde UTF-8 olarak kaydedin.",
            path.display()
        ),
        Err(e) => bail!("vocab sidecar okunamadı ({}): {e}", path.display()),
    };
    // A UTF-8 BOM would otherwise ride along on the first line and turn a valid
    // set name into an unknown one.
    let name = text
        .trim_start_matches('\u{feff}')
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty() && !l.starts_with('#'))
        .map(str::to_lowercase);
    match name {
        // Present but blank/comment-only: treat as "no selection" rather than an
        // error, matching an absent sidecar.
        None => Ok(None),
        Some(name) if name.is_empty() => Ok(None),
        Some(name) => Ok(Some(name)),
    }
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
        // `--quick` caps per (bucket, domain), not per bucket. Capping per bucket
        // would let alphabetical order erase a whole domain — with `ji*`/`jl*`
        // ids, `--quick 5` would take five `ji*` clips, still satisfy the
        // ≥5-per-bucket rule, and report a one-domain pass while the legal
        // domain was never measured at all. For a single-domain bucket this is
        // identical to the old behaviour.
        let mut per_domain: BTreeMap<String, usize> = BTreeMap::new();
        for wav in wavs {
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
            let vocab = read_vocab_set(&dir, &id)?;
            let domain = vocab
                .clone()
                .unwrap_or_else(|| DEFAULT_VOCAB_SET.to_string());
            let taken = per_domain.entry(domain).or_default();
            if quick.is_some_and(|n| *taken >= n) {
                continue;
            }
            clips.push(Clip {
                bucket: bucket.to_string(),
                id,
                wav,
                ref_path,
                lang,
                vocab,
            });
            *taken += 1;
        }
    }
    Ok(clips)
}

/// Phase-0 is a fail-closed product gate: every required environment must be
/// represented by at least five human-reviewed reference pairs. A `.ref.txt`
/// file is the workflow's explicit human-approval artifact; drafts never count.
fn validate_bucket_coverage(clips: &[Clip]) -> Result<()> {
    let mut counts: BTreeMap<&str, usize> = BUCKETS.into_iter().map(|b| (b, 0)).collect();
    for clip in clips {
        if let Some(count) = counts.get_mut(clip.bucket.as_str()) {
            *count += 1;
        }
    }

    let missing = counts
        .iter()
        .filter(|(_, count)| **count < MIN_GATE_CLIPS_PER_BUCKET)
        .map(|(bucket, count)| {
            format!("{bucket}: {count}/{MIN_GATE_CLIPS_PER_BUCKET} geçerli çift")
        })
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        bail!(
            "Phase-0 gate veri seti eksik; `run` fail-closed durduruldu. Her kovada en az \
             {MIN_GATE_CLIPS_PER_BUCKET} adet 16 kHz mono s16 WAV + insan tarafından düzeltilip \
             onaylanmış .ref.txt çifti zorunlu. Eksikler: {}\nAkış: kayıtları \
             eval/raw/<kova>/ altına koy → `eval-harness prep` → `eval-harness draft` → \
             taslakları insan olarak düzeltip .ref.txt yap → `eval-harness run`",
            missing.join(", ")
        );
    }
    Ok(())
}

/// Every domain vocabulary present in the `jargon` bucket is gated on its own
/// term-recall median, so each one needs the same five-pair evidence the bucket
/// as a whole does. Without this, a domain represented by one or two clips would
/// still produce a PASS/FAIL cell in the threshold table off a median that is
/// not gate-grade — and a domain that lost its clips would vanish from the
/// report entirely rather than fail closed.
fn validate_jargon_domain_coverage(clips: &[Clip]) -> Result<()> {
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for clip in clips.iter().filter(|c| c.bucket == "jargon") {
        *counts
            .entry(clip.vocab.as_deref().unwrap_or(DEFAULT_VOCAB_SET))
            .or_default() += 1;
    }
    // A single-domain jargon bucket is already covered by the per-bucket rule.
    if counts.len() < 2 {
        return Ok(());
    }
    let thin: Vec<String> = counts
        .iter()
        .filter(|(_, count)| **count < MIN_GATE_CLIPS_PER_BUCKET)
        .map(|(domain, count)| format!("{domain}: {count}/{MIN_GATE_CLIPS_PER_BUCKET} klip"))
        .collect();
    if !thin.is_empty() {
        bail!(
            "jargon kovasında {} domain var ve her biri kendi terim-yakalama eşiğiyle \
             kapılanıyor; bu yüzden her domain en az {MIN_GATE_CLIPS_PER_BUCKET} klip ister. \
             Eksik: {}\nYa eksik domainin kliplerini tamamlayın ya da o domaini bu koşudan \
             tamamen çıkarın (sidecar'ları kaldırın) — yarım domain kapı kanıtı sayılmaz.",
            counts.len(),
            thin.join(", ")
        );
    }
    Ok(())
}

fn validate_gate_inputs(clips: &[Clip]) -> Result<()> {
    validate_bucket_coverage(clips)?;
    validate_jargon_domain_coverage(clips)?;

    let mut invalid = Vec::new();
    for clip in clips {
        match std::fs::read_to_string(&clip.ref_path) {
            Ok(reference) if !reference.trim().is_empty() => {}
            Ok(_) => invalid.push(format!(
                "{}/{}: .ref.txt boş (insan-onaylı referans gerekli)",
                clip.bucket, clip.id
            )),
            Err(error) => invalid.push(format!(
                "{}/{}: .ref.txt okunamadı: {error}",
                clip.bucket, clip.id
            )),
        }
        if let Err(error) = wav::read_wav_16k_mono_s16(&clip.wav) {
            invalid.push(format!("{}/{}: {error:#}", clip.bucket, clip.id));
        }
    }
    if !invalid.is_empty() {
        bail!(
            "Phase-0 gate girdileri geçersiz; model yüklenmeden durduruldu:\n{}",
            invalid.join("\n")
        );
    }
    Ok(())
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

/// Name of the fallback vocabulary set, backed by `eval/jargon.txt`.
const DEFAULT_VOCAB_SET: &str = "default";

/// One domain vocabulary: the folded terms used for term recall plus the whisper
/// `initial_prompt` built from the same list.
struct VocabSet {
    /// File this set came from, relative to `eval/` — for the report note.
    source: String,
    terms: usize,
    folded: Vec<String>,
    prompt: Option<String>,
    /// How many terms fit inside the whisper prompt budget.
    prompt_terms: usize,
}

/// Every domain vocabulary found under `eval/`: `jargon.txt` is the `default`
/// set and each `jargon.<name>.txt` adds the set `<name>`. A clip picks its set
/// through the `<id>.vocab.txt` sidecar.
///
/// This split exists because the whisper prompt budget is ~600 chars
/// ([`engines::MAX_PROMPT_CHARS`], ≈224 tokens). A single merged multi-domain
/// term list would only ever bias whichever domain is listed first, so the
/// `*_vocab` configs for every other domain would silently measure the wrong
/// vocabulary — and the run would still look successful.
struct VocabSets {
    sets: BTreeMap<String, VocabSet>,
}

fn build_vocab_set(source: &str, terms: &[String]) -> VocabSet {
    let folded: Vec<String> = terms
        .iter()
        .map(|t| metrics::normalize(t).folded)
        .filter(|t| !t.is_empty())
        .collect();
    let prompt_info = build_vocab_prompt(terms);
    VocabSet {
        source: source.to_string(),
        terms: terms.len(),
        folded,
        prompt: prompt_info.as_ref().map(|(p, _)| p.clone()),
        prompt_terms: prompt_info.map_or(0, |(_, used)| used),
    }
}

fn load_vocab_sets(eval_dir: &Path) -> Result<VocabSets> {
    let mut sets = BTreeMap::new();
    let default_terms = load_jargon_file(&eval_dir.join("jargon.txt"))?;
    sets.insert(
        DEFAULT_VOCAB_SET.to_string(),
        build_vocab_set("jargon.txt", &default_terms),
    );

    if eval_dir.is_dir() {
        let mut files: Vec<PathBuf> = std::fs::read_dir(eval_dir)
            .with_context(|| format!("dizin okunamadı: {}", eval_dir.display()))?
            .filter_map(std::result::Result::ok)
            .map(|e| e.path())
            .filter(|p| p.is_file())
            .collect();
        files.sort();
        for path in files {
            let Some(file) = path.file_name().and_then(|s| s.to_str()) else {
                continue;
            };
            // "jargon.txt" strips to "txt", which has no ".txt" suffix → skipped
            // here and handled above as the default set.
            let Some(name) = file
                .strip_prefix("jargon.")
                .and_then(|rest| rest.strip_suffix(".txt"))
            else {
                continue;
            };
            let name = name.to_lowercase();
            if name.is_empty() {
                continue;
            }
            if name == DEFAULT_VOCAB_SET {
                bail!(
                    "eval/{file} adı '{DEFAULT_VOCAB_SET}' setiyle çakışıyor \
                     (o set eval/jargon.txt'ten gelir). Dosyayı farklı adlandırın."
                );
            }
            let terms = load_jargon_file(&path)?;
            sets.insert(name, build_vocab_set(file, &terms));
        }
    }
    Ok(VocabSets { sets })
}

impl VocabSets {
    /// Reject any clip naming a set that was not loaded — before a model is
    /// loaded, like the other gate checks. A typo must never fall back to
    /// `default`: that is exactly the silent wrong-domain measurement this
    /// per-clip split was introduced to prevent.
    fn validate_clips(&self, clips: &[Clip]) -> Result<()> {
        let mut bad: Vec<String> = clips
            .iter()
            .filter_map(|c| c.vocab.as_deref().map(|v| (c, v)))
            .filter(|(_, v)| !self.sets.contains_key(*v))
            .map(|(c, v)| format!("{}/{}.vocab.txt → '{v}'", c.bucket, c.id))
            .collect();
        if bad.is_empty() {
            return Ok(());
        }
        bad.sort();
        let known: Vec<&str> = self.sets.keys().map(String::as_str).collect();
        bail!(
            "Bilinmeyen vocab seti ({} klip):\n  {}\n\
             Tanımlı setler: {}\n\
             Yeni set için eval/jargon.<ad>.txt dosyası oluşturun.",
            bad.len(),
            bad.join("\n  "),
            known.join(", ")
        );
    }

    /// The set actually used for a clip, with the name it resolved to.
    fn for_clip(&self, clip: &Clip) -> (&str, &VocabSet) {
        let requested = clip.vocab.as_deref().unwrap_or(DEFAULT_VOCAB_SET);
        if let Some((name, set)) = self.sets.get_key_value(requested) {
            return (name.as_str(), set);
        }
        let (name, set) = self
            .sets
            .get_key_value(DEFAULT_VOCAB_SET)
            .expect("default vocab set is always inserted");
        (name.as_str(), set)
    }

    fn is_empty(&self) -> bool {
        self.sets.values().all(|s| s.terms == 0)
    }
}

fn run_notes(
    cfgs: &[RunConfig],
    vocabs: &VocabSets,
    clips: &[Clip],
    need_parakeet: bool,
) -> Vec<String> {
    let mut notes: Vec<String> = Vec::new();

    // Term recall is scored against the clip's own set, so the mapping belongs in
    // the report even when no *_vocab config runs.
    let mut per_set: BTreeMap<&str, usize> = BTreeMap::new();
    for clip in clips {
        let (name, _) = vocabs.for_clip(clip);
        *per_set.entry(name).or_default() += 1;
    }
    for (name, clip_count) in &per_set {
        let Some(set) = vocabs.sets.get(*name) else {
            continue;
        };
        notes.push(format!(
            "Vocab seti '{name}' (eval/{}): {} terim — {clip_count} klip \
             (terim yakalama bu sete göre ölçülür)",
            set.source, set.terms
        ));
    }
    if cfgs.iter().any(|c| c.vocab) {
        for name in per_set.keys() {
            let Some(set) = vocabs.sets.get(*name) else {
                continue;
            };
            if set.prompt.is_none() {
                continue;
            }
            notes.push(format!(
                "Whisper vocab prompt '{name}': {}/{} terim (~{} karakter sınırı; \
                 whisper initial-prompt ≈224 token)",
                set.prompt_terms, set.terms, MAX_PROMPT_CHARS
            ));
        }
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
    notes.push(
        "Diyarizasyon BU koşuda puanlanmaz: A5 referansları düz metindir, konuşmacı turn'ü \
         taşımaz (docs/A5_SPRINT.md) — dolayısıyla report.md'deki multi-speaker insan \
         inceleme alanında nitel olarak kaydedilir. Konuşmacı etiketli RTTM referansı \
         olduğunda `eval-harness der --reference X.rttm --hypothesis Y.rttm` DER hesaplar"
            .to_string(),
    );
    notes
}

/// Shared per-run inputs for `eval_config`.
struct EvalCtx<'a> {
    eval_dir: &'a Path,
    clips: &'a [Clip],
    vocabs: &'a VocabSets,
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
        // Vocabulary is per clip: prompt bias and term recall both come from the
        // set this clip declares, so a legal clip is never biased or scored with
        // another domain's terms.
        let (vocab_set_name, vocab) = ctx.vocabs.for_clip(clip);
        let samples = wav::read_wav_16k_mono_s16(&clip.wav)?;
        let audio_secs = samples.len() as f64 / f64::from(wav::SAMPLE_RATE);
        let started = Instant::now();
        let hyp = match engine {
            EngineRef::Whisper(w) => {
                let prompt = if cfg.vocab {
                    vocab.prompt.as_deref()
                } else {
                    None
                };
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
        let s = metrics::score(&ref_text, &hyp, &vocab.folded);
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
            vocab_set: vocab_set_name.to_string(),
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
    validate_gate_inputs(&clips)?;

    let vocabs = load_vocab_sets(&eval_dir)?;
    vocabs.validate_clips(&clips)?;
    if vocabs.is_empty() {
        eprintln!(
            "Uyarı: eval/jargon.txt (ve eval/jargon.<ad>.txt) boş/yok — terim yakalama ve \
             vocab konfigleri sınırlı olur"
        );
    }

    let need_parakeet = cfgs.iter().any(|c| c.engine == EngineKind::Parakeet);
    let mut notes = run_notes(&cfgs, &vocabs, &clips, need_parakeet);

    let ctx = EvalCtx {
        eval_dir: &eval_dir,
        clips: &clips,
        vocabs: &vocabs,
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

    // `der` scores two RTTM paths given on the command line and never reads the
    // corpus, so it is dispatched before the eval/ requirement below.
    if let Cmd::Der {
        reference,
        hypothesis,
        file_id,
        collar,
        skip_overlap,
    } = &cli.cmd
    {
        return cmd_der(
            reference,
            hypothesis,
            file_id.as_deref(),
            *collar,
            *skip_overlap,
        );
    }

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
        // Handled above, before the eval/ requirement.
        Cmd::Der { .. } => unreachable!("dispatched before the eval/ check"),
    }
}

/// Score one hypothesis RTTM against a reference RTTM.
///
/// Prints the three DER components separately, not just the headline number:
/// the same 20% DER means very different things when it is all missed speech
/// (the segmenter is deaf) versus all confusion (the clustering is wrong), and
/// only the components say which part to fix. The speaker mapping is printed for
/// the same reason — it is the step most likely to be silently wrong.
fn cmd_der(
    reference: &Path,
    hypothesis: &Path,
    file_id: Option<&str>,
    collar_secs: f64,
    skip_overlap: bool,
) -> Result<()> {
    if !(collar_secs.is_finite() && collar_secs >= 0.0) {
        bail!("--collar negatif olamaz ve sonlu olmalı: {collar_secs}");
    }

    let read = |path: &Path, what: &str| -> Result<Vec<diarization::Turn>> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("{what} RTTM okunamadı: {}", path.display()))?;
        diarization::parse_rttm(&text, file_id)
            .with_context(|| format!("{what} RTTM ayrıştırılamadı: {}", path.display()))
    };
    let reference_turns = read(reference, "Referans")?;
    let hypothesis_turns = read(hypothesis, "Hipotez")?;

    let report = diarization::der(
        &reference_turns,
        &hypothesis_turns,
        diarization::DerOptions {
            collar: diarization::secs_to_micros(collar_secs),
            skip_overlap,
        },
    )?;

    println!("DER            {:.2}%", report.der_percent());
    println!("  missed       {:.2}s", report.missed);
    println!("  false alarm  {:.2}s", report.false_alarm);
    println!("  confusion    {:.2}s", report.confusion);
    println!("  reference    {:.2}s", report.total_reference);
    if report.excluded > 0.0 {
        println!("  excluded     {:.2}s (collar/overlap)", report.excluded);
    }
    println!(
        "konuşmacı      referans {} / hipotez {}",
        report.ref_speakers, report.hyp_speakers
    );
    for (r, h) in &report.mapping {
        println!("  {r} -> {h}");
    }
    let unmatched: Vec<&String> = reference_turns
        .iter()
        .map(|t| &t.speaker)
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .filter(|s| !report.mapping.contains_key(*s))
        .collect();
    for s in unmatched {
        println!("  {s} -> (eşleşmedi)");
    }
    println!(
        "koşul          collar={collar_secs:.2}s, örtüşme {}",
        if skip_overlap {
            "puanlanmadı"
        } else {
            "dahil"
        }
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clip(bucket: &str, index: usize) -> Clip {
        Clip {
            bucket: bucket.to_string(),
            id: format!("{bucket}-{index}"),
            wav: PathBuf::from(format!("{bucket}-{index}.wav")),
            ref_path: PathBuf::from(format!("{bucket}-{index}.ref.txt")),
            lang: None,
            vocab: None,
        }
    }

    fn complete_gate_set() -> Vec<Clip> {
        BUCKETS
            .iter()
            .flat_map(|bucket| (0..MIN_GATE_CLIPS_PER_BUCKET).map(move |index| clip(bucket, index)))
            .collect()
    }

    #[test]
    fn gate_coverage_accepts_five_pairs_in_every_bucket() {
        validate_bucket_coverage(&complete_gate_set()).expect("complete gate set");
    }

    #[test]
    fn gate_coverage_fails_closed_when_a_bucket_has_fewer_than_five_pairs() {
        let mut clips = complete_gate_set();
        clips.retain(|clip| !(clip.bucket == "field" && clip.id == "field-4"));

        let error = validate_bucket_coverage(&clips).expect_err("incomplete field bucket");
        let message = error.to_string();
        assert!(message.contains("field: 4/5"));
        assert!(message.contains("fail-closed"));
    }

    #[test]
    fn gate_coverage_requires_all_four_named_buckets() {
        let clips: Vec<Clip> = (0..MIN_GATE_CLIPS_PER_BUCKET)
            .map(|index| clip("quiet", index))
            .collect();

        let message = validate_bucket_coverage(&clips)
            .expect_err("three buckets are absent")
            .to_string();
        assert!(message.contains("field: 0/5"));
        assert!(message.contains("multi: 0/5"));
        assert!(message.contains("jargon: 0/5"));
    }

    // --- per-clip domain vocabulary -------------------------------------------

    fn temp_eval_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("mityu-eval-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp eval dir");
        dir
    }

    fn write_file(dir: &Path, name: &str, body: &str) {
        std::fs::write(dir.join(name), body).expect("write fixture");
    }

    #[test]
    fn vocab_sets_discovers_named_files_and_skips_comments() {
        let dir = temp_eval_dir("sets");
        write_file(&dir, "jargon.txt", "konveyör\nAGV\n");
        write_file(
            &dir,
            "jargon.legal.txt",
            "# yorum\n\nihtarname\nbilirkişi raporu\n",
        );

        let sets = load_vocab_sets(&dir).expect("sets load");

        assert_eq!(sets.sets.len(), 2);
        assert_eq!(sets.sets[DEFAULT_VOCAB_SET].terms, 2);
        assert_eq!(sets.sets["legal"].terms, 2);
        assert_eq!(sets.sets["legal"].source, "jargon.legal.txt");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The regression this per-clip split exists for: with one merged list the
    /// ~600-char whisper prompt only ever carries whichever domain is listed
    /// first, so the other domain's `*_vocab` run silently measures the wrong
    /// vocabulary. Each clip must get its own domain's prompt *and* its own
    /// terms for recall.
    #[test]
    fn each_clip_gets_only_its_own_domain_vocabulary() {
        let dir = temp_eval_dir("perclip");
        write_file(&dir, "jargon.txt", "konveyör\n");
        write_file(&dir, "jargon.legal.txt", "ihtarname\n");
        let sets = load_vocab_sets(&dir).expect("sets load");

        let mut legal_clip = clip("jargon", 1);
        legal_clip.vocab = Some("legal".to_string());
        let default_clip = clip("jargon", 2); // no sidecar

        let (legal_name, legal_set) = sets.for_clip(&legal_clip);
        let legal_prompt = legal_set.prompt.as_deref().expect("legal prompt");
        assert_eq!(legal_name, "legal");
        assert!(legal_prompt.contains("ihtarname"));
        assert!(!legal_prompt.contains("konveyör"));
        assert_eq!(legal_set.folded.len(), 1);

        let (default_name, default_set) = sets.for_clip(&default_clip);
        let default_prompt = default_set.prompt.as_deref().expect("default prompt");
        assert_eq!(default_name, DEFAULT_VOCAB_SET);
        assert!(default_prompt.contains("konveyör"));
        assert!(!default_prompt.contains("ihtarname"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn unknown_vocab_set_fails_closed() {
        let dir = temp_eval_dir("unknown");
        write_file(&dir, "jargon.txt", "konveyör\n");
        write_file(&dir, "jargon.legal.txt", "ihtarname\n");
        let sets = load_vocab_sets(&dir).expect("sets load");

        let mut typo = clip("jargon", 1);
        typo.vocab = Some("legall".to_string());

        let message = sets
            .validate_clips(&[typo])
            .expect_err("typo must not fall back to default")
            .to_string();
        assert!(message.contains("legall"));
        assert!(message.contains("Tanımlı setler"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn declared_vocab_sets_validate() {
        let dir = temp_eval_dir("valid");
        write_file(&dir, "jargon.txt", "konveyör\n");
        write_file(&dir, "jargon.legal.txt", "ihtarname\n");
        let sets = load_vocab_sets(&dir).expect("sets load");

        let mut legal = clip("jargon", 1);
        legal.vocab = Some("legal".to_string());
        assert!(sets.validate_clips(&[legal, clip("quiet", 1)]).is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn jargon_default_txt_collides_with_the_default_set() {
        let dir = temp_eval_dir("collide");
        write_file(&dir, "jargon.txt", "konveyör\n");
        write_file(&dir, "jargon.default.txt", "ihtarname\n");
        assert!(load_vocab_sets(&dir).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn sidecar_reads_first_meaningful_line_lowercased() {
        let dir = temp_eval_dir("sidecar");
        write_file(&dir, "j01.vocab.txt", "\n# hangi set\nLEGAL\n");
        let name = read_vocab_set(&dir, "j01").expect("readable sidecar");
        assert_eq!(name.as_deref(), Some("legal"));
        // absent sidecar is the normal "use default" case, not an error
        assert_eq!(read_vocab_set(&dir, "absent").expect("absent is ok"), None);
        // present but blank/comment-only behaves like absent
        write_file(&dir, "j02.vocab.txt", "# yalnızca yorum\n\n");
        assert_eq!(read_vocab_set(&dir, "j02").expect("comment-only"), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A Windows PowerShell 5.1 `>` redirect writes UTF-16LE. Flattening that
    /// read error to `None` would silently score the clip against `default` —
    /// the wrong-domain failure the sidecar exists to prevent.
    #[test]
    fn sidecar_that_is_not_utf8_fails_closed() {
        let dir = temp_eval_dir("utf16");
        // UTF-16LE BOM + "legal"
        let utf16: Vec<u8> = vec![0xFF, 0xFE, b'l', 0, b'e', 0, b'g', 0, b'a', 0, b'l', 0];
        std::fs::write(dir.join("j01.vocab.txt"), utf16).expect("write utf16");

        let message = read_vocab_set(&dir, "j01")
            .expect_err("UTF-16 sidecar must not be flattened to None")
            .to_string();
        assert!(message.contains("UTF-8 değil"));
        assert!(message.contains("PowerShell"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn sidecar_utf8_bom_does_not_break_the_set_name() {
        let dir = temp_eval_dir("bom");
        std::fs::write(dir.join("j01.vocab.txt"), "\u{feff}legal\n".as_bytes())
            .expect("write bom file");
        let name = read_vocab_set(&dir, "j01").expect("bom sidecar readable");
        assert_eq!(name.as_deref(), Some("legal"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn make_clip_files(bucket_dir: &Path, id: &str, vocab: Option<&str>) {
        std::fs::write(bucket_dir.join(format!("{id}.wav")), b"").expect("wav");
        std::fs::write(bucket_dir.join(format!("{id}.ref.txt")), "metin").expect("ref");
        if let Some(v) = vocab {
            std::fs::write(bucket_dir.join(format!("{id}.vocab.txt")), v).expect("vocab");
        }
    }

    /// `--quick` caps per (bucket, domain). Capping per bucket let alphabetical
    /// order erase a domain: with `ji*`/`jl*` ids, `--quick 5` took five `ji*`
    /// clips, still satisfied the ≥5-per-bucket rule, and reported a one-domain
    /// pass while the legal domain was never measured.
    #[test]
    fn quick_sampling_cannot_erase_a_domain() {
        let root = temp_eval_dir("quick");
        let jargon = root.join("jargon");
        std::fs::create_dir_all(&jargon).expect("jargon dir");
        for i in 1..=5 {
            make_clip_files(&jargon, &format!("ji0{i}"), None);
            make_clip_files(&jargon, &format!("jl0{i}"), Some("legal"));
        }

        let clips = collect_clips(&root, RefFilter::HasRef, Some(5)).expect("collect");
        let legal = clips
            .iter()
            .filter(|c| c.vocab.as_deref() == Some("legal"))
            .count();
        let default = clips.iter().filter(|c| c.vocab.is_none()).count();
        assert_eq!(
            legal, 5,
            "alphabetical order must not drop the legal domain"
        );
        assert_eq!(default, 5);

        // The cap still applies — per domain.
        let capped = collect_clips(&root, RefFilter::HasRef, Some(2)).expect("collect capped");
        assert_eq!(capped.len(), 4);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_thin_second_jargon_domain_fails_closed() {
        let mut clips: Vec<Clip> = (0..MIN_GATE_CLIPS_PER_BUCKET)
            .map(|i| clip("jargon", i))
            .collect();
        let mut legal = clip("jargon", 99);
        legal.vocab = Some("legal".to_string());
        clips.push(legal);

        let message = validate_jargon_domain_coverage(&clips)
            .expect_err("one legal clip is not gate evidence")
            .to_string();
        assert!(message.contains("legal: 1/5"));
    }

    #[test]
    fn two_complete_jargon_domains_pass_coverage() {
        let mut clips: Vec<Clip> = (0..MIN_GATE_CLIPS_PER_BUCKET)
            .map(|i| clip("jargon", i))
            .collect();
        for i in 0..MIN_GATE_CLIPS_PER_BUCKET {
            let mut legal = clip("jargon", 100 + i);
            legal.vocab = Some("legal".to_string());
            clips.push(legal);
        }
        assert!(validate_jargon_domain_coverage(&clips).is_ok());
    }

    /// A single-domain jargon bucket is already covered by the per-bucket rule;
    /// this check must not double-report it.
    #[test]
    fn single_jargon_domain_is_left_to_the_bucket_rule() {
        let clips: Vec<Clip> = (0..2).map(|i| clip("jargon", i)).collect();
        assert!(validate_jargon_domain_coverage(&clips).is_ok());
    }

    #[test]
    fn default_set_exists_even_with_no_jargon_file() {
        let dir = temp_eval_dir("empty");
        let sets = load_vocab_sets(&dir).expect("sets load");

        assert!(sets.is_empty());
        let (name, set) = sets.for_clip(&clip("quiet", 1));
        assert_eq!(name, DEFAULT_VOCAB_SET);
        assert!(set.prompt.is_none());
        assert!(set.folded.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
