// audio/transcription/nemotron_provider.rs
//
// NVIDIA Nemotron 3.5 Transcribe (`nvidia/NVIDIA-Nemotron-3.5-Transcribe-30B-A4B`)
// Speech-Audio-Language Model (SALM) provider.
//
// Supports both:
// 1. Live sliding-window chunk transcription with rolling context, speaker attribution
//    (<spk:0>..<spk:7> -> Speaker 1..Speaker 8), overlapping speech detection,
//    contextual jargon prompting, inline PII redaction, and live translation.
// 2. Post-meeting batch retranscription.

use super::provider::{TranscriptResult, TranscriptionError, TranscriptionProvider};
use crate::config::{DEFAULT_NEMOTRON_ENDPOINT, DEFAULT_NEMOTRON_MODEL};
use crate::database::repositories::speaker_turn::SpeakerTurn;
use async_trait::async_trait;
use hound::{SampleFormat, WavSpec, WavWriter};
use log::{debug, info, warn};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::io::Cursor;
use std::sync::{LazyLock, Mutex};
use std::time::Duration;

const SAMPLE_RATE_HZ: u32 = 16_000;
const MAX_ROLLING_CONTEXT_ENTRIES: usize = 3;
const MAX_ROLLING_CONTEXT_CHARS: usize = 600;
const REQUEST_TIMEOUT_SECS: u64 = 45;
const MAX_RETRY_COUNT: usize = 2;
const INITIAL_BACKOFF_MS: u64 = 350;
const MAX_SPEAKERS: u32 = 8;

/// Configuration for NVIDIA Nemotron 3.5 Transcribe.
///
/// Can be serialized as a JSON string inside `transcript_settings.model` or passed
/// as a plain model identifier string (e.g. `"nvidia/NVIDIA-Nemotron-3.5-Transcribe-30B-A4B"`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct NemotronConfig {
    pub model: String,
    pub endpoint: String,
    #[serde(default = "default_true", alias = "speaker_attribution")]
    pub enable_diarization: bool,
    #[serde(default, alias = "inline_pii_redaction")]
    pub enable_pii_redaction: bool,
    #[serde(default, alias = "context_prompt")]
    pub context_terms: String,
    #[serde(default, alias = "target_language")]
    pub translate_to: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
}

fn default_true() -> bool {
    true
}

impl Default for NemotronConfig {
    fn default() -> Self {
        Self {
            model: DEFAULT_NEMOTRON_MODEL.to_string(),
            endpoint: DEFAULT_NEMOTRON_ENDPOINT.to_string(),
            enable_diarization: true,
            enable_pii_redaction: false,
            context_terms: String::new(),
            translate_to: None,
            api_key: None,
        }
    }
}

static LIVE_SPEAKER_TURNS: LazyLock<Mutex<Vec<SpeakerTurn>>> =
    LazyLock::new(|| Mutex::new(Vec::new()));

static SHARED_NEMOTRON_HTTP_CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
        .pool_max_idle_per_host(8)
        .pool_idle_timeout(Duration::from_secs(90))
        .tcp_nodelay(true)
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
});

/// Clear accumulated live speaker turns at the start of a recording session.
pub fn clear_live_speaker_turns() {
    if let Ok(mut guard) = LIVE_SPEAKER_TURNS.lock() {
        guard.clear();
    }
}

/// Append live speaker turns produced during a recording chunk.
pub fn append_live_speaker_turns(turns: &[SpeakerTurn]) {
    if turns.is_empty() {
        return;
    }
    if let Ok(mut guard) = LIVE_SPEAKER_TURNS.lock() {
        guard.extend_from_slice(turns);
    }
}

/// Drain and return all accumulated live speaker turns when saving a meeting.
pub fn take_live_speaker_turns() -> Vec<SpeakerTurn> {
    if let Ok(mut guard) = LIVE_SPEAKER_TURNS.lock() {
        std::mem::take(&mut *guard)
    } else {
        Vec::new()
    }
}

impl NemotronConfig {
    /// Alias for `from_model_field` used across engine and retranscription pipelines.
    pub fn from_model_string(raw: &str) -> Self {
        Self::from_model_field(raw)
    }

