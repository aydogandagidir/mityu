//! What the operating system will *actually* do when the user shares their
//! screen — and the sentence the panel must show about it.
//!
//! This module exists because the honest answer differs per platform and the
//! product must not round it up. ADR-0038's first invariant is "private, not
//! covert": the panel is excluded from screen capture so a user's private notes
//! do not leak into a shared screen, and that is a *privacy* feature, not an
//! invisibility promise. Two facts make the distinction load-bearing:
//!
//! - **Windows honours it.** `SetWindowDisplayAffinity(WDA_EXCLUDEFROMCAPTURE)`
//!   — which is what Tauri's `set_content_protected` reaches on Windows — is
//!   enforced by the DWM from Windows 10 2004 (build 19041). Below that build
//!   the older `WDA_MONITOR` behaviour paints a black rectangle instead of
//!   excluding the window, which is a different and worse outcome.
//! - **macOS no longer honours it.** `NSWindow.sharingType = .none` blocks only
//!   the legacy CoreGraphics capture path. Since macOS 15 it is ignored by
//!   ScreenCaptureKit, which is what Zoom, Meet and Teams use, and Apple's DTS
//!   answer is explicit: "At this time there are no public APIs for preventing
//!   screen capture" (developer forums thread 792152; tauri-apps/tauri#14200 is
//!   open and marked upstream). Even before macOS 15 an app that chose
//!   ScreenCaptureKit could capture the panel, so this module **never** returns
//!   [`CaptureProtection::Enforced`] for macOS at any version — a pinned test
//!   enforces that, because "it worked on my Sonoma machine" is exactly the
//!   observation that would otherwise turn into a false promise.
//!
//! Linux has no equivalent API at all.
//!
//! The verdict therefore carries its own user-facing wording. A caller that
//! wanted to phrase this itself would have to re-derive the platform rules, and
//! the first time the two drifted the UI would be the copy that lies.

use serde::{Deserialize, Serialize};

/// The host operating system, as far as this decision is concerned.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostOs {
    Windows,
    MacOs,
    Linux,
    Other,
}

impl HostOs {
    /// The OS this binary was compiled for.
    pub fn current() -> Self {
        #[cfg(target_os = "windows")]
        {
            HostOs::Windows
        }
        #[cfg(target_os = "macos")]
        {
            HostOs::MacOs
        }
        #[cfg(target_os = "linux")]
        {
            HostOs::Linux
        }
        #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
        {
            HostOs::Other
        }
    }
}

/// How much the OS will actually do when asked to keep a window out of a screen
/// capture.
///
/// Three states rather than a bool, because collapsing `BestEffort` into either
/// neighbour makes the UI lie in one direction or the other: as `Enforced` it
/// promises macOS users something Apple removed, and as `Unsupported` it would
/// tell them not to bother with a setting that still blocks legacy capture and
/// screenshot tools.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CaptureProtection {
    /// The OS excludes the window from screen capture.
    Enforced,
    /// The OS accepts the request but does not guarantee the outcome.
    BestEffort,
    /// No mechanism exists on this platform.
    Unsupported,
}

/// The posture plus the exact words for it. `headline` is the chip in the panel;
/// `detail` is the sentence under the Settings switch.
///
/// **Serialize only, deliberately.** The wording is `&'static str` because it
/// lives in the binary — the backend owns it so the UI cannot invent its own
/// version — and `Deserialize` cannot produce a `&'static str` from borrowed
/// input, which is a compile error rather than a runtime surprise. This type
/// only ever travels backend → frontend, so there is nothing to deserialize.
/// Do not re-add `Deserialize` without first changing these to `String`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProtectionVerdict {
    pub level: CaptureProtection,
    pub headline: &'static str,
    pub detail: &'static str,
}

impl ProtectionVerdict {
    /// Whether to call the platform API at all. False only where no mechanism
    /// exists, so we never log a failure for something the OS was never going
    /// to do.
    pub fn should_request(&self) -> bool {
        self.level != CaptureProtection::Unsupported
    }
}

