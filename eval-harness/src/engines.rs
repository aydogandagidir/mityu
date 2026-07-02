//! Standalone wrappers around the app's own transcription engines.
//!
//! Both engines are constructed exactly like the app's Tauri commands do
//! (`new_with_models_dir`) — just without an `AppHandle`: the models dir is
//! resolved to the same app-data locations the desktop app uses
//! (`app_data_dir()/models` for bundle id `com.bluedev.mityu`).

use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Result};
use app_lib::config::WHISPER_MODEL_CATALOG;
use app_lib::parakeet_engine::parakeet_engine::ModelStatus as ParakeetStatus;
use app_lib::parakeet_engine::ParakeetEngine;
use app_lib::whisper_engine::whisper_engine::ModelStatus as WhisperStatus;
use app_lib::whisper_engine::WhisperEngine;

pub const DEFAULT_WHISPER_MODEL: &str = "large-v3";
/// App catalog entry `("large-v3-turbo", "ggml-large-v3-turbo.bin", 1549, ...)`;
/// the in-app "Large V3 Turbo" download lands in the same models dir.
pub const DEFAULT_WHISPER_TURBO_MODEL: &str = "large-v3-turbo";
pub const DEFAULT_PARAKEET_MODEL: &str = "parakeet-tdt-0.6b-v3-int8";
/// Parakeet gets fixed windows (mirrors the app's chunked streaming usage; the
/// ONNX encoder is not intended to take 10-minute clips in one pass).
pub const PARAKEET_WINDOW_SECS: usize = 60;
const SAMPLE_RATE: usize = 16_000;
/// A short tail is merged into the previous window instead of forming a
/// degenerate final chunk.
const MIN_TAIL_SECS: usize = 5;
/// whisper.cpp's initial prompt budget is ~224 tokens; ~600 chars is a safe cap for TR.
const MAX_PROMPT_CHARS: usize = 600;

/// Model roots shared by both engines — mirrors the app: `app_data_dir()/models`
/// (bundle id `com.bluedev.mityu`), the engines' own `data_dir()/Mityu/models`
/// fallback, plus repo-local dev fallbacks.
pub fn candidate_model_roots(repo_root: &Path) -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Some(data) = dirs::data_dir() {
        v.push(data.join("com.bluedev.mityu").join("models"));
        v.push(data.join("Mityu").join("models"));
    }
    v.push(repo_root.join("frontend").join("models"));
    v.push(repo_root.join("models"));
    v.push(
        repo_root
            .join("backend")
            .join("whisper-server-package")
            .join("models"),
    );
    v
}

fn catalog_filename(name: &str) -> Option<&'static str> {
    WHISPER_MODEL_CATALOG
        .iter()
        .find(|entry| entry.0 == name)
        .map(|entry| entry.1)
}

fn catalog_names() -> String {
    WHISPER_MODEL_CATALOG
        .iter()
        .map(|entry| entry.0)
        .collect::<Vec<_>>()
        .join(", ")
}

fn missing_whisper_message(model_name: &str, searched: &[PathBuf]) -> String {
    let filename = catalog_filename(model_name).unwrap_or("ggml-<model>.bin");
    let mut msg =
        format!("Whisper modeli bulunamadı: '{model_name}' ({filename}).\nAranan konumlar:\n");
    for p in searched {
        msg.push_str(&format!("  - {}\n", p.display()));
    }
    msg.push_str(&format!(
        "Çözüm: Mityu uygulamasını açın → Settings → Transcription → '{model_name}' modelini indirin.\n\
         İndirme bittikten sonra bu komutu yeniden çalıştırın (veya --model / --model-turbo ile ggml dosya yolu verin)."
    ));
    msg
}

/// A whisper model resolved to a concrete models dir + catalog name, ready to load.
pub struct ResolvedWhisper {
    pub models_dir: PathBuf,
    pub model_name: String,
}