    /// Parse `NemotronConfig` from the `model` column in `transcript_settings`.
    /// Accepts either a JSON object string or a plain model name.
    pub fn from_model_field(raw: &str) -> Self {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Self::default();
        }
        if trimmed.starts_with('{') {
            if let Ok(mut parsed) = serde_json::from_str::<NemotronConfig>(trimmed) {
                if parsed.model.trim().is_empty() {
                    parsed.model = DEFAULT_NEMOTRON_MODEL.to_string();
                }
                if parsed.endpoint.trim().is_empty() {
                    parsed.endpoint = DEFAULT_NEMOTRON_ENDPOINT.to_string();
                }
                parsed.translate_to = parsed
                    .translate_to
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty() && s != "none");
                return parsed;
            }
        }
        Self {
            model: trimmed.to_string(),
            ..Self::default()
        }
    }

    /// Validate and normalize the OpenAI-compatible `/v1` base URL.
    pub fn normalized_chat_completions_url(&self) -> Result<String, TranscriptionError> {
        let trimmed = self.endpoint.trim().trim_end_matches('/');
        if !trimmed.starts_with("http://") && !trimmed.starts_with("https://") {
            return Err(TranscriptionError::EngineFailed(format!(
                "Invalid Nemotron endpoint '{}': must start with http:// or https://",
                self.endpoint
            )));
        }
        if trimmed.ends_with("/chat/completions") {
            Ok(trimmed.to_string())
        } else if trimmed.ends_with("/v1") {
            Ok(format!("{}/chat/completions", trimmed))
        } else {
            Ok(format!("{}/v1/chat/completions", trimmed))
        }
    }
}

/// Encode 16kHz mono f32 PCM samples into an in-memory 16-bit PCM WAV data URI.
pub fn encode_pcm16_wav_data_uri(
    samples: &[f32],
    sample_rate: u32,
) -> Result<String, TranscriptionError> {
    let spec = WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };

    let mut cursor = Cursor::new(Vec::with_capacity(44 + samples.len() * 2));
    {
        let mut writer = WavWriter::new(&mut cursor, spec).map_err(|e| {
            TranscriptionError::EngineFailed(format!("Failed to initialize WAV writer: {}", e))
        })?;
        for &sample in samples {
            let clamped = sample.clamp(-1.0, 1.0);
            let pcm16 = (clamped * i16::MAX as f32) as i16;
            writer.write_sample(pcm16).map_err(|e| {
                TranscriptionError::EngineFailed(format!("Failed to write PCM16 sample: {}", e))
            })?;
        }
        writer.finalize().map_err(|e| {
            TranscriptionError::EngineFailed(format!("Failed to finalize WAV buffer: {}", e))
        })?;
    }

    let wav_bytes = cursor.into_inner();
    const PREFIX: &str = "data:audio/wav;base64,";
    let mut uri = String::with_capacity(PREFIX.len() + ((wav_bytes.len() + 2) / 3) * 4);
    uri.push_str(PREFIX);
    encode_base64_into(&wav_bytes, &mut uri);
    Ok(uri)
}

/// Pure RFC 4648 standard Base64 encoder (avoids adding external crate dependencies).
pub fn encode_base64(input: &[u8]) -> String {
    let mut out = String::with_capacity(((input.len() + 2) / 3) * 4);
    encode_base64_into(input, &mut out);
    out
}