/// Windows 10 2004. `WDA_EXCLUDEFROMCAPTURE` — exclusion rather than the black
/// rectangle `WDA_MONITOR` produces — was introduced in this build.
pub const WINDOWS_EXCLUDE_FROM_CAPTURE_BUILD: u32 = 19041;

/// The first macOS whose ScreenCaptureKit ignores the window sharing type.
/// Used only to pick the wording; [`verdict_for`] returns `BestEffort` for macOS
/// either way.
pub const MACOS_SCREENCAPTUREKIT_IGNORES_SHARING_TYPE: u32 = 15;

/// Decide the posture from platform facts alone. Pure, so the whole matrix is
/// testable on any host — including the macOS and old-Windows cases this
/// container can never run.
///
/// `os_version_major` is the major version (macOS `15`, Windows `11`);
/// `windows_build` is the Windows build number (`19045`). Both are `None` when
/// the probe could not read them, and an unknown value always resolves to the
/// *weaker* claim: a missing reading must never be the reason a user is told
/// they are protected.
pub fn verdict_for(
    os: HostOs,
    os_version_major: Option<u32>,
    windows_build: Option<u32>,
) -> ProtectionVerdict {
    match os {
        HostOs::Windows => match windows_build {
            Some(build) if build >= WINDOWS_EXCLUDE_FROM_CAPTURE_BUILD => ProtectionVerdict {
                level: CaptureProtection::Enforced,
                headline: "Hidden from screen sharing",
                detail: "Windows excludes this panel from screen sharing and recording \
                         (Windows 10 build 19041 and newer). A photograph of the screen \
                         still shows it.",
            },
            Some(_) => ProtectionVerdict {
                level: CaptureProtection::BestEffort,
                headline: "Partly hidden from screen sharing",
                detail: "This build of Windows predates capture exclusion, so the panel may \
                         appear as a black rectangle to viewers instead of being hidden.",
            },
            None => ProtectionVerdict {
                level: CaptureProtection::BestEffort,
                headline: "Screen-sharing behaviour unconfirmed",
                detail: "Mityu could not read this Windows build number, so it does not claim \
                         the panel is hidden. Check with a test share before relying on it.",
            },
        },
        HostOs::MacOs => {
            let sequoia_or_newer = os_version_major
                .map(|v| v >= MACOS_SCREENCAPTUREKIT_IGNORES_SHARING_TYPE)
                .unwrap_or(true);
            if sequoia_or_newer {
                ProtectionVerdict {
                    level: CaptureProtection::BestEffort,
                    headline: "Best effort on macOS",
                    detail: "macOS 15 and newer ignore this for ScreenCaptureKit-based sharing, \
                             which is what Zoom, Meet and Teams use, so the panel can still be \
                             captured. Apple provides no API to prevent it.",
                }
            } else {
                ProtectionVerdict {
                    level: CaptureProtection::BestEffort,
                    headline: "Best effort on macOS",
                    detail: "On this macOS the request blocks only the legacy capture path; an \
                             app that uses ScreenCaptureKit can still capture the panel.",
                }
            }
        }
        HostOs::Linux | HostOs::Other => ProtectionVerdict {
            level: CaptureProtection::Unsupported,
            headline: "Not hidden from screen sharing",
            detail: "This platform has no capture-exclusion API, so treat the panel as visible \
                     to anyone you share your screen with.",
        },
    }
}

/// The verdict for the machine this is running on.
pub fn current_verdict() -> ProtectionVerdict {
    verdict_for(HostOs::current(), os_version_major(), windows_build())
}

/// Major OS version via `sysinfo` (already a dependency; the same reading
/// `inference::capability` takes). `None` when it cannot be parsed — which the
/// matrix above treats as "do not claim protection".
fn os_version_major() -> Option<u32> {
    let version = sysinfo::System::os_version()?;
    version
        .split(['.', '-', ' '])
        .next()?
        .trim()
        .parse::<u32>()
        .ok()
}

