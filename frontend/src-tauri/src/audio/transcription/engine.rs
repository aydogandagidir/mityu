// audio/transcription/engine.rs
//
// TranscriptionEngine enum and model initialization/validation logic.

use super::provider::TranscriptionProvider;
use log::{info, warn};
use serde::Serialize;
use std::sync::Arc;
use tauri::{AppHandle, Manager, Runtime};

// ============================================================================
// TRANSCRIPTION ENGINE ENUM
// ============================================================================

// Transcription engine abstraction to support multiple providers
pub enum TranscriptionEngine {
    Whisper(Arc<crate::whisper_engine::WhisperEngine>), // Direct access (backward compat)
    Parakeet(Arc<crate::parakeet_engine::ParakeetEngine>), // Direct access (backward compat)
    Provider(Arc<dyn TranscriptionProvider>),           // Trait-based (preferred for new code)
}

impl TranscriptionEngine {
    /// Check if the engine has a model loaded
    pub async fn is_model_loaded(&self) -> bool {
        match self {
            Self::Whisper(engine) => engine.is_model_loaded().await,
            Self::Parakeet(engine) => engine.is_model_loaded().await,
            Self::Provider(provider) => provider.is_model_loaded().await,
        }
    }

    /// Get the current model name
    pub async fn get_current_model(&self) -> Option<String> {
        match self {
            Self::Whisper(engine) => engine.get_current_model().await,
            Self::Parakeet(engine) => engine.get_current_model().await,
            Self::Provider(provider) => provider.get_current_model().await,
        }
    }

    /// Get the provider name for logging
    pub fn provider_name(&self) -> &str {
        match self {
            Self::Whisper(_) => "Whisper (direct)",
            Self::Parakeet(_) => "Parakeet (direct)",
            Self::Provider(provider) => provider.provider_name(),
        }
    }
}

// ============================================================================
// PRE-FLIGHT READINESS (the answer the UI asks before offering to record)
// ============================================================================

/// Can the transcription engine the user ACTUALLY configured record right now?
///
/// This exists because the renderer used to answer that question itself, and
/// answered it wrong. `useRecordingStart` called `parakeet_has_available_models`
/// on every start path regardless of the configured provider, so a user on
/// `localWhisper` with no Parakeet model was told "Transcription model not
/// ready" and could never start a recording — while
/// [`validate_transcription_model_ready`], three lines further down the same
/// path, would have accepted their Whisper model happily.
///
/// The rule the repo already follows elsewhere (ADR-0042: no validation
/// mirrored in TypeScript) is the fix: **which engine is configured is decided
/// in exactly one place, here.** The renderer asks and renders the answer.
///
/// Deliberately CHEAP: it lists models, it does not initialise or load one.
/// The authoritative check stays in [`validate_transcription_model_ready`] at
/// the moment recording actually starts; this only decides whether the UI
/// should offer the button and what to say when it should not.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptionReadiness {
    /// May a recording be started with the configured engine?
    pub ready: bool,
    /// The provider that was actually consulted — never assumed. The UI shows
    /// it so a refusal names the engine the user chose, not a different one.
    pub provider: String,
    /// A model for THAT provider is downloading: "wait", not "go and get one".
    pub downloading: bool,
    /// Why not ready, in a sentence the UI renders verbatim rather than
    /// composing its own (which is how the wording drifted from the rule).
    pub reason: Option<String>,
}

/// What each local engine has on disk. Both are read, so the decision function
/// is total and testable without a running app.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct EngineModels {
    pub whisper_available: bool,
    pub whisper_downloading: bool,
    pub parakeet_available: bool,
    pub parakeet_downloading: bool,
}

