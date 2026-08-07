//! What this device can actually run locally.
//!
//! Mityu's local-first thesis depends on on-device inference getting cheaper and
//! better (Copilot+ NPUs, Apple Neural Engine, OS-native small models). Choosing
//! a backend for that wave needs one honest picture of the host, and today the
//! codebase has none: `audio::HardwareProfile` exists but is shaped for Whisper
//! tuning, lives inside `audio/`, and is consumed only by the whisper engine —
//! the summary/LLM side is completely blind to hardware.
//!
//! This module is that shared picture. It is **pure and offline** (no network,
//! no model load) and is safe to call before anything is initialised.
//!
//! ## Honesty rules
//!
//! Two failure modes matter more than coverage here, because a capability probe
//! that lies is worse than no probe: it would let the product claim a device can
//! run something it cannot.
//!
//! 1. **"We could not tell" is a distinct answer from "there is none"** —
//!    [`NpuVendor::Undetermined`] vs [`NpuVendor::None`]. Only the latter may be
//!    used to state that a device lacks an NPU.
//! 2. **Classification is a pure function over a device name**, separated from
//!    the platform enumeration that feeds it, so it can be tested exhaustively
//!    on any machine — including for false positives, which are the real risk
//!    (see [`classify_npu_device`]).

use std::sync::OnceLock;

use crate::audio::GpuType;

/// Host operating system, at the granularity backend availability depends on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostOs {
    Windows,
    MacOs,
    Linux,
    Other,
}

impl HostOs {
    pub fn current() -> Self {
        if cfg!(target_os = "windows") {
            Self::Windows
        } else if cfg!(target_os = "macos") {
            Self::MacOs
        } else if cfg!(target_os = "linux") {
            Self::Linux
        } else {
            Self::Other
        }
    }
}

/// Neural accelerator exposed by the host, when it can be established.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NpuVendor {
    /// Established that the host exposes no neural accelerator.
    None,
    /// Apple Neural Engine — present on every Apple Silicon part.
    AppleNeuralEngine,
    /// Intel AI Boost (Meteor Lake and later).
    IntelAiBoost,
    /// AMD XDNA / Ryzen AI.
    AmdXdna,
    /// Qualcomm Hexagon (Snapdragon X).
    QualcommHexagon,
    /// A device that classifies as a neural accelerator but is not one we name.
    Other,
    /// **Not established.** The platform probe for this OS is not implemented,
    /// so nothing may be claimed either way. Never treat this as "no NPU".
    Undetermined,
}

impl NpuVendor {
    /// Whether an accelerator is known to be present. `Undetermined` is false —
    /// callers must not upgrade an unknown into a capability.
    pub fn is_known_present(self) -> bool {
        !matches!(self, Self::None | Self::Undetermined)
    }
}

/// One offline snapshot of the host's local-inference capability.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DeviceCapabilities {
    pub os: HostOs,
    pub arch: &'static str,
    /// Major OS version, when the host reports one — `26` for macOS 26.
    ///
    /// Carried because backend availability depends on it: Apple Foundation
    /// Models needs macOS 26+, and without a version here a future "just mark it
    /// Available" change would select it on macOS 15, where the API does not
    /// exist. `None` means the version could not be parsed, which
    /// [`super::backend`] treats as unsupported rather than assuming.
    pub os_version_major: Option<u32>,
    /// Physical cores, when the OS reports them.
    ///
    /// `None` when it does not. Kept separate from [`Self::logical_threads`]
    /// because `available_parallelism()` counts *hardware threads*: on an SMT
    /// part a 2-core/4-thread machine would otherwise clear a four-core floor.
    pub physical_cores: Option<usize>,
    /// Hardware threads available to this process.
    pub logical_threads: usize,
    /// Total installed RAM in GiB.
    ///
    /// Measured, not guessed. `audio::HardwareProfile::detect_memory_gb` returns
    /// a hard-coded `8` unless a `MEMORY_GB` env var is set, which makes the
    /// performance tier derived from it fictional on most machines; this reads
    /// the real value through `sysinfo`, already a dependency.
    pub total_ram_gb: f32,
    pub gpu: GpuType,
    pub npu: NpuVendor,
}