/// Resolve a whisper model per config: `model_arg` may be a catalog name or a
/// `ggml-*.bin` path; `None` falls back to `default_name` searched in the app
/// model dirs.
pub fn resolve_whisper(
    repo_root: &Path,
    model_arg: Option<&str>,
    default_name: &str,
) -> Result<ResolvedWhisper> {
    if let Some(arg) = model_arg {
        let p = Path::new(arg);
        if p.is_file() {
            let file_name = p
                .file_name()
                .and_then(|s| s.to_str())
                .ok_or_else(|| anyhow!("geçersiz model dosya adı: {}", p.display()))?;
            let name = WHISPER_MODEL_CATALOG
                .iter()
                .find(|entry| entry.1 == file_name)
                .map(|entry| entry.0.to_string())
                .ok_or_else(|| {
                    anyhow!(
                        "'{file_name}' uygulama kataloğundaki bir whisper dosyası değil \
                         (ör. ggml-large-v3.bin bekleniyor). Geçerli modeller: {}",
                        catalog_names()
                    )
                })?;
            let dir = p
                .parent()
                .ok_or_else(|| anyhow!("model dosyasının dizini belirlenemedi: {}", p.display()))?
                .to_path_buf();
            return Ok(ResolvedWhisper {
                models_dir: dir,
                model_name: name,
            });
        }
        if arg.contains('/') || arg.contains('\\') || arg.to_ascii_lowercase().ends_with(".bin") {
            bail!("--model / --model-turbo dosyası bulunamadı: {arg}");
        }
    }
    let name = model_arg.unwrap_or(default_name);
    let filename = catalog_filename(name).ok_or_else(|| {
        anyhow!(
            "bilinmeyen whisper modeli '{name}'. Geçerli modeller: {}",
            catalog_names()
        )
    })?;
    let candidates = candidate_model_roots(repo_root);
    for dir in &candidates {
        if dir.join(filename).is_file() {
            return Ok(ResolvedWhisper {
                models_dir: dir.clone(),
                model_name: name.to_string(),
            });
        }
    }
    let searched: Vec<PathBuf> = candidates.iter().map(|d| d.join(filename)).collect();
    bail!("{}", missing_whisper_message(name, &searched));
}

/// The app's whisper engine, loaded once and reused for all clips/configs.
pub struct WhisperRunner {
    engine: WhisperEngine,
    pub model_name: String,
}

impl WhisperRunner {
    /// Convenience for `draft`: resolve (default `large-v3`) + load.
    pub async fn load(repo_root: &Path, model_arg: Option<&str>) -> Result<Self> {
        let resolved = resolve_whisper(repo_root, model_arg, DEFAULT_WHISPER_MODEL)?;
        Self::load_resolved(resolved).await
    }

    /// Load an already-resolved whisper model. Only one whisper model should be
    /// alive at a time (a large context is multiple GB); drop the runner before
    /// loading the next one.
    pub async fn load_resolved(resolved: ResolvedWhisper) -> Result<Self> {
        let ResolvedWhisper {
            models_dir,
            model_name,
        } = resolved;
        eprintln!(
            "Whisper: '{}' yükleniyor ({}) — large modellerde bu 1 dk kadar sürebilir...",
            model_name,
            models_dir.display()
        );
        let engine = WhisperEngine::new_with_models_dir(Some(models_dir))
            .map_err(|e| anyhow!("Whisper engine oluşturulamadı: {e}"))?;
        let models = engine
            .discover_models()
            .await
            .map_err(|e| anyhow!("Whisper model taraması başarısız: {e}"))?;
        let info = models
            .into_iter()
            .find(|m| m.name == model_name)
            .ok_or_else(|| {
                anyhow!(
                    "'{model_name}' katalogda yok. Geçerli modeller: {}",
                    catalog_names()
                )
            })?;
        match &info.status {
            WhisperStatus::Available => {}
            WhisperStatus::Corrupted { .. } => bail!(
                "Whisper modeli '{model_name}' bozuk görünüyor: {}\n\
                 Çözüm: Mityu → Settings → Transcription'dan modeli silip yeniden indirin.",
                info.path.display()
            ),
            _ => bail!(
                "{}",
                missing_whisper_message(&model_name, std::slice::from_ref(&info.path))
            ),
        }
        engine
            .load_model(&model_name)
            .await
            .map_err(|e| anyhow!("Whisper modeli yüklenemedi ({model_name}): {e}"))?;
        eprintln!("Whisper hazır: {model_name}");
        Ok(Self { engine, model_name })
    }

    /// 16 kHz mono f32 in, text out. `initial_prompt` = domain vocabulary bias.
    pub async fn transcribe(
        &self,
        samples: Vec<f32>,
        language: Option<&str>,
        initial_prompt: Option<&str>,
    ) -> Result<String> {
        self.engine
            .transcribe_audio_with_prompt(
                samples,
                language.map(str::to_string),
                initial_prompt.map(str::to_string),
            )
            .await
    }
}

/// The app's Parakeet engine, loaded once and reused for all clips/configs.
pub struct ParakeetRunner {
    engine: ParakeetEngine,
    pub model_name: String,
}

