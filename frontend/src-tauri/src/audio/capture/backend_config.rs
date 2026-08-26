// Backend configuration for system audio capture
use log::info;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};

/// Available audio capture backends.
///
/// Every variant is gated to the platform that can actually run it. This enum
/// used to be macOS-shaped: `ScreenCaptureKit` existed on every target and was
/// the non-macOS default, so Windows and Linux reported an Apple framework as
/// their capture backend — in the logs (`stream.rs` still used CPAL regardless)
/// and, more visibly, in the settings UI, which offered a single option named
/// "ScreenCaptureKit". ADR-0022 recorded the log half of that as a known
/// cosmetic defect.
///
/// The shape matters beyond cosmetics: adding a real non-macOS backend — the
/// Linux PipeWire/PulseAudio system-audio work ADR-0022 left open as its own
/// epic — should be *adding a variant here*, not widening a macOS one. Keep the
/// per-platform gating when you do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AudioCaptureBackend {
    /// ScreenCaptureKit backend (macOS only)
    /// Uses CPAL with ScreenCaptureKit host for system audio
    #[cfg(target_os = "macos")]
    ScreenCaptureKit,

    /// Core Audio backend (macOS only)
    /// Uses direct Core Audio API with aggregate device + tap
    #[cfg(target_os = "macos")]
    CoreAudio,

    /// CPAL backend (Windows and Linux).
    ///
    /// Windows: microphone plus system audio, the latter through WASAPI
    /// loopback (an input stream opened on a render endpoint).
    /// Linux: microphone only — the ALSA host does not surface
    /// PipeWire/PulseAudio monitor sources, so there is no system-audio
    /// stream to open (ADR-0022).
    #[cfg(not(target_os = "macos"))]
    Cpal,
}

impl AudioCaptureBackend {
    /// Get human-readable name
    pub fn name(&self) -> &'static str {
        match self {
            #[cfg(target_os = "macos")]
            AudioCaptureBackend::ScreenCaptureKit => "ScreenCaptureKit",
            #[cfg(target_os = "macos")]
            AudioCaptureBackend::CoreAudio => "Core Audio",
            #[cfg(not(target_os = "macos"))]
            AudioCaptureBackend::Cpal => "CPAL",
        }
    }

    /// Get description
    pub fn description(&self) -> &'static str {
        match self {
            #[cfg(target_os = "macos")]
            AudioCaptureBackend::ScreenCaptureKit => {
                "Apple's ScreenCaptureKit framework - Higher level API with good compatibility"
            }
            #[cfg(target_os = "macos")]
            AudioCaptureBackend::CoreAudio => {
                "Direct Core Audio API - Lower latency, more control over audio pipeline"
            }
            #[cfg(target_os = "windows")]
            AudioCaptureBackend::Cpal => {
                "Cross-platform audio I/O - microphone plus system audio via WASAPI loopback"
            }
            #[cfg(not(any(target_os = "macos", target_os = "windows")))]
            AudioCaptureBackend::Cpal => {
                "Cross-platform audio I/O - microphone only on this platform; system-audio capture \
                 needs a PipeWire/PulseAudio backend that does not exist yet (ADR-0022)"
            }
        }
    }

    /// Whether the settings UI should offer this backend as a choice.
    ///
    /// On macOS the ScreenCaptureKit option has been shown but disabled since
    /// before this change; that decision used to live in the frontend as a
    /// comparison against the literal id string (`AudioBackendSelector.tsx`).
    /// It belongs here, next to the variants it talks about.
    pub fn selectable(&self) -> bool {
        match self {
            #[cfg(target_os = "macos")]
            AudioCaptureBackend::ScreenCaptureKit => false,
            #[cfg(target_os = "macos")]
            AudioCaptureBackend::CoreAudio => true,
            #[cfg(not(target_os = "macos"))]
            AudioCaptureBackend::Cpal => true,
        }
    }

    /// Get backend from string
    pub fn from_string(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            #[cfg(target_os = "macos")]
            "screencapturekit" => Some(AudioCaptureBackend::ScreenCaptureKit),
            #[cfg(target_os = "macos")]
            "coreaudio" | "core_audio" => Some(AudioCaptureBackend::CoreAudio),
            #[cfg(not(target_os = "macos"))]
            "cpal" => Some(AudioCaptureBackend::Cpal),
            // Backwards compatibility. Builds before this change persisted
            // "screencapturekit" into recording_preferences.json on *every*
            // platform, and off macOS that string only ever meant "the CPAL
            // path" — `stream.rs` never had a ScreenCaptureKit implementation
            // to reach. Accept it so upgrading does not reject a stored value
            // and silently reset the user's preference.
            #[cfg(not(target_os = "macos"))]
            "screencapturekit" => Some(AudioCaptureBackend::Cpal),
            _ => None,
        }
    }

    /// Get all available backends for current platform
    pub fn available_backends() -> Vec<Self> {
        #[cfg(target_os = "macos")]
        {
            vec![
                AudioCaptureBackend::ScreenCaptureKit,
                AudioCaptureBackend::CoreAudio,
            ]
        }

        #[cfg(not(target_os = "macos"))]
        {
            vec![AudioCaptureBackend::Cpal]
        }
    }

    /// Get default backend for current platform
    pub fn default() -> Self {
        #[cfg(target_os = "macos")]
        return AudioCaptureBackend::CoreAudio;

        #[cfg(not(target_os = "macos"))]
        return AudioCaptureBackend::Cpal;
    }
}

impl Default for AudioCaptureBackend {
    fn default() -> Self {
        Self::default()
    }
}