static CAPABILITIES: OnceLock<DeviceCapabilities> = OnceLock::new();

impl DeviceCapabilities {
    /// Probe the host once and cache it. Offline and side-effect free.
    pub fn probe() -> &'static DeviceCapabilities {
        CAPABILITIES.get_or_init(Self::detect)
    }

    fn detect() -> DeviceCapabilities {
        let profile = crate::audio::HardwareProfile::detect();
        let (physical_cores, total_ram_gb) = host_cores_and_ram();
        DeviceCapabilities {
            os: HostOs::current(),
            arch: std::env::consts::ARCH,
            os_version_major: os_version_major(),
            physical_cores,
            // `|n| n.get()` rather than the `std::num::NonZero::get` path: that
            // generic alias is only stable since 1.79 and this crate's MSRV is
            // 1.77, so naming it would break a 1.77 toolchain.
            logical_threads: std::thread::available_parallelism().map_or(4, |n| n.get()),
            total_ram_gb,
            gpu: profile.gpu_type,
            npu: detect_npu(),
        }
    }

    /// Cores to reason about, erring low when the OS will not say.
    ///
    /// When the physical count is unknown, hardware threads are halved rather
    /// than used directly: assuming SMT understates a non-SMT part but never
    /// overstates an SMT one, and overstating is what would wrongly clear the
    /// local-model floor.
    pub fn effective_cores(&self) -> usize {
        self.physical_cores
            .unwrap_or_else(|| (self.logical_threads / 2).max(1))
    }

    /// How comfortably this device can be expected to run a local model.
    ///
    /// Deliberately coarse: it drives product copy and backend preference, not
    /// scheduling. RAM is weighted hardest because it is what actually stops a
    /// quantised model from loading.
    pub fn local_inference_fitness(&self) -> LocalInferenceFitness {
        let cores = self.effective_cores();
        if self.total_ram_gb < 8.0 || cores < 4 {
            return LocalInferenceFitness::NotRecommended;
        }
        let accelerated = self.npu.is_known_present() || self.gpu != GpuType::None;
        if self.total_ram_gb >= 16.0 && cores >= 8 && accelerated {
            LocalInferenceFitness::Comfortable
        } else {
            LocalInferenceFitness::Constrained
        }
    }
}

/// Major OS version as an integer, when it can be parsed.
///
/// `sysinfo` reports e.g. `"26.1"` on macOS and `"11"` on Windows; only the
/// leading component is used, and anything unparseable stays `None` so callers
/// fail closed instead of assuming a version.
fn os_version_major() -> Option<u32> {
    let version = sysinfo::System::os_version()?;
    version
        .split(['.', '-', ' '])
        .next()?
        .trim()
        .parse::<u32>()
        .ok()
}

/// Coarse verdict on running a local model, for backend preference and copy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalInferenceFitness {
    /// Enough headroom for a local model as the default path.
    Comfortable,
    /// Will run, but slowly or with a smaller model.
    Constrained,
    /// Below the floor — offer a cloud provider instead of promising local.
    NotRecommended,
}

/// Physical cores (when the OS reports them) and total RAM in GiB, from one
/// `sysinfo` handle — `physical_core_count` is a method, so both readings share
/// an instance rather than building two.
fn host_cores_and_ram() -> (Option<usize>, f32) {
    let mut system = sysinfo::System::new();
    system.refresh_memory();
    // sysinfo 0.32 reports bytes (see whisper_engine::system_monitor, which
    // divides by 1024 twice for MB).
    let ram_gb = system.total_memory() as f32 / 1024.0 / 1024.0 / 1024.0;
    (system.physical_core_count(), ram_gb)
}