/// The decision, separated from the I/O so the engine-blindness bug is
/// expressible as a unit test.
pub(crate) fn decide_readiness(provider: &str, models: &EngineModels) -> TranscriptionReadiness {
    let (available, downloading) = match provider {
        "localWhisper" => (models.whisper_available, models.whisper_downloading),
        "parakeet" => (models.parakeet_available, models.parakeet_downloading),
        other => {
            return TranscriptionReadiness {
                ready: false,
                provider: other.to_string(),
                downloading: false,
                reason: Some(format!(
                    "'{other}' cannot transcribe on this device. Choose Local Whisper or Parakeet in Settings."
                )),
            };
        }
    };

    let engine_label = if provider == "localWhisper" {
        "Local Whisper"
    } else {
        "Parakeet"
    };

    if available {
        return TranscriptionReadiness {
            ready: true,
            provider: provider.to_string(),
            downloading,
            reason: None,
        };
    }

    // Downloading is reported even when a model is also missing: "wait" and
    // "go and fetch one" are different instructions and the user is owed the
    // one that is true.
    let reason = if downloading {
        format!(
            "The {engine_label} model is still downloading. Recording can start once it finishes."
        )
    } else {
        format!(
            "No {engine_label} model is installed yet. Download one in Settings before recording."
        )
    };

    TranscriptionReadiness {
        ready: false,
        provider: provider.to_string(),
        downloading,
        reason: Some(reason),
    }
}

/// Read both engines' model lists. Never fails: an engine that is not
/// initialised, or whose directory cannot be read, reports nothing rather than
/// taking the whole check down — the authoritative validation still runs when
/// recording starts.
async fn read_engine_models() -> EngineModels {
    use crate::parakeet_engine::ModelStatus as ParakeetStatus;
    use crate::whisper_engine::ModelStatus as WhisperStatus;

    let mut models = EngineModels::default();

    match crate::whisper_engine::commands::whisper_get_available_models().await {
        Ok(list) => {
            models.whisper_available = list
                .iter()
                .any(|m| matches!(m.status, WhisperStatus::Available));
            models.whisper_downloading = list
                .iter()
                .any(|m| matches!(m.status, WhisperStatus::Downloading { .. }));
        }
        Err(e) => info!("readiness: Whisper models unreadable ({e})"),
    }

    match crate::parakeet_engine::commands::parakeet_get_available_models().await {
        Ok(list) => {
            models.parakeet_available = list
                .iter()
                .any(|m| matches!(m.status, ParakeetStatus::Available));
            models.parakeet_downloading = list
                .iter()
                .any(|m| matches!(m.status, ParakeetStatus::Downloading { .. }));
        }
        Err(e) => info!("readiness: Parakeet models unreadable ({e})"),
    }

    models
}

/// The pre-flight the UI calls before offering to record.
#[tauri::command]
pub async fn api_transcription_readiness<R: Runtime>(app: AppHandle<R>) -> TranscriptionReadiness {
    let provider = configured_provider(&app).await;
    let models = read_engine_models().await;
    let readiness = decide_readiness(&provider, &models);
    info!(
        "🔍 transcription readiness: provider={} ready={} downloading={}",
        readiness.provider, readiness.ready, readiness.downloading
    );
    readiness
}

/// The configured transcription provider, with the same default and the same
/// failure behaviour [`validate_transcription_model_ready`] uses, so the
/// pre-flight and the authoritative check can never disagree about which
/// engine is in play.
async fn configured_provider<R: Runtime>(app: &AppHandle<R>) -> String {
    match crate::api::api::api_get_transcript_config(app.clone(), app.clone().state(), None).await {
        Ok(Some(config)) => config.provider,
        Ok(None) => "parakeet".to_string(),
        Err(e) => {
            warn!("⚠️ readiness: transcript config unreadable ({e}); defaulting to parakeet");
            "parakeet".to_string()
        }
    }
}

// ============================================================================
// MODEL VALIDATION AND INITIALIZATION
// ============================================================================

