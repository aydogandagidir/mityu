//! Regression tests for the Linux "honest microphone-only" degradation.
//!
//! ADR-0022(e) made a deliberate choice: rather than drop the `deb`/`appimage`
//! bundle targets, Linux keeps shipping but stops *pretending* to offer
//! system-audio capture. Four things had to hold for that to be honest, and all
//! four were verified once, by hand, in a throwaway container — which means
//! nothing stops a later change from quietly undoing them. The Linux
//! system-audio backend epic will rewrite exactly this code, quite possibly in
//! a pull request from outside the team, so the invariants are pinned here
//! instead of in an ADR's prose.
//!
//! **These tests must not require an audio device.** CI runners have none, and
//! neither does the container this file was written in. Every assertion below
//! is about the code's own contracts, reached either before cpal is touched or
//! through enumeration that is correct when the enumeration is empty.
//!
//! When the PipeWire/PulseAudio backend does land, `linux_reports_no_system_audio_device`
//! and `linux_rejects_the_output_scope_with_a_named_error` become the tests that
//! have to *change* — deliberately, in the same commit, with the ADR that
//! authorises it. That is
//! the point: they turn a silent behaviour change into a visible one.

use app_lib::audio::devices::configuration::{AudioDevice, DeviceType};

/// `AudioDevice::from_name` requires an `(input)`/`(output)` suffix, and that
/// contract is why the old synthetic `"<name> (System Audio)"` entries could
/// never round-trip: `configure_linux_audio` registered them, and every later
/// lookup rejected them. A PipeWire monitor source must therefore arrive under a
/// name this function accepts — see the epic's acceptance criteria in
/// `docs/BACKLOG.md`.
#[test]
fn the_old_synthetic_system_audio_name_still_does_not_round_trip() {
    assert!(
        AudioDevice::from_name("Built-in Audio Analog Stereo (System Audio)").is_err(),
        "a name with no (input)/(output) suffix must be rejected; if this now \
         parses, the device-name contract changed and the persisted \
         preferred_system_device values change meaning with it"
    );

    // The contract it does honour, in both directions.
    for (name, expected) in [
        ("Some Mic (input)", DeviceType::Input),
        ("Some Speaker (output)", DeviceType::Output),
    ] {
        let parsed = AudioDevice::from_name(name)
            .unwrap_or_else(|e| panic!("{name:?} should parse, got {e}"));
        assert_eq!(parsed.device_type, expected, "wrong scope for {name:?}");
        assert_eq!(
            parsed.to_string(),
            name,
            "{name:?} must survive from_name -> Display unchanged"
        );
    }
}

/// Empty names were the other half of the old breakage: they produced a device
/// that no lookup could resolve.
#[test]
fn blank_device_names_are_rejected() {
    assert!(AudioDevice::from_name("").is_err());
    assert!(AudioDevice::from_name("   ").is_err());
}

/// On Linux `default_output_device()` fails *early and by name*, before cpal is
/// consulted — `RecordingManager` reads that `Err` as "no system device" and
/// records the microphone only. If this ever returns `Ok`, Linux is back to
/// opening an ALSA output endpoint that yields silence.
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
#[test]
fn linux_reports_no_system_audio_device() {
    let err = app_lib::audio::devices::default_output_device()
        .expect_err("system audio must not be offered on this platform (ADR-0022)");
    let msg = err.to_string();
    assert!(
        msg.contains("not supported on this platform"),
        "the error must say why, so the log is actionable; got: {msg}"
    );
    assert!(
        msg.contains("ADR-0022"),
        "the error must cite the decision it implements; got: {msg}"
    );
}

/// `configure_linux_audio` used to scan the ALSA host for names containing
/// "monitor" and register them as `"<name> (System Audio)"`. ALSA does not
/// surface PipeWire/PulseAudio monitor sources at all, so those entries only
/// ever cluttered the picker with options that failed later.
///
/// Correct with zero devices present, which is the CI case.
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
#[test]
fn linux_enumerates_no_phantom_system_audio_entries() {
    let host = cpal::default_host();
    let devices = match app_lib::audio::devices::platform::linux::configure_linux_audio(&host) {
        Ok(devices) => devices,
        // No sound system at all (containers, CI). The invariant cannot be
        // violated by an enumeration that did not happen.
        Err(_) => return,
    };

    for device in &devices {
        assert!(
            !device.name.contains("System Audio"),
            "phantom system-audio entry is back: {:?}",
            device.name
        );
        assert_eq!(
            device.device_type,
            DeviceType::Input,
            "Linux enumeration is microphone-only; {:?} is not an input",
            device.name
        );
    }
}

/// Asking for the Output scope on Linux must fail with a *named* error rather
/// than falling through to a generic "Device not found" from deep inside cpal.
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
#[tokio::test]
async fn linux_rejects_the_output_scope_with_a_named_error() {
    let device = AudioDevice::new("Whatever".to_string(), DeviceType::Output);
    // `cpal::Device` is not `Debug`, so `expect_err` is unavailable here.
    let msg = match app_lib::audio::devices::get_device_and_config(&device).await {
        Ok(_) => panic!("the Output scope has no implementation on this platform"),
        Err(e) => e.to_string(),
    };
    assert!(
        !msg.contains("Device not found"),
        "the Output scope must fail with its own explanation, not a generic \
         lookup miss; got: {msg}"
    );
}

/// The capture backend advertised off macOS must be the one `stream.rs`
/// actually uses. Before ADR-0039 this was the string "screencapturekit" on
/// Windows and Linux, which named an Apple framework that no non-macOS code
/// path could reach.
#[test]
fn the_advertised_backend_is_one_this_platform_can_run() {
    use app_lib::audio::capture::AudioCaptureBackend;

    let backends = AudioCaptureBackend::available_backends();
    assert!(!backends.is_empty(), "every platform must offer a backend");

    #[cfg(not(target_os = "macos"))]
    for backend in &backends {
        assert_ne!(
            backend.to_string(),
            "screencapturekit",
            "ScreenCaptureKit is macOS-only and must not be advertised here"
        );
    }

    assert!(
        backends.contains(&AudioCaptureBackend::default()),
        "the default backend must be one of the offered ones"
    );
}