/// Classify a neural accelerator from an OS-reported device name.
///
/// Pure so it can be tested without the hardware. The risk this guards is
/// **false positives, not misses**: a first pass that substring-matched `"npu"`
/// flagged `"Microsoft Input Configuration Device"` on a machine with no NPU at
/// all — `I·npu·t` contains it. Matching is therefore token-based, and the
/// vendor names are matched as whole phrases.
///
/// Returns `None` when the name is not a neural accelerator.
pub fn classify_npu_device(name: &str) -> Option<NpuVendor> {
    let lower = name.to_lowercase();

    // Vendor phrases must name the accelerator, not just the vendor's AI brand:
    // a bare `intel(r) ai` prefix also matches "Intel(R) AI Camera", which is not
    // an NPU and would then count as acceleration.
    if lower.contains("ai boost") {
        return Some(NpuVendor::IntelAiBoost);
    }
    if lower.contains("xdna") || lower.contains("ryzen ai") {
        return Some(NpuVendor::AmdXdna);
    }
    if lower.contains("hexagon") || lower.contains("snapdragon npu") {
        return Some(NpuVendor::QualcommHexagon);
    }
    if lower.contains("neural engine") {
        return Some(NpuVendor::AppleNeuralEngine);
    }

    // Generic terms must match as whole words, or "Input" swallows "npu".
    let is_neural_token = lower
        .split(|c: char| !c.is_alphanumeric())
        .any(|token| matches!(token, "npu" | "vpu"));
    let neural_phrase = lower.contains("neural processor")
        || lower.contains("neural processing")
        || lower.contains("ai accelerator");
    if is_neural_token || neural_phrase {
        return Some(NpuVendor::Other);
    }
    None
}