/// Validate that transcription models (Whisper or Parakeet) are ready before starting recording
pub async fn validate_transcription_model_ready<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<(), String> {
    // Check transcript configuration to determine which engine to validate
    let config =
        match crate::api::api::api_get_transcript_config(app.clone(), app.clone().state(), None)
            .await
        {
            Ok(Some(config)) => {
                info!(
                    "📝 Found transcript config - provider: {}, model: {}",
                    config.provider, config.model
                );
                config
            }
            Ok(None) => {
                info!("📝 No transcript config found, defaulting to parakeet");
                crate::api::api::TranscriptConfig {
                    provider: "parakeet".to_string(),
                    model: crate::config::DEFAULT_PARAKEET_MODEL.to_string(),
                    api_key: None,
                    has_api_key: false,
                }
            }
            Err(e) => {
                warn!(
                    "⚠️ Failed to get transcript config: {}, defaulting to parakeet",
                    e
                );
                crate::api::api::TranscriptConfig {
                    provider: "parakeet".to_string(),
                    model: crate::config::DEFAULT_PARAKEET_MODEL.to_string(),
                    api_key: None,
                    has_api_key: false,
                }
            }
        };

    // Validate based on provider
    match config.provider.as_str() {
        "localWhisper" => {
            info!("🔍 Validating Whisper model...");
            // Ensure whisper engine is initialized first
            if let Err(init_error) = crate::whisper_engine::commands::whisper_init().await {
                warn!("❌ Failed to initialize Whisper engine: {}", init_error);
                return Err(format!(
                    "Failed to initialize speech recognition: {}",
                    init_error
                ));
            }

            // Call the whisper validation command with config support
            match crate::whisper_engine::commands::whisper_validate_model_ready_with_config(app)
                .await
            {
                Ok(model_name) => {
                    info!(
                        "✅ Whisper model validation successful: {} is ready",
                        model_name
                    );
                    Ok(())
                }
                Err(e) => {
                    warn!("❌ Whisper model validation failed: {}", e);
                    Err(e)
                }
            }
        }
        "parakeet" => {
            info!("🔍 Validating Parakeet model...");
            // Ensure parakeet engine is initialized first
            if let Err(init_error) = crate::parakeet_engine::commands::parakeet_init().await {
                warn!("❌ Failed to initialize Parakeet engine: {}", init_error);
                return Err(format!(
                    "Failed to initialize Parakeet speech recognition: {}",
                    init_error
                ));
            }

            // Use the validation command that includes auto-discovery and loading
            // This matches the Whisper behavior for consistency
            match crate::parakeet_engine::commands::parakeet_validate_model_ready_with_config(app)
                .await
            {
                Ok(model_name) => {
                    info!(
                        "✅ Parakeet model validation successful: {} is ready",
                        model_name
                    );
                    Ok(())
                }
                Err(e) => {
                    warn!("❌ Parakeet model validation failed: {}", e);
                    Err(e)
                }
            }
        }
        other => {
            warn!(
                "❌ Unsupported transcription provider for local recording: {}",
                other
            );
            Err(format!(
                "Provider '{}' is not supported for local transcription. Please select 'localWhisper' or 'parakeet'.",
                other
            ))
        }
    }
}