fn encode_base64_into(input: &[u8], out: &mut String) {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };

        let triple = (b0 << 16) | (b1 << 8) | b2;

        out.push(TABLE[((triple >> 18) & 0x3F) as usize] as char);
        out.push(TABLE[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            out.push(TABLE[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(TABLE[(triple & 0x3F) as usize] as char);
        } else {
            out.push('=');
        }
    }
}

/// Build the structured instruction prompt for NVIDIA Nemotron 3.5 Transcribe.
pub fn build_nemotron_prompt(
    config: &NemotronConfig,
    language: Option<&str>,
    rolling_context: &[String],
) -> String {
    let mut directives: Vec<String> = Vec::new();

    if let Some(target_lang) = config
        .translate_to
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        directives.push(format!(
            "Transcribe and translate the speech directly into {}.",
            target_lang
        ));
    } else if let Some(lang) = language
        .map(str::trim)
        .filter(|l| !l.is_empty() && *l != "auto" && *l != "auto-translate")
    {
        directives.push(format!(
            "Transcribe the speech accurately in language '{}'.",
            lang
        ));
    } else {
        directives.push("Transcribe the speech accurately in its spoken language.".to_string());
    }

    if config.enable_diarization {
        directives.push(
            "Enable speaker attribution for up to 8 speakers using <spk:0> through <spk:7> tags. \
             When multiple speakers talk at the same time (overlapping speech), include each speaker's \
             turn with [MM:SS.mmm - MM:SS.mmm] <spk:N> timestamps so overlapping speech is preserved."
                .to_string(),
        );
    }

    let terms = config.context_terms.trim();
    if !terms.is_empty() {
        directives.push(format!(
            "Contextual vocabulary, domain jargon, and proper nouns to recognize accurately: {}.",
            terms
        ));
    }

    if config.enable_pii_redaction {
        directives.push(
            "Redact any Personally Identifiable Information (PII) such as credit card numbers, \
             national ID/SSN numbers, phone numbers, email addresses, or secret keys with [REDACTED]."
                .to_string(),
        );
    }

    if !rolling_context.is_empty() {
        let joined = rolling_context.join(" | ");
        directives.push(format!(
            "Previous audio chunk context (keep <spk:N> speaker identities consistent with this history, \
             do NOT repeat this previous text in your output): \"{}\"",
            joined
        ));
    }

    directives.push("Output only the transcript without extra commentary.".to_string());
    directives.join("\n")
}

static TIMESTAMPED_SPK_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?m)^\s*\[(?P<start>\d{1,2}:\d{2}(?:\.\d{1,3})?|\d+(?:\.\d+)?s?)\s*[-–>to]+\s*(?P<end>\d{1,2}:\d{2}(?:\.\d{1,3})?|\d+(?:\.\d+)?s?)\]\s*(?:<spk:(?P<spk>\d+)>|Speaker\s*(?P<spk_num>\d+)\s*:?)\s*(?P<text>.+?)\s*$",
    )
    .expect("valid timestamped speaker regex")
});

static INLINE_SPK_TAG_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(?:<spk:(?P<spk>\d+)>|(?:^|\n)\s*Speaker\s+(?P<spk_num>\d+)\s*:)")
        .expect("valid inline speaker tag regex")
});

static OVERLAP_MARKER_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)(?:<overlap>|\[overlap\])").expect("valid overlap regex"));

fn parse_time_to_ms(raw: &str) -> Option<i64> {
    let cleaned = raw.trim().trim_end_matches('s');
    if let Some((mins, secs)) = cleaned.split_once(':') {
        let m: f64 = mins.trim().parse().ok()?;
        let s: f64 = secs.trim().parse().ok()?;
        Some(((m * 60.0 + s) * 1000.0).round() as i64)
    } else {
        let s: f64 = cleaned.parse().ok()?;
        Some((s * 1000.0).round() as i64)
    }
}

/// Normalize a 0-based `<spk:N>` or 1-based `Speaker N` index into Mityu's
/// ADR-0034 anonymous speaker label (`"Speaker 1"`..`"Speaker 8"`).
fn normalize_speaker_label(zero_based_spk: Option<&str>, one_based_spk: Option<&str>) -> String {
    let speaker_number = if let Some(z) = zero_based_spk.and_then(|s| s.parse::<u32>().ok()) {
        (z + 1).clamp(1, MAX_SPEAKERS)
    } else if let Some(o) = one_based_spk.and_then(|s| s.parse::<u32>().ok()) {
        o.clamp(1, MAX_SPEAKERS)
    } else {
        1
    };
    format!("Speaker {}", speaker_number)
}

