//! Speaker diarization (ADR-0034, ADR-0035).
//!
//! The engine itself lives OUT of this process, in the `diarize-helper` sidecar:
//! sherpa-onnx links its own ONNX Runtime and the app already links a different
//! one through `ort`. What lives here is everything the app is responsible for —
//! today, resolving and verifying the models the sidecar is pointed at.
//!
//! Nothing in this module runs during capture. ADR-0034 makes diarization a
//! post-hoc pass over a finished recording, so it can fail without affecting a
//! recording in progress (`CLAUDE.md` §4).

pub mod commands;
pub mod models;
pub mod service;
pub mod sidecar;
