/**
 * Default model names for transcription engines.
 * IMPORTANT: Keep in sync with Rust constants in src-tauri/src/config.rs
 */

/**
 * Default Whisper model for transcription when no preference is configured.
 * This is the recommended balance of accuracy and speed.
 */
export const DEFAULT_WHISPER_MODEL = 'large-v3-turbo';

/**
 * Default Parakeet model for transcription when no preference is configured.
 * This is the quantized version optimized for speed.
 */
export const DEFAULT_PARAKEET_MODEL = 'parakeet-tdt-0.6b-v3-int8';

/**
 * Default NVIDIA Nemotron 3.5 Transcribe 30B-A4B MoE SALM model.
 */
export const DEFAULT_NEMOTRON_MODEL = 'nvidia/NVIDIA-Nemotron-3.5-Transcribe-30B-A4B';

/**
 * Default local OpenAI-compatible vLLM / NIM endpoint for Nemotron 3.5 Transcribe.
 */
export const DEFAULT_NEMOTRON_ENDPOINT = 'http://127.0.0.1:8000/v1';

/**
 * Model defaults by provider type
 */
export const MODEL_DEFAULTS = {
  whisper: DEFAULT_WHISPER_MODEL,
  localWhisper: DEFAULT_WHISPER_MODEL,
  parakeet: DEFAULT_PARAKEET_MODEL,
  nemotron: DEFAULT_NEMOTRON_MODEL,
} as const;

/**
 * Transcription provider used when nothing has been saved yet. ConfigContext and the
 * Settings page both fall back through `resolveTranscriptDefaults`, so opening Settings
 * can no longer swap an unsaved Parakeet default for Whisper (v1.2.5 and earlier).
 */
export const DEFAULT_TRANSCRIPT_PROVIDER = 'parakeet' as const;

/**
 * Fill a possibly-empty saved provider/model pair with the shared defaults. The model
 * default follows the PROVIDER, so a saved `localWhisper` with no model gets a Whisper
 * model rather than the Parakeet one.
 */
export function resolveTranscriptDefaults(
  provider: string | null | undefined,
  model: string | null | undefined,
): { provider: string; model: string } {
  const resolvedProvider = provider || DEFAULT_TRANSCRIPT_PROVIDER;
  const providerDefault =
    (MODEL_DEFAULTS as Record<string, string>)[resolvedProvider] ?? DEFAULT_PARAKEET_MODEL;
  return { provider: resolvedProvider, model: model || providerDefault };
}