/// Get or initialize the appropriate transcription engine based on provider configuration
pub async fn get_or_init_transcription_engine<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<TranscriptionEngine, String> {
    // Get provider configuration from API
    let config =
        match crate::api::api::api_get_transcript_config(app.clone(), app.clone().state(), None)
            .await
        {
            Ok(Some(config)) => {
                info!(
                    "📝 Transcript config - provider: {}, model: {}",
                    config.provider, config.model
                );
                config
            }
            Ok(None) => {
                info!("📝 No transcript config found, defaulting to parakeet");
                crate::api::api::TranscriptConfig {
                    provider: "parakeet".to_string(),
                    model: crate::config::DEFAULT_PARAKEET_MODEL.to_string(),
                    api_key: None,
                    has_api_key: false,
                }
            }
            Err(e) => {
                warn!(
                    "⚠️ Failed to get transcript config: {}, defaulting to parakeet",
                    e
                );
                crate::api::api::TranscriptConfig {
                    provider: "parakeet".to_string(),
                    model: crate::config::DEFAULT_PARAKEET_MODEL.to_string(),
                    api_key: None,
                    has_api_key: false,
                }
            }
        };

    // Initialize the appropriate engine based on provider
    match config.provider.as_str() {
        "parakeet" => {
            info!("🦜 Initializing Parakeet transcription engine");

            // Get Parakeet engine
            let engine = {
                let guard = crate::parakeet_engine::commands::PARAKEET_ENGINE
                    .lock()
                    .unwrap();
                guard.as_ref().cloned()
            };

            match engine {
                Some(engine) => {
                    // Check if model is loaded
                    if engine.is_model_loaded().await {
                        let model_name = engine
                            .get_current_model()
                            .await
                            .unwrap_or_else(|| "unknown".to_string());
                        info!("✅ Parakeet model '{}' already loaded", model_name);
                        Ok(TranscriptionEngine::Parakeet(engine))
                    } else {
                        Err("Parakeet engine initialized but no model loaded. This should not happen after validation.".to_string())
                    }
                }
                None => Err(
                    "Parakeet engine not initialized. This should not happen after validation."
                        .to_string(),
                ),
            }
        }
        "localWhisper" | _ => {
            info!("🎤 Initializing Whisper transcription engine");
            let whisper_engine = get_or_init_whisper(app).await?;
            Ok(TranscriptionEngine::Whisper(whisper_engine))
        }
    }
}