/// Parse raw Nemotron 3.5 Transcribe output into:
/// 1. Clean display text for the transcript row.
/// 2. Chunk-relative `SpeakerTurn` items (`0..=chunk_duration_ms`).
///
/// The caller shifts `start_ms` and `end_ms` by the chunk's recording offset.
pub fn parse_nemotron_output(raw: &str, chunk_duration_sec: f64) -> (String, Vec<SpeakerTurn>) {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return (String::new(), Vec::new());
    }

    let chunk_duration_ms = ((chunk_duration_sec.max(0.2)) * 1000.0).round() as i64;

    // 1. Check for timestamped speaker lines first:
    //    `[00:00.200 - 00:02.400] <spk:0> Text`
    let mut timestamped_turns: Vec<SpeakerTurn> = Vec::new();
    let mut cleaned_parts: Vec<String> = Vec::new();
    let mut distinct_labels: Vec<String> = Vec::new();

    for caps in TIMESTAMPED_SPK_RE.captures_iter(trimmed) {
        let start_raw = caps.name("start").map(|m| m.as_str()).unwrap_or("0");
        let end_raw = caps.name("end").map(|m| m.as_str()).unwrap_or("0");
        let label = normalize_speaker_label(
            caps.name("spk").map(|m| m.as_str()),
            caps.name("spk_num").map(|m| m.as_str()),
        );
        let raw_text = caps.name("text").map(|m| m.as_str()).unwrap_or("").trim();
        let text = OVERLAP_MARKER_RE
            .replace_all(raw_text, "")
            .trim()
            .to_string();
        if text.is_empty() {
            continue;
        }

        let start_ms = parse_time_to_ms(start_raw).unwrap_or(0).max(0);
        let mut end_ms = parse_time_to_ms(end_raw).unwrap_or(chunk_duration_ms);
        if end_ms <= start_ms {
            end_ms = (start_ms + 500).min(chunk_duration_ms.max(start_ms + 500));
        }

        if !distinct_labels.contains(&label) {
            distinct_labels.push(label.clone());
        }
        timestamped_turns.push(SpeakerTurn {
            start_ms,
            end_ms,
            speaker_label: label,
            confidence: Some(0.94),
        });
        cleaned_parts.push(text);
    }

    if !timestamped_turns.is_empty() {
        let formatted_text = if distinct_labels.len() > 1 {
            timestamped_turns
                .iter()
                .zip(cleaned_parts.iter())
                .map(|(turn, text)| format!("[{}] {}", turn.speaker_label, text))
                .collect::<Vec<_>>()
                .join(" ")
        } else {
            cleaned_parts.join(" ")
        };
        return (formatted_text, timestamped_turns);
    }

    // 2. Check for inline `<spk:0> ... <spk:1> ...` or `Speaker 1: ...` tags without timestamps
    let matches: Vec<_> = INLINE_SPK_TAG_RE.find_iter(trimmed).collect();
    if !matches.is_empty() {
        let has_explicit_overlap = OVERLAP_MARKER_RE.is_match(trimmed);
        let mut segments: Vec<(String, String)> = Vec::new();

        for (idx, mat) in matches.iter().enumerate() {
            let caps = INLINE_SPK_TAG_RE
                .captures(mat.as_str())
                .expect("capture matched regex");
            let label = normalize_speaker_label(
                caps.name("spk").map(|m| m.as_str()),
                caps.name("spk_num").map(|m| m.as_str()),
            );
            let content_start = mat.end();
            let content_end = matches
                .get(idx + 1)
                .map(|next| next.start())
                .unwrap_or(trimmed.len());
            let slice = trimmed[content_start..content_end].trim();
            let cleaned_slice = OVERLAP_MARKER_RE.replace_all(slice, "").trim().to_string();
            if !cleaned_slice.is_empty() {
                if !distinct_labels.contains(&label) {
                    distinct_labels.push(label.clone());
                }
                segments.push((label, cleaned_slice));
            }
        }

        if !segments.is_empty() {
            let total_chars: usize = segments.iter().map(|(_, t)| t.chars().count().max(1)).sum();
            let mut cursor_ms: i64 = 0;
            let mut turns: Vec<SpeakerTurn> = Vec::with_capacity(segments.len());

            for (idx, (label, text)) in segments.iter().enumerate() {
                let (start_ms, end_ms) = if has_explicit_overlap && segments.len() > 1 {
                    // Explicit crosstalk/overlap marker: overlap speaker intervals across the chunk
                    (0, chunk_duration_ms.max(200))
                } else {
                    let weight = text.chars().count().max(1) as f64 / total_chars.max(1) as f64;
                    let span_ms = ((chunk_duration_ms as f64) * weight).round() as i64;
                    let start = cursor_ms;
                    let end = if idx + 1 == segments.len() {
                        chunk_duration_ms.max(start + 100)
                    } else {
                        (start + span_ms.max(100)).min(chunk_duration_ms.max(start + 100))
                    };
                    cursor_ms = end;
                    (start, end)
                };

                turns.push(SpeakerTurn {
                    start_ms,
                    end_ms: end_ms.max(start_ms + 50),
                    speaker_label: label.clone(),
                    confidence: Some(0.92),
                });
            }

            let formatted_text = if distinct_labels.len() > 1 {
                segments
                    .iter()
                    .map(|(label, text)| format!("[{}] {}", label, text))
                    .collect::<Vec<_>>()
                    .join(" ")
            } else {
                segments
                    .into_iter()
                    .map(|(_, text)| text)
                    .collect::<Vec<_>>()
                    .join(" ")
            };

            return (formatted_text, turns);
        }
    }

    // 3. Plain transcript without speaker tags
    let clean = OVERLAP_MARKER_RE
        .replace_all(trimmed, "")
        .trim()
        .to_string();
    (clean, Vec::new())
}