/// Platform shim feeding [`classify_npu_device`].
///
/// macOS is decided by construction: every Apple Silicon part ships a Neural
/// Engine, so `macos + aarch64` is a fact rather than a probe. Intel Macs have
/// none.
///
/// Windows and Linux require enumerating PnP devices, which is not implemented
/// here — so they report [`NpuVendor::Undetermined`] rather than `None`. That is
/// the honest answer: this slice could not be validated against a machine that
/// actually has an NPU, and reporting `None` would let callers state that a
/// Copilot+ PC has no accelerator.
fn detect_npu() -> NpuVendor {
    #[cfg(target_os = "macos")]
    {
        if std::env::consts::ARCH == "aarch64" {
            return NpuVendor::AppleNeuralEngine;
        }
        return NpuVendor::None;
    }
    #[cfg(not(target_os = "macos"))]
    {
        NpuVendor::Undetermined
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The regression that motivated token matching: a real device name from a
    /// machine with no NPU, which a substring search for "npu" flags.
    #[test]
    fn input_devices_are_not_mistaken_for_an_npu() {
        assert_eq!(
            classify_npu_device("Microsoft Input Configuration Device"),
            None
        );
        assert_eq!(classify_npu_device("HID Keyboard Input Device"), None);
        assert_eq!(classify_npu_device("Input/Output Controller"), None);
    }

    /// A vendor's AI branding is not an accelerator. `Intel(R) AI Camera` is a
    /// camera; matching it would feed a phantom NPU into the fitness verdict.
    #[test]
    fn vendor_ai_branding_is_not_an_accelerator() {
        assert_eq!(classify_npu_device("Intel(R) AI Camera"), None);
        assert_eq!(classify_npu_device("Intel(R) AI Analytics Helper"), None);
        assert_eq!(classify_npu_device("NVIDIA AI Display Driver"), None);
    }

    #[test]
    fn recognises_vendor_accelerators() {
        assert_eq!(
            classify_npu_device("Intel(R) AI Boost"),
            Some(NpuVendor::IntelAiBoost)
        );
        assert_eq!(
            classify_npu_device("AMD XDNA Neural Processing Unit"),
            Some(NpuVendor::AmdXdna)
        );
        assert_eq!(
            classify_npu_device("Qualcomm(R) Hexagon(TM) NPU"),
            Some(NpuVendor::QualcommHexagon)
        );
        assert_eq!(
            classify_npu_device("Apple Neural Engine"),
            Some(NpuVendor::AppleNeuralEngine)
        );
    }

    #[test]
    fn recognises_generic_accelerators_by_whole_token() {
        assert_eq!(classify_npu_device("Generic NPU"), Some(NpuVendor::Other));
        assert_eq!(
            classify_npu_device("Vendor Neural Processor"),
            Some(NpuVendor::Other)
        );
        assert_eq!(
            classify_npu_device("Some AI Accelerator"),
            Some(NpuVendor::Other)
        );
    }

    #[test]
    fn undetermined_is_not_a_capability() {
        assert!(!NpuVendor::Undetermined.is_known_present());
        assert!(!NpuVendor::None.is_known_present());
        assert!(NpuVendor::AppleNeuralEngine.is_known_present());
        assert!(NpuVendor::AmdXdna.is_known_present());
    }

    fn caps(ram: f32, cores: usize, gpu: GpuType, npu: NpuVendor) -> DeviceCapabilities {
        DeviceCapabilities {
            os: HostOs::Windows,
            arch: "x86_64",
            os_version_major: Some(11),
            physical_cores: Some(cores),
            logical_threads: cores * 2,
            total_ram_gb: ram,
            gpu,
            npu,
        }
    }

    #[test]
    fn fitness_floors_on_ram_and_cores() {
        assert_eq!(
            caps(6.0, 16, GpuType::Cuda, NpuVendor::AmdXdna).local_inference_fitness(),
            LocalInferenceFitness::NotRecommended
        );
        assert_eq!(
            caps(32.0, 2, GpuType::Cuda, NpuVendor::AmdXdna).local_inference_fitness(),
            LocalInferenceFitness::NotRecommended
        );
    }

    #[test]
    fn comfortable_needs_headroom_and_acceleration() {
        assert_eq!(
            caps(32.0, 12, GpuType::Cuda, NpuVendor::Undetermined).local_inference_fitness(),
            LocalInferenceFitness::Comfortable
        );
        // Same machine without a known accelerator is only constrained.
        assert_eq!(
            caps(32.0, 12, GpuType::None, NpuVendor::Undetermined).local_inference_fitness(),
            LocalInferenceFitness::Constrained
        );
        // An undetermined NPU must not be counted as acceleration.
        assert_eq!(
            caps(16.0, 8, GpuType::None, NpuVendor::Undetermined).local_inference_fitness(),
            LocalInferenceFitness::Constrained
        );
        assert_eq!(
            caps(16.0, 8, GpuType::None, NpuVendor::AmdXdna).local_inference_fitness(),
            LocalInferenceFitness::Comfortable
        );
    }

    /// Hardware threads must never be read as cores: a 2-core/4-thread part
    /// would otherwise clear the four-core floor it is meant to fail.
    #[test]
    fn smt_threads_do_not_clear_the_core_floor() {
        let smt_dual_core = DeviceCapabilities {
            os: HostOs::Windows,
            arch: "x86_64",
            os_version_major: Some(11),
            physical_cores: None, // OS would not say
            logical_threads: 4,
            total_ram_gb: 32.0,
            gpu: GpuType::Cuda,
            npu: NpuVendor::AmdXdna,
        };
        assert_eq!(smt_dual_core.effective_cores(), 2);
        assert_eq!(
            smt_dual_core.local_inference_fitness(),
            LocalInferenceFitness::NotRecommended
        );
    }

    #[test]
    fn a_reported_physical_count_wins_over_the_smt_estimate() {
        let non_smt = DeviceCapabilities {
            os: HostOs::Windows,
            arch: "x86_64",
            os_version_major: Some(11),
            physical_cores: Some(8),
            logical_threads: 8,
            total_ram_gb: 32.0,
            gpu: GpuType::Cuda,
            npu: NpuVendor::Undetermined,
        };
        assert_eq!(non_smt.effective_cores(), 8);
        assert_eq!(
            non_smt.local_inference_fitness(),
            LocalInferenceFitness::Comfortable
        );
    }

    #[test]
    fn probe_reads_a_plausible_host() {
        let c = DeviceCapabilities::probe();
        assert!(c.logical_threads >= 1);
        assert!(c.effective_cores() >= 1);
        // Real measurement, not the hard-coded 8 the whisper profile falls back to.
        assert!(
            c.total_ram_gb > 0.5,
            "total_ram_gb should be measured, got {}",
            c.total_ram_gb
        );
    }
}