/// Get or initialize transcription engine using API configuration
/// Returns Whisper engine if provider is localWhisper, otherwise returns error for non-Whisper providers
pub async fn get_or_init_whisper<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<Arc<crate::whisper_engine::WhisperEngine>, String> {
    // Check if engine already exists and has a model loaded
    let existing_engine = {
        let engine_guard = crate::whisper_engine::commands::WHISPER_ENGINE
            .lock()
            .unwrap();
        engine_guard.as_ref().cloned()
    };

    if let Some(engine) = existing_engine {
        // Check if a model is already loaded
        if engine.is_model_loaded().await {
            let current_model = engine
                .get_current_model()
                .await
                .unwrap_or_else(|| "unknown".to_string());

            // NEW: Check if loaded model matches saved config
            let configured_model = match crate::api::api::api_get_transcript_config(
                app.clone(),
                app.clone().state(),
                None,
            )
            .await
            {
                Ok(Some(config)) => {
                    info!(
                        "📝 Saved transcript config - provider: {}, model: {}",
                        config.provider, config.model
                    );
                    if config.provider == "localWhisper" && !config.model.is_empty() {
                        Some(config.model)
                    } else {
                        None
                    }
                }
                Ok(None) => {
                    info!("📝 No transcript config found in database");
                    None
                }
                Err(e) => {
                    warn!("⚠️ Failed to get transcript config: {}", e);
                    None
                }
            };

            // If loaded model matches config, reuse it
            if let Some(ref expected_model) = configured_model {
                if current_model == *expected_model {
                    info!(
                        "✅ Loaded model '{}' matches saved config, reusing",
                        current_model
                    );
                    return Ok(engine);
                } else {
                    info!(
                        "🔄 Loaded model '{}' doesn't match saved config '{}', reloading correct model...",
                        current_model, expected_model
                    );
                    // Unload the incorrect model
                    engine.unload_model().await;
                    info!("📉 Unloaded incorrect model '{}'", current_model);
                    // Continue to model loading logic below
                }
            } else {
                // No specific config saved, accept currently loaded model
                info!(
                    "✅ No specific model configured, using currently loaded model: '{}'",
                    current_model
                );
                return Ok(engine);
            }
        } else {
            info!("🔄 Whisper engine exists but no model loaded, will load model from config");
        }
    }

    // Initialize new engine if needed
    info!("Initializing Whisper engine");

    // First ensure the engine is initialized
    if let Err(e) = crate::whisper_engine::commands::whisper_init().await {
        return Err(format!("Failed to initialize Whisper engine: {}", e));
    }

    // Get the engine reference
    let engine = {
        let engine_guard = crate::whisper_engine::commands::WHISPER_ENGINE
            .lock()
            .unwrap();
        engine_guard
            .as_ref()
            .cloned()
            .ok_or("Failed to get initialized engine")?
    };

    // Get model configuration from API
    let model_to_load = match crate::api::api::api_get_transcript_config(
        app.clone(),
        app.clone().state(),
        None,
    )
    .await
    {
        Ok(Some(config)) => {
            info!(
                "Got transcript config from API - provider: {}, model: {}",
                config.provider, config.model
            );
            if config.provider == "localWhisper" {
                info!("Using model from API config: {}", config.model);
                config.model
            } else {
                // Non-Whisper provider (e.g., parakeet) - this function shouldn't be called
                return Err(format!(
                        "Cannot initialize Whisper engine: Config uses '{}' provider. This is a bug in the transcription task initialization.",
                        config.provider
                    ));
            }
        }
        Ok(None) => {
            info!("No transcript config found in API, falling back to 'small'");
            "small".to_string()
        }
        Err(e) => {
            warn!(
                "Failed to get transcript config from API: {}, falling back to 'small'",
                e
            );
            "small".to_string()
        }
    };

    info!("Selected model to load: {}", model_to_load);

    // Discover available models to check if the desired model is downloaded
    let models = engine
        .discover_models()
        .await
        .map_err(|e| format!("Failed to discover models: {}", e))?;

    info!("Discovered {} models", models.len());
    for model in &models {
        info!(
            "Model: {} - Status: {:?} - Path: {}",
            model.name,
            model.status,
            model.path.display()
        );
    }

    // Check if the desired model is available
    let model_info = models.iter().find(|model| model.name == model_to_load);

    if model_info.is_none() {
        info!(
            "Model '{}' not found in discovered models. Available models: {:?}",
            model_to_load,
            models.iter().map(|m| &m.name).collect::<Vec<_>>()
        );
    }

    match model_info {
        Some(model) => {
            match model.status {
                crate::whisper_engine::ModelStatus::Available => {
                    info!("Loading model: {}", model_to_load);
                    engine
                        .load_model(&model_to_load)
                        .await
                        .map_err(|e| format!("Failed to load model '{}': {}", model_to_load, e))?;
                    info!("✅ Model '{}' loaded successfully", model_to_load);
                }
                crate::whisper_engine::ModelStatus::Missing => {
                    return Err(format!(
                        "Model '{}' is not downloaded. Please download it first from the settings.",
                        model_to_load
                    ));
                }
                crate::whisper_engine::ModelStatus::Downloading { progress } => {
                    return Err(format!("Model '{}' is currently downloading ({}%). Please wait for it to complete.", model_to_load, progress));
                }
                crate::whisper_engine::ModelStatus::Error(ref err) => {
                    return Err(format!("Model '{}' has an error: {}. Please check the model or try downloading it again.", model_to_load, err));
                }
                crate::whisper_engine::ModelStatus::Corrupted { .. } => {
                    return Err(format!("Model '{}' is corrupted. Please delete it and download again from the settings.", model_to_load));
                }
            }
        }
        None => {
            // Check if we have any available models and try to load the first one
            let available_models: Vec<_> = models
                .iter()
                .filter(|m| matches!(m.status, crate::whisper_engine::ModelStatus::Available))
                .collect();

            if let Some(fallback_model) = available_models.first() {
                warn!(
                    "Model '{}' not found, falling back to available model: '{}'",
                    model_to_load, fallback_model.name
                );
                engine.load_model(&fallback_model.name).await.map_err(|e| {
                    format!(
                        "Failed to load fallback model '{}': {}",
                        fallback_model.name, e
                    )
                })?;
                info!(
                    "✅ Fallback model '{}' loaded successfully",
                    fallback_model.name
                );
            } else {
                return Err(format!("Model '{}' is not supported and no other models are available. Please download a model from the settings.", model_to_load));
            }
        }
    }

    Ok(engine)
}