/// Shift chunk-relative speaker turns by `chunk_offset_ms` so their timestamps
/// are relative to the start of the meeting recording.
pub fn offset_speaker_turns(turns: &[SpeakerTurn], chunk_offset_ms: i64) -> Vec<SpeakerTurn> {
    turns
        .iter()
        .map(|t| {
            let start_ms = (t.start_ms + chunk_offset_ms).max(0);
            let end_ms = (t.end_ms + chunk_offset_ms).max(start_ms + 50);
            SpeakerTurn {
                start_ms,
                end_ms,
                speaker_label: t.speaker_label.clone(),
                confidence: t.confidence,
            }
        })
        .collect()
}

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatMessageContent,
}

#[derive(Debug, Deserialize)]
struct ChatMessageContent {
    content: Option<String>,
}

/// NVIDIA Nemotron 3.5 Transcribe SALM Provider
pub struct NemotronTranscribeProvider {
    config: NemotronConfig,
    api_key: Option<String>,
    client: reqwest::Client,
    rolling_context: Mutex<VecDeque<String>>,
}

impl NemotronTranscribeProvider {
    pub fn new(config: NemotronConfig) -> Result<Self, String> {
        let client = SHARED_NEMOTRON_HTTP_CLIENT.clone();

        let api_key = config
            .api_key
            .clone()
            .map(|k| k.trim().to_string())
            .filter(|k| !k.is_empty());

        Ok(Self {
            config,
            api_key,
            client,
            rolling_context: Mutex::new(VecDeque::with_capacity(MAX_ROLLING_CONTEXT_ENTRIES)),
        })
    }

    pub fn config(&self) -> &NemotronConfig {
        &self.config
    }

    fn snapshot_rolling_context(&self) -> Vec<String> {
        if let Ok(guard) = self.rolling_context.lock() {
            guard.iter().cloned().collect()
        } else {
            Vec::new()
        }
    }

    fn push_rolling_context(&self, raw_chunk_output: &str) {
        let trimmed = raw_chunk_output.trim();
        if trimmed.is_empty() {
            return;
        }
        if let Ok(mut guard) = self.rolling_context.lock() {
            let mut snippet = trimmed.to_string();
            if snippet.len() > 240 {
                let mut cut = 240;
                while !snippet.is_char_boundary(cut) {
                    cut -= 1;
                }
                snippet.truncate(cut);
            }
            guard.push_back(snippet);
            while guard.len() > MAX_ROLLING_CONTEXT_ENTRIES {
                guard.pop_front();
            }
            while guard.iter().map(|s| s.len()).sum::<usize>() > MAX_ROLLING_CONTEXT_CHARS
                && guard.len() > 1
            {
                guard.pop_front();
            }
        }
    }
}