impl ParakeetRunner {
    pub async fn load(repo_root: &Path, model_name: &str) -> Result<Self> {
        let candidates = candidate_model_roots(repo_root);
        let mut searched = Vec::new();
        let mut found: Option<PathBuf> = None;
        for root in &candidates {
            let dir = root.join("parakeet").join(model_name);
            if dir.join("encoder-model.int8.onnx").is_file()
                || dir.join("encoder-model.onnx").is_file()
            {
                found = Some(root.clone());
                break;
            }
            searched.push(dir);
        }
        let Some(models_root) = found else {
            let mut msg =
                format!("Parakeet modeli bulunamadı: '{model_name}'.\nAranan konumlar:\n");
            for p in &searched {
                msg.push_str(&format!("  - {}\n", p.display()));
            }
            msg.push_str(
                "Çözüm: Mityu uygulamasını açın → Settings → Transcription → Parakeet modelini indirin, sonra tekrar deneyin.",
            );
            bail!("{msg}");
        };
        eprintln!(
            "Parakeet: '{}' yükleniyor ({})...",
            model_name,
            models_root.join("parakeet").display()
        );
        let engine = ParakeetEngine::new_with_models_dir(Some(models_root))
            .map_err(|e| anyhow!("Parakeet engine oluşturulamadı: {e}"))?;
        let models = engine
            .discover_models()
            .await
            .map_err(|e| anyhow!("Parakeet model taraması başarısız: {e}"))?;
        let info = models
            .iter()
            .find(|m| m.name == model_name)
            .ok_or_else(|| {
                let known = models
                    .iter()
                    .map(|m| m.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                anyhow!("'{model_name}' uygulamanın Parakeet kataloğunda yok. Geçerli: {known}")
            })?;
        match &info.status {
            ParakeetStatus::Available => {}
            ParakeetStatus::Corrupted { .. } => bail!(
                "Parakeet modeli '{model_name}' eksik/bozuk görünüyor. \
                 Mityu → Settings → Transcription'dan silip yeniden indirin."
            ),
            _ => bail!(
                "Parakeet modeli '{model_name}' indirilmemiş. \
                 Mityu → Settings → Transcription'dan indirin."
            ),
        }
        engine
            .load_model(model_name)
            .await
            .map_err(|e| anyhow!("Parakeet modeli yüklenemedi ({model_name}): {e}"))?;
        eprintln!("Parakeet hazır: {model_name}");
        Ok(Self {
            engine,
            model_name: model_name.to_string(),
        })
    }

    /// 16 kHz mono f32 in, text out. No language / vocabulary parameters exist
    /// in the app's ort integration (multilingual model, no hotword biasing).
    pub async fn transcribe(&self, samples: &[f32]) -> Result<String> {
        let window = PARAKEET_WINDOW_SECS * SAMPLE_RATE;
        let min_tail = MIN_TAIL_SECS * SAMPLE_RATE;
        let mut parts: Vec<String> = Vec::new();
        let mut start = 0usize;
        while start < samples.len() {
            let mut end = (start + window).min(samples.len());
            if samples.len() - end < min_tail {
                end = samples.len();
            }
            let text = self
                .engine
                .transcribe_audio(samples[start..end].to_vec())
                .await
                .map_err(|e| anyhow!("Parakeet transkripsiyon hatası: {e}"))?;
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                parts.push(trimmed.to_string());
            }
            start = end;
        }
        Ok(parts.join(" "))
    }
}

/// Read eval/jargon.txt (one term per line, `#` comments allowed).
pub fn load_jargon(eval_dir: &Path) -> Result<Vec<String>> {
    let path = eval_dir.join("jargon.txt");
    if !path.is_file() {
        return Ok(Vec::new());
    }
    let text = std::fs::read_to_string(&path)
        .map_err(|e| anyhow!("jargon listesi okunamadı ({}): {e}", path.display()))?;
    Ok(text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(String::from)
        .collect())
}

/// Build the whisper `initial_prompt` from jargon terms, capped to the prompt
/// budget. Returns `(prompt, number_of_terms_used)`.
pub fn build_vocab_prompt(terms: &[String]) -> Option<(String, usize)> {
    if terms.is_empty() {
        return None;
    }
    let mut prompt = String::from("Sözlük: ");
    let mut used = 0usize;
    for term in terms {
        let clean = term.replace('\0', " ");
        let sep = if used == 0 { "" } else { ", " };
        if prompt.chars().count() + sep.len() + clean.chars().count() > MAX_PROMPT_CHARS {
            break;
        }
        prompt.push_str(sep);
        prompt.push_str(&clean);
        used += 1;
    }
    if used == 0 {
        None
    } else {
        Some((prompt, used))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vocab_prompt_caps_at_budget() {
        let terms: Vec<String> = (0..500).map(|i| format!("terim{i}")).collect();
        let (prompt, used) = build_vocab_prompt(&terms).expect("non-empty terms");
        assert!(prompt.chars().count() <= MAX_PROMPT_CHARS);
        assert!(used > 0 && used < terms.len());
        assert!(prompt.starts_with("Sözlük: terim0, terim1"));
    }

    #[test]
    fn vocab_prompt_empty_terms() {
        assert!(build_vocab_prompt(&[]).is_none());
    }
}
