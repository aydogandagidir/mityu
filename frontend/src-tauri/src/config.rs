/// Application configuration constants
///
/// Centralized definitions for default models and settings.
/// Used across database initialization, import, and retranscription.

/// Default Whisper model for transcription when no preference is configured.
/// This is the current default based on size/speed tradeoffs; it is not an
/// application-specific accuracy claim.
pub const DEFAULT_WHISPER_MODEL: &str = "large-v3-turbo";

/// Default Parakeet model for transcription when no preference is configured.
/// This is the quantized version optimized for speed.
pub const DEFAULT_PARAKEET_MODEL: &str = "parakeet-tdt-0.6b-v3-int8";

/// Whisper model catalog with metadata for all supported models.
/// Used by both WhisperEngine::discover_models() and discover_models_standalone().
///
/// Format: (name, filename, size_mb, accuracy, speed, description)
pub const WHISPER_MODEL_CATALOG: &[(&str, &str, u32, &str, &str, &str)] = &[
    // Standard f16 models (full precision)
    (
        "tiny",
        "ggml-tiny.bin",
        74,
        "Decent",
        "Very Fast",
        "Smallest model with the fastest processing",
    ),
    (
        "base",
        "ggml-base.bin",
        142,
        "Good",
        "Fast",
        "Small general-purpose model with fast processing",
    ),
    (
        "small",
        "ggml-small.bin",
        466,
        "Good",
        "Medium",
        "Mid-size model with moderate processing speed",
    ),
    (
        "medium",
        "ggml-medium.bin",
        1463,
        "High",
        "Slow",
        "Larger model with slower processing",
    ),
    (
        "large-v3-turbo",
        "ggml-large-v3-turbo.bin",
        1549,
        "High",
        "Medium",
        "Large turbo model with moderate processing speed",
    ),
    (
        "large-v3",
        "ggml-large-v3.bin",
        2951,
        "High",
        "Slow",
        "Largest supported model with slower processing",
    ),
    // Q5_1 quantized models (balanced speed/accuracy, slightly better quality than Q5_0)
    (
        "tiny-q5_1",
        "ggml-tiny-q5_1.bin",
        31,
        "Decent",
        "Very Fast",
        "Quantized tiny model, ~50% faster processing",
    ),
    (
        "base-q5_1",
        "ggml-base-q5_1.bin",
        57,
        "Good",
        "Fast",
        "Quantized base model with fast processing",
    ),
    (
        "small-q5_1",
        "ggml-small-q5_1.bin",
        181,
        "Good",
        "Fast",
        "Quantized small model, faster than f16 version",
    ),
    // Q5_0 quantized models (balanced speed/accuracy)
    (
        "medium-q5_0",
        "ggml-medium-q5_0.bin",
        514,
        "High",
        "Medium",
        "Quantized medium model with moderate processing speed",
    ),
    (
        "large-v3-turbo-q5_0",
        "ggml-large-v3-turbo-q5_0.bin",
        547,
        "High",
        "Medium",
        "Quantized large model, best balance",
    ),
    (
        "large-v3-q5_0",
        "ggml-large-v3-q5_0.bin",
        1031,
        "High",
        "Slow",
        "Quantized large model with lower storage use than full precision",
    ),
];