#[async_trait]
impl TranscriptionProvider for NemotronTranscribeProvider {
    async fn transcribe(
        &self,
        audio: Vec<f32>,
        language: Option<String>,
    ) -> Result<TranscriptResult, TranscriptionError> {
        if audio.len() < 1600 {
            return Err(TranscriptionError::AudioTooShort {
                samples: audio.len(),
                minimum: 1600,
            });
        }

        let chunk_duration_sec = audio.len() as f64 / SAMPLE_RATE_HZ as f64;
        let wav_data_uri = encode_pcm16_wav_data_uri(&audio, SAMPLE_RATE_HZ)?;
        let context_history = self.snapshot_rolling_context();
        let prompt = build_nemotron_prompt(&self.config, language.as_deref(), &context_history);
        let url = self.config.normalized_chat_completions_url()?;

        let payload = serde_json::json!({
            "model": self.config.model,
            "temperature": 0.0,
            "max_tokens": 1024,
            "messages": [
                {
                    "role": "user",
                    "content": [
                        {
                            "type": "audio_url",
                            "audio_url": {
                                "url": wav_data_uri
                            }
                        },
                        {
                            "type": "text",
                            "text": prompt
                        }
                    ]
                }
            ]
        });

        let mut last_err: Option<String> = None;
        for attempt in 0..=MAX_RETRY_COUNT {
            if attempt > 0 {
                let backoff = INITIAL_BACKOFF_MS * (1_u64 << (attempt - 1));
                tokio::time::sleep(Duration::from_millis(backoff)).await;
            }

            let mut req = self.client.post(&url).json(&payload);
            if let Some(ref key) = self.api_key {
                req = req.bearer_auth(key);
            }

            match req.send().await {
                Ok(resp) => {
                    let status = resp.status();
                    if status.is_success() {
                        let body: ChatCompletionResponse = resp.json().await.map_err(|e| {
                            TranscriptionError::EngineFailed(format!(
                                "Failed to parse Nemotron response JSON: {}",
                                e
                            ))
                        })?;

                        let raw_text = body
                            .choices
                            .into_iter()
                            .next()
                            .and_then(|c| c.message.content)
                            .unwrap_or_default();

                        self.push_rolling_context(&raw_text);
                        let (clean_text, speaker_turns) =
                            parse_nemotron_output(&raw_text, chunk_duration_sec);

                        debug!(
                            "Nemotron transcribed {:.1}s chunk -> {} chars, {} speaker turn(s)",
                            chunk_duration_sec,
                            clean_text.len(),
                            speaker_turns.len()
                        );

                        return Ok(TranscriptResult {
                            text: clean_text,
                            confidence: Some(0.94),
                            is_partial: false,
                            speaker_turns,
                        });
                    } else if status.as_u16() == 502
                        || status.as_u16() == 503
                        || status.as_u16() == 504
                        || status.as_u16() == 429
                    {
                        let body_text = resp.text().await.unwrap_or_default();
                        warn!(
                            "Transient Nemotron HTTP {} on attempt {}/{}: {}",
                            status,
                            attempt + 1,
                            MAX_RETRY_COUNT + 1,
                            body_text
                        );
                        last_err = Some(format!("HTTP {}: {}", status, body_text));
                        continue;
                    } else {
                        let body_text = resp.text().await.unwrap_or_default();
                        return Err(TranscriptionError::EngineFailed(format!(
                            "Nemotron endpoint returned HTTP {}: {}",
                            status, body_text
                        )));
                    }
                }
                Err(e) => {
                    warn!(
                        "Nemotron request error on attempt {}/{}: {}",
                        attempt + 1,
                        MAX_RETRY_COUNT + 1,
                        e
                    );
                    last_err = Some(e.to_string());
                }
            }
        }

        info!(
            "Nemotron transcription failed after {} attempts",
            MAX_RETRY_COUNT + 1
        );
        Err(TranscriptionError::EngineFailed(format!(
            "Nemotron endpoint unreachable ({}): {}",
            url,
            last_err.unwrap_or_else(|| "unknown transport error".to_string())
        )))
    }

    async fn is_model_loaded(&self) -> bool {
        !self.config.model.trim().is_empty() && !self.config.endpoint.trim().is_empty()
    }

    async fn get_current_model(&self) -> Option<String> {
        Some(self.config.model.clone())
    }

