//! Audio capture backends.
//!
//! **This module does not own the recording path.** A recording's streams are
//! built in `audio/stream.rs` (`AudioStream` / `AudioStreamManager`) from the
//! devices resolved in `audio/devices/`. What lives here is the backend
//! *selection* ([`backend_config`]) plus the macOS Core Audio tap that
//! `stream.rs` calls into.
//!
//! So a new platform backend — for example the Linux PipeWire/PulseAudio
//! system-audio work ADR-0022 left open — belongs in `audio/devices/platform/`
//! and `audio/stream.rs`, with a variant added to
//! [`backend_config::AudioCaptureBackend`]. It does **not** belong behind a
//! standalone capture struct in this module.
//!
//! That distinction is worth stating because this module used to invite the
//! opposite. A `SystemAudioCapture` type lived here whose non-macOS arm was
//! `bail!("System audio capture not yet implemented for this platform")`,
//! reachable from no recording and from no UI — an inviting-looking hole that
//! led nowhere. It, and the `system_audio_stream.rs` sibling that was not even
//! declared as a module, were removed rather than left as a trap.

pub mod backend_config;
pub mod microphone;

#[cfg(target_os = "macos")]
pub mod core_audio;

#[cfg(target_os = "macos")]
pub use core_audio::{CoreAudioCapture, CoreAudioStream};

// Re-export backend configuration
pub use backend_config::{
    get_available_backends, get_current_backend, set_current_backend, AudioCaptureBackend,
    BackendConfig, BACKEND_CONFIG,
};
