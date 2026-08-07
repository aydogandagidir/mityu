//! Which local inference backend should run a task — the seam OS-native models
//! slot into.
//!
//! Today exactly one local backend exists: the built-in GGUF sidecar behind
//! [`LLMProvider::BuiltInAI`]. The on-device wave adds OS-native runtimes (Apple
//! Foundation Models, Windows ONNX / Copilot Runtime) that are *interchangeable*
//! with it. Naming them now, with selection expressed as one pure function, is
//! what keeps their arrival an addition rather than a refactor of every call
//! site — the same dormant-seam approach as `sync/` (ADR-0012) and `agents/`
//! (ADR-0013).
//!
//! **This module is dormant: nothing calls it in the summary path yet, and it
//! changes no behaviour.** It exists so the decision has one home.
//!
//! ## The invariant that matters
//!
//! A declared-but-unbuilt backend must never be selectable. If
//! [`select_local_backend`] could hand back [`LocalBackend::AppleFoundationModels`]
//! before it exists, a call site would route real work into nothing. Selection
//! is therefore derived from [`BackendStatus`] rather than written out by hand,
//! so an unimplemented arm cannot be returned by construction, and
//! `os_native_backends_are_never_selectable_yet` pins it.

use super::capability::{DeviceCapabilities, HostOs, LocalInferenceFitness};
use crate::summary::llm_client::LLMProvider;

/// First macOS release exposing the on-device Foundation Models API.
const APPLE_FM_MIN_MACOS: u32 = 26;

/// A backend able to run a model on this device, without the network.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalBackend {
    /// The shipped GGUF sidecar (`llama-helper`), reached through
    /// [`LLMProvider::BuiltInAI`]. The only implemented local backend.
    GgufSidecar,
    /// Apple Foundation Models — the on-device model exposed by macOS 26+ on
    /// Apple Silicon. Declared, not implemented.
    AppleFoundationModels,
    /// ONNX Runtime against a Windows NPU/GPU execution provider. Less
    /// speculative than it looks — `ort` is already a dependency, used for
    /// Parakeet — but it needs its own model-management path. Declared, not
    /// implemented.
    WindowsOnnxRuntime,
}

impl LocalBackend {
    /// Every backend in declaration order. Used by selection and the UI list.
    pub const ALL: [LocalBackend; 3] = [
        LocalBackend::GgufSidecar,
        LocalBackend::AppleFoundationModels,
        LocalBackend::WindowsOnnxRuntime,
    ];

    /// The provider a caller uses to actually reach this backend today.
    ///
    /// `None` for backends with no implementation — which is precisely why they
    /// cannot be selected.
    pub fn provider(self) -> Option<LLMProvider> {
        match self {
            Self::GgufSidecar => Some(LLMProvider::BuiltInAI),
            Self::AppleFoundationModels | Self::WindowsOnnxRuntime => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::GgufSidecar => "Built-in model",
            Self::AppleFoundationModels => "Apple Foundation Models",
            Self::WindowsOnnxRuntime => "Windows ONNX Runtime",
        }
    }
}