/// Formats as the lowercase string ID (round-trips with `from_string`).
/// `.to_string()` call sites rely on this output via the blanket `ToString` impl.
impl std::fmt::Display for AudioCaptureBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            #[cfg(target_os = "macos")]
            AudioCaptureBackend::ScreenCaptureKit => f.write_str("screencapturekit"),
            #[cfg(target_os = "macos")]
            AudioCaptureBackend::CoreAudio => f.write_str("coreaudio"),
            #[cfg(not(target_os = "macos"))]
            AudioCaptureBackend::Cpal => f.write_str("cpal"),
        }
    }
}

/// Global backend configuration
pub struct BackendConfig {
    current_backend: RwLock<AudioCaptureBackend>,
}

impl BackendConfig {
    fn new() -> Self {
        Self {
            current_backend: RwLock::new(AudioCaptureBackend::default()),
        }
    }

    /// Get current backend
    pub fn get(&self) -> AudioCaptureBackend {
        *self.current_backend.read().unwrap()
    }

    /// Set current backend
    pub fn set(&self, backend: AudioCaptureBackend) {
        info!("Switching audio capture backend to: {:?}", backend);
        *self.current_backend.write().unwrap() = backend;
    }

    /// Get available backends
    pub fn available(&self) -> Vec<AudioCaptureBackend> {
        AudioCaptureBackend::available_backends()
    }

    /// Reset to default
    pub fn reset(&self) {
        self.set(AudioCaptureBackend::default());
    }
}

/// Global backend configuration instance
pub static BACKEND_CONFIG: Lazy<Arc<BackendConfig>> = Lazy::new(|| Arc::new(BackendConfig::new()));

/// Get current backend
pub fn get_current_backend() -> AudioCaptureBackend {
    BACKEND_CONFIG.get()
}

/// Set current backend
pub fn set_current_backend(backend: AudioCaptureBackend) {
    BACKEND_CONFIG.set(backend);
}

/// Get available backends
pub fn get_available_backends() -> Vec<AudioCaptureBackend> {
    BACKEND_CONFIG.available()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every advertised backend must survive `to_string()` → `from_string()`.
    /// The persisted `system_audio_backend` preference is exactly this round
    /// trip, so a variant that fails it silently resets the user's choice.
    #[test]
    fn every_available_backend_round_trips_through_its_id() {
        for backend in AudioCaptureBackend::available_backends() {
            let id = backend.to_string();
            assert_eq!(
                AudioCaptureBackend::from_string(&id),
                Some(backend),
                "backend {:?} does not round-trip through id {:?}",
                backend,
                id
            );
        }
    }

    #[test]
    fn ids_are_lowercase_and_unique() {
        let ids: Vec<String> = AudioCaptureBackend::available_backends()
            .iter()
            .map(|b| b.to_string())
            .collect();
        for id in &ids {
            assert_eq!(
                *id,
                id.to_lowercase(),
                "backend id {:?} is not lowercase",
                id
            );
        }
        let mut deduped = ids.clone();
        deduped.sort();
        deduped.dedup();
        assert_eq!(deduped.len(), ids.len(), "duplicate backend ids: {:?}", ids);
    }

    #[test]
    fn unknown_backend_ids_are_rejected() {
        assert_eq!(AudioCaptureBackend::from_string(""), None);
        assert_eq!(AudioCaptureBackend::from_string("pipewire"), None);
        assert_eq!(AudioCaptureBackend::from_string("wasapi"), None);
    }

    #[test]
    fn the_default_backend_is_offered_by_the_platform() {
        let default = AudioCaptureBackend::default();
        assert!(
            AudioCaptureBackend::available_backends().contains(&default),
            "default backend {:?} is not in available_backends()",
            default
        );
    }

    #[test]
    fn at_least_one_backend_is_user_selectable() {
        assert!(
            AudioCaptureBackend::available_backends()
                .iter()
                .any(|b| b.selectable()),
            "the settings UI would render every backend disabled"
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_offers_screencapturekit_and_core_audio() {
        let backends = AudioCaptureBackend::available_backends();
        assert!(backends.contains(&AudioCaptureBackend::ScreenCaptureKit));
        assert!(backends.contains(&AudioCaptureBackend::CoreAudio));
        assert_eq!(
            AudioCaptureBackend::default(),
            AudioCaptureBackend::CoreAudio
        );
        assert_eq!(
            AudioCaptureBackend::from_string("core_audio"),
            Some(AudioCaptureBackend::CoreAudio)
        );
        // Shown, but not offered as a choice.
        assert!(!AudioCaptureBackend::ScreenCaptureKit.selectable());
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn non_macos_offers_cpal_only_and_still_reads_the_legacy_id() {
        assert_eq!(
            AudioCaptureBackend::available_backends(),
            vec![AudioCaptureBackend::Cpal]
        );
        assert_eq!(AudioCaptureBackend::default(), AudioCaptureBackend::Cpal);
        // No Apple framework is advertised off macOS any more...
        assert_eq!(AudioCaptureBackend::Cpal.to_string(), "cpal");
        // ...but a preferences file written by an older build still loads.
        assert_eq!(
            AudioCaptureBackend::from_string("screencapturekit"),
            Some(AudioCaptureBackend::Cpal)
        );
    }

    #[test]
    fn backend_config_starts_at_the_default_and_resets_to_it() {
        let config = BackendConfig::new();
        assert_eq!(config.get(), AudioCaptureBackend::default());

        for backend in AudioCaptureBackend::available_backends() {
            config.set(backend);
            assert_eq!(config.get(), backend);
        }

        config.reset();
        assert_eq!(config.get(), AudioCaptureBackend::default());
    }
}