#[cfg(test)]
mod readiness_tests {
    use super::*;

    fn models(whisper: bool, parakeet: bool) -> EngineModels {
        EngineModels {
            whisper_available: whisper,
            whisper_downloading: false,
            parakeet_available: parakeet,
            parakeet_downloading: false,
        }
    }

    /// **The bug this function exists to kill.** The renderer asked
    /// `parakeet_has_available_models` on every start path, whatever the user
    /// had configured. A Local Whisper user with a working Whisper model and no
    /// Parakeet model was told to download a transcription model and could not
    /// record at all — on the core path of the product.
    #[test]
    fn a_whisper_user_is_ready_without_any_parakeet_model() {
        let r = decide_readiness("localWhisper", &models(true, false));
        assert!(r.ready, "Whisper is configured and installed: {r:?}");
        assert_eq!(r.provider, "localWhisper");
        assert!(r.reason.is_none());
    }

    /// And the mirror image, so the fix cannot be "always say yes".
    #[test]
    fn a_parakeet_user_is_ready_without_any_whisper_model() {
        let r = decide_readiness("parakeet", &models(false, true));
        assert!(r.ready, "{r:?}");
        assert_eq!(r.provider, "parakeet");
    }

    #[test]
    fn each_engine_is_judged_only_by_its_own_models() {
        // Configured engine missing, the OTHER engine installed: still not ready.
        assert!(!decide_readiness("localWhisper", &models(false, true)).ready);
        assert!(!decide_readiness("parakeet", &models(true, false)).ready);
    }

    /// "Wait for the download" and "go and install one" are different
    /// instructions; the user is owed the true one.
    #[test]
    fn downloading_is_reported_as_wait_not_as_missing() {
        let m = EngineModels {
            whisper_available: false,
            whisper_downloading: true,
            ..Default::default()
        };
        let r = decide_readiness("localWhisper", &m);
        assert!(!r.ready);
        assert!(r.downloading);
        let reason = r.reason.expect("a reason");
        assert!(reason.contains("downloading"), "{reason}");
        assert!(
            !reason.contains("No Local Whisper model is installed"),
            "{reason}"
        );
    }

    #[test]
    fn a_missing_model_names_the_engine_the_user_chose() {
        let whisper = decide_readiness("localWhisper", &models(false, false));
        assert!(whisper.reason.expect("reason").contains("Local Whisper"));
        let parakeet = decide_readiness("parakeet", &models(false, false));
        assert!(parakeet.reason.expect("reason").contains("Parakeet"));
    }

    /// A cloud provider cannot transcribe locally. The refusal says which one
    /// and what to do, rather than the old generic "model not ready".
    #[test]
    fn a_non_local_provider_is_refused_by_name() {
        for provider in ["deepgram", "openai", "groq", "elevenLabs", ""] {
            let r = decide_readiness(provider, &models(true, true));
            assert!(!r.ready, "{provider} must not be recordable locally");
            assert_eq!(r.provider, provider);
            let reason = r.reason.expect("a reason");
            if !provider.is_empty() {
                assert!(reason.contains(provider), "{reason}");
            }
        }
    }

    /// Readiness never depends on the OTHER engine's download either.
    #[test]
    fn a_download_on_the_other_engine_does_not_say_wait() {
        let m = EngineModels {
            whisper_available: true,
            whisper_downloading: false,
            parakeet_available: false,
            parakeet_downloading: true,
        };
        let r = decide_readiness("localWhisper", &m);
        assert!(r.ready);
        assert!(!r.downloading, "a Parakeet download is not a Whisper wait");
    }
}