/// Windows build number. `sysinfo` reports it as the kernel version on Windows
/// (e.g. `"19045"`); on other platforms this is not meaningful and the value is
/// unused by [`verdict_for`].
fn windows_build() -> Option<u32> {
    if HostOs::current() != HostOs::Windows {
        return None;
    }
    sysinfo::System::kernel_version()?
        .trim()
        .parse::<u32>()
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_2004_and_newer_is_enforced() {
        let v = verdict_for(HostOs::Windows, Some(10), Some(19041));
        assert_eq!(v.level, CaptureProtection::Enforced);
        let v = verdict_for(HostOs::Windows, Some(11), Some(26100));
        assert_eq!(v.level, CaptureProtection::Enforced);
    }

    #[test]
    fn windows_before_2004_is_only_best_effort() {
        // The black-rectangle case: the API exists but does something else.
        let v = verdict_for(HostOs::Windows, Some(10), Some(18363));
        assert_eq!(v.level, CaptureProtection::BestEffort);
        assert!(v.detail.contains("black rectangle"));
    }

    #[test]
    fn an_unreadable_windows_build_never_claims_protection() {
        let v = verdict_for(HostOs::Windows, Some(11), None);
        assert_eq!(v.level, CaptureProtection::BestEffort);
    }

    /// The rule this module exists for. macOS must never reach `Enforced`, at
    /// any version, known or unknown — including versions that do not exist yet.
    #[test]
    fn macos_is_never_enforced_at_any_version() {
        for major in 10..=40 {
            let v = verdict_for(HostOs::MacOs, Some(major), None);
            assert_ne!(
                v.level,
                CaptureProtection::Enforced,
                "macOS {major} was reported as enforced"
            );
        }
        assert_ne!(
            verdict_for(HostOs::MacOs, None, None).level,
            CaptureProtection::Enforced
        );
    }

    #[test]
    fn an_unknown_macos_version_reads_as_the_newer_less_protected_case() {
        // Unknown must degrade toward "you may be visible", not away from it.
        let unknown = verdict_for(HostOs::MacOs, None, None);
        let sequoia = verdict_for(HostOs::MacOs, Some(15), None);
        assert_eq!(unknown, sequoia);
    }

    #[test]
    fn linux_is_unsupported_and_does_not_call_the_api() {
        let v = verdict_for(HostOs::Linux, None, None);
        assert_eq!(v.level, CaptureProtection::Unsupported);
        assert!(!v.should_request());
    }

    #[test]
    fn every_verdict_carries_words_for_the_user() {
        let cases = [
            verdict_for(HostOs::Windows, Some(11), Some(26100)),
            verdict_for(HostOs::Windows, Some(10), Some(18363)),
            verdict_for(HostOs::Windows, Some(11), None),
            verdict_for(HostOs::MacOs, Some(15), None),
            verdict_for(HostOs::MacOs, Some(14), None),
            verdict_for(HostOs::Linux, None, None),
            verdict_for(HostOs::Other, None, None),
        ];
        for v in cases {
            assert!(!v.headline.is_empty());
            assert!(!v.detail.is_empty());
            // Nothing in this module may promise invisibility (ADR-0038).
            let text = format!("{} {}", v.headline, v.detail).to_lowercase();
            assert!(
                !text.contains("undetectable"),
                "verdict promised invisibility"
            );
            assert!(!text.contains("invisible"), "verdict promised invisibility");
        }
    }

    #[test]
    fn only_unsupported_skips_the_platform_call() {
        assert!(verdict_for(HostOs::Windows, Some(11), Some(26100)).should_request());
        assert!(verdict_for(HostOs::MacOs, Some(15), None).should_request());
        assert!(!verdict_for(HostOs::Other, None, None).should_request());
    }
}