    fn provider_name(&self) -> &'static str {
        "NemotronTranscribe"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nemotron_config_from_plain_and_json() {
        let default_cfg = NemotronConfig::from_model_field("");
        assert_eq!(default_cfg.model, DEFAULT_NEMOTRON_MODEL);
        assert_eq!(default_cfg.endpoint, DEFAULT_NEMOTRON_ENDPOINT);
        assert!(default_cfg.enable_diarization);
        assert!(!default_cfg.enable_pii_redaction);

        let plain_cfg = NemotronConfig::from_model_field(
            "nvidia/NVIDIA-Nemotron-3.5-Transcribe-30B-A4B-DFlash2",
        );
        assert_eq!(
            plain_cfg.model,
            "nvidia/NVIDIA-Nemotron-3.5-Transcribe-30B-A4B-DFlash2"
        );
        assert!(plain_cfg.enable_diarization);

        let json_str = r#"{
            "model": "nvidia/NVIDIA-Nemotron-3.5-Transcribe-30B-A4B",
            "endpoint": "http://localhost:8000/v1/",
            "enableDiarization": true,
            "enablePiiRedaction": true,
            "contextTerms": "Mityu, Sortformer, Kubernetes, ARR",
            "translateTo": "Turkish"
        }"#;
        let parsed = NemotronConfig::from_model_field(json_str);
        assert_eq!(
            parsed.normalized_chat_completions_url().unwrap(),
            "http://localhost:8000/v1/chat/completions"
        );
        assert!(parsed.enable_diarization);
        assert!(parsed.enable_pii_redaction);
        assert_eq!(parsed.context_terms, "Mityu, Sortformer, Kubernetes, ARR");
        assert_eq!(parsed.translate_to.as_deref(), Some("Turkish"));
    }

    #[test]
    fn test_encode_pcm16_wav_data_uri_produces_valid_wav_header() {
        let samples = vec![0.0_f32, 0.25, -0.25, 0.5, -0.5, 0.99, -0.99];
        let uri = encode_pcm16_wav_data_uri(&samples, 16_000).expect("encode wav");
        assert!(uri.starts_with("data:audio/wav;base64,UklGR")); // "RIFF" in base64 is "UklGR"
    }

    #[test]
    fn test_parse_nemotron_output_timestamped_overlapping_speakers() {
        let raw = "\
[00:00.100 - 00:02.600] <spk:0> Merhaba, Q3 bütçesini konuşalım.
[00:01.800 - 00:03.900] <spk:1> Evet, hemen tabloyu paylaşıyorum.";
        let (clean_text, turns) = parse_nemotron_output(raw, 4.0);

        assert_eq!(turns.len(), 2);
        assert_eq!(turns[0].speaker_label, "Speaker 1");
        assert_eq!(turns[0].start_ms, 100);
        assert_eq!(turns[0].end_ms, 2600);
        assert_eq!(turns[1].speaker_label, "Speaker 2");
        assert_eq!(turns[1].start_ms, 1800);
        assert_eq!(turns[1].end_ms, 3900);
        // Verify overlap is preserved (1800 < 2600)
        assert!(turns[1].start_ms < turns[0].end_ms);
        assert!(clean_text.contains("[Speaker 1] Merhaba, Q3 bütçesini konuşalım."));
        assert!(clean_text.contains("[Speaker 2] Evet, hemen tabloyu paylaşıyorum."));
    }

    #[test]
    fn test_parse_nemotron_output_inline_up_to_8_speakers() {
        let raw = "<spk:0> Birinci konuşmacı. <spk:7> Sekizinci konuşmacı.";
        let (clean_text, turns) = parse_nemotron_output(raw, 3.0);

        assert_eq!(turns.len(), 2);
        assert_eq!(turns[0].speaker_label, "Speaker 1");
        assert_eq!(turns[1].speaker_label, "Speaker 8");
        assert!(turns[0].end_ms > turns[0].start_ms);
        assert!(turns[1].end_ms > turns[1].start_ms);
        assert!(clean_text.contains("[Speaker 1]"));
        assert!(clean_text.contains("[Speaker 8]"));
    }

    #[test]
    fn test_build_nemotron_prompt_includes_context_pii_and_translation() {
        let cfg = NemotronConfig {
            model: DEFAULT_NEMOTRON_MODEL.to_string(),
            endpoint: DEFAULT_NEMOTRON_ENDPOINT.to_string(),
            enable_diarization: true,
            enable_pii_redaction: true,
            context_terms: "Mityu, Cluely, Nemotron".to_string(),
            translate_to: Some("English".to_string()),
            api_key: None,
        };
        let history = vec!["<spk:0> Merhaba".to_string()];
        let prompt = build_nemotron_prompt(&cfg, Some("tr"), &history);

        assert!(prompt.contains("translate the speech directly into English"));
        assert!(prompt.contains("<spk:0> through <spk:7>"));
        assert!(prompt.contains("Mityu, Cluely, Nemotron"));
        assert!(prompt.contains("[REDACTED]"));
        assert!(prompt.contains("<spk:0> Merhaba"));
    }
}