/// Why a backend can or cannot run here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendStatus {
    /// Implemented and usable on this host.
    Available,
    /// Named so call sites can be written against it, but no implementation
    /// ships yet. Carries where the work is tracked.
    NotImplemented { tracking: &'static str },
    /// Implemented, but this host cannot run it.
    UnsupportedHost { reason: &'static str },
}

impl BackendStatus {
    pub fn is_available(self) -> bool {
        matches!(self, Self::Available)
    }
}

/// Status of one backend on the probed host.
///
/// Host support is evaluated *before* implementation status so the reason a
/// user sees is the honest one: an Intel Mac is told Apple Foundation Models
/// needs Apple Silicon, not that it is unbuilt.
pub fn backend_status(backend: LocalBackend, caps: &DeviceCapabilities) -> BackendStatus {
    match backend {
        LocalBackend::GgufSidecar => {
            if caps.local_inference_fitness() == LocalInferenceFitness::NotRecommended {
                BackendStatus::UnsupportedHost {
                    reason: "below the memory/CPU floor for a local model",
                }
            } else {
                BackendStatus::Available
            }
        }
        LocalBackend::AppleFoundationModels => {
            if caps.os != HostOs::MacOs || caps.arch != "aarch64" {
                BackendStatus::UnsupportedHost {
                    reason: "requires Apple Silicon on macOS",
                }
            } else if caps.os_version_major.is_none_or(|v| v < APPLE_FM_MIN_MACOS) {
                // Fails closed on an unknown version: implementing this backend
                // later must not make it selectable on a release whose OS has no
                // Foundation Models API.
                BackendStatus::UnsupportedHost {
                    reason: "requires macOS 26 or newer",
                }
            } else {
                BackendStatus::NotImplemented {
                    tracking: "STRATEGY_2026-2030 Tier-0: OS-native SLM backends",
                }
            }
        }
        LocalBackend::WindowsOnnxRuntime => {
            if caps.os != HostOs::Windows {
                BackendStatus::UnsupportedHost {
                    reason: "requires Windows",
                }
            } else {
                BackendStatus::NotImplemented {
                    tracking: "STRATEGY_2026-2030 Tier-0: OS-native SLM backends",
                }
            }
        }
    }
}

/// Every backend with its status on this host, in preference order.
pub fn backend_matrix(caps: &DeviceCapabilities) -> Vec<(LocalBackend, BackendStatus)> {
    preference_order(caps)
        .into_iter()
        .map(|backend| (backend, backend_status(backend, caps)))
        .collect()
}

/// Preference order for this host, independent of whether a backend exists yet.
///
/// This is the ranking OS-native runtimes are expected to win once built — they
/// use the NPU and cost nothing per token — so the order is written now and the
/// implementations simply become selectable later.
fn preference_order(caps: &DeviceCapabilities) -> Vec<LocalBackend> {
    let mut order = Vec::with_capacity(LocalBackend::ALL.len());
    match caps.os {
        HostOs::MacOs => order.push(LocalBackend::AppleFoundationModels),
        HostOs::Windows => order.push(LocalBackend::WindowsOnnxRuntime),
        HostOs::Linux | HostOs::Other => {}
    }
    order.push(LocalBackend::GgufSidecar);
    for backend in LocalBackend::ALL {
        if !order.contains(&backend) {
            order.push(backend);
        }
    }
    order
}

/// The local backend to use on this host, or `None` when none can run.
///
/// Derived from [`backend_status`] rather than hand-written, so a backend that
/// is only *declared* can never be returned.
pub fn select_local_backend(caps: &DeviceCapabilities) -> Option<LocalBackend> {
    backend_matrix(caps)
        .into_iter()
        .find(|(_, status)| status.is_available())
        .map(|(backend, _)| backend)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::GpuType;
    use crate::inference::capability::NpuVendor;

    fn caps(os: HostOs, arch: &'static str, ram: f32, cores: usize) -> DeviceCapabilities {
        caps_on(os, arch, ram, cores, Some(APPLE_FM_MIN_MACOS))
    }

    fn caps_on(
        os: HostOs,
        arch: &'static str,
        ram: f32,
        cores: usize,
        os_version_major: Option<u32>,
    ) -> DeviceCapabilities {
        DeviceCapabilities {
            os,
            arch,
            os_version_major,
            physical_cores: Some(cores),
            logical_threads: cores * 2,
            total_ram_gb: ram,
            gpu: GpuType::None,
            npu: NpuVendor::Undetermined,
        }
    }

    /// The load-bearing guarantee of this seam: naming a backend must not make
    /// it reachable. If this ever fails, a call site can route work into a
    /// backend that does not exist.
    #[test]
    fn os_native_backends_are_never_selectable_yet() {
        for host in [
            caps(HostOs::MacOs, "aarch64", 32.0, 12),
            caps(HostOs::MacOs, "x86_64", 32.0, 12),
            caps(HostOs::Windows, "x86_64", 32.0, 12),
            caps(HostOs::Linux, "x86_64", 32.0, 12),
        ] {
            let selected = select_local_backend(&host);
            assert_ne!(selected, Some(LocalBackend::AppleFoundationModels));
            assert_ne!(selected, Some(LocalBackend::WindowsOnnxRuntime));
        }
    }

    /// Anything selectable must be reachable through a real provider.
    #[test]
    fn a_selected_backend_always_has_a_provider() {
        for host in [
            caps(HostOs::MacOs, "aarch64", 32.0, 12),
            caps(HostOs::Windows, "x86_64", 16.0, 8),
            caps(HostOs::Linux, "x86_64", 16.0, 8),
        ] {
            if let Some(backend) = select_local_backend(&host) {
                assert!(
                    backend.provider().is_some(),
                    "{backend:?} was selected without a provider"
                );
            }
        }
    }

    #[test]
    fn gguf_sidecar_is_todays_answer_on_a_capable_host() {
        assert_eq!(
            select_local_backend(&caps(HostOs::Windows, "x86_64", 16.0, 8)),
            Some(LocalBackend::GgufSidecar)
        );
    }

    #[test]
    fn nothing_is_selected_below_the_floor() {
        assert_eq!(
            select_local_backend(&caps(HostOs::Windows, "x86_64", 4.0, 2)),
            None
        );
    }

    /// An unsupported host should be told why it is unsupported, not that the
    /// backend is unbuilt — the actionable reason is the host one.
    #[test]
    fn host_support_is_reported_before_implementation_status() {
        assert!(matches!(
            backend_status(
                LocalBackend::AppleFoundationModels,
                &caps(HostOs::MacOs, "x86_64", 32.0, 12)
            ),
            BackendStatus::UnsupportedHost { .. }
        ));
        assert!(matches!(
            backend_status(
                LocalBackend::AppleFoundationModels,
                &caps(HostOs::MacOs, "aarch64", 32.0, 12)
            ),
            BackendStatus::NotImplemented { .. }
        ));
    }

    /// Apple Silicon is not sufficient — the Foundation Models API arrives in
    /// macOS 26. Without this gate, implementing the backend later would make it
    /// selectable on releases that cannot run it, and an unknown version must
    /// fail closed rather than be assumed new enough.
    #[test]
    fn apple_foundation_models_requires_macos_26() {
        for version in [None, Some(15), Some(25)] {
            assert!(
                matches!(
                    backend_status(
                        LocalBackend::AppleFoundationModels,
                        &caps_on(HostOs::MacOs, "aarch64", 32.0, 12, version)
                    ),
                    BackendStatus::UnsupportedHost { .. }
                ),
                "macOS {version:?} must not be reported as supportable"
            );
        }
        assert!(matches!(
            backend_status(
                LocalBackend::AppleFoundationModels,
                &caps_on(HostOs::MacOs, "aarch64", 32.0, 12, Some(26))
            ),
            BackendStatus::NotImplemented { .. }
        ));
    }

    #[test]
    fn os_native_backend_leads_the_preference_order_on_its_host() {
        assert_eq!(
            preference_order(&caps(HostOs::MacOs, "aarch64", 32.0, 12))[0],
            LocalBackend::AppleFoundationModels
        );
        assert_eq!(
            preference_order(&caps(HostOs::Windows, "x86_64", 32.0, 12))[0],
            LocalBackend::WindowsOnnxRuntime
        );
        assert_eq!(
            preference_order(&caps(HostOs::Linux, "x86_64", 32.0, 12))[0],
            LocalBackend::GgufSidecar
        );
    }

    #[test]
    fn the_matrix_covers_every_backend_exactly_once() {
        let matrix = backend_matrix(&caps(HostOs::Windows, "x86_64", 16.0, 8));
        assert_eq!(matrix.len(), LocalBackend::ALL.len());
        for backend in LocalBackend::ALL {
            assert_eq!(
                matrix.iter().filter(|(b, _)| *b == backend).count(),
                1,
                "{backend:?} should appear exactly once"
            );
        }
    }
}
