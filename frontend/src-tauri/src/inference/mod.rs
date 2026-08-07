//! Local inference capability and backend selection (Tier-0 seam).
//!
//! Two questions with one home each:
//!
//! - [`capability`] — what can this device actually run? An offline probe that
//!   both STT and the summary path can consult. Until now only the whisper
//!   engine knew anything about the hardware.
//! - [`backend`] — which local backend should run a task? One pure function,
//!   with the OS-native runtimes of the on-device wave named but not yet
//!   implemented, so their arrival is an addition rather than a refactor.
//!
//! **Dormant.** Nothing in the summary or transcription path calls this yet; it
//! changes no behaviour. Both modules are pure and offline, so the local-first
//! invariant holds by construction.

pub mod backend;
pub mod capability;

pub use backend::{
    backend_matrix, backend_status, select_local_backend, BackendStatus, LocalBackend,
};
pub use capability::{DeviceCapabilities, HostOs, LocalInferenceFitness, NpuVendor};
