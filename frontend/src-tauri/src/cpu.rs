//! The CPU instruction-set floor this binary was compiled against, and a check
//! that the machine actually clears it.
//!
//! ## Why this module exists
//!
//! v1.2.2 executed an AVX-512 instruction (`vcvtusi2ss`, EVEX-encoded) on an AMD
//! Ryzen 7 7435HS, which has AVX2 but no AVX-512. The CPU raised `#UD`, Windows
//! killed the process, and the user saw the app vanish the moment they pressed
//! Record. There was no dialog, no Rust panic, no `[ERROR]` line — the log simply
//! stopped mid-sentence. Diagnosing it took disassembly of two minidumps.
//!
//! `cmake/mityu-cpu-baseline.cmake` fixes the cause: the build no longer derives
//! its instruction set from whichever machine compiled it. This module is the
//! second line — it exists because a CPU floor that is not checked at runtime is
//! not a floor, it is a hope. Two things follow from that:
//!
//! 1. **[`startup_line`] is logged on every launch, before anything can crash.**
//!    Had one line naming the CPU's features been in the v1.2.2 log, the whole
//!    investigation would have been one grep. It costs a few microseconds.
//! 2. **[`baseline_ok`] gates the recording path.** We now compile for AVX2, so a
//!    machine without AVX2 would die exactly the way this bug did, just in
//!    different code. A refusal the user can read beats a process that disappears.
//!
//! ## On raising the floor
//!
//! [`BASELINE`] is a promise to users, not a tuning knob. Raising it silently
//! breaks machines that used to work, invisibly, with no error — which is the
//! entire defect this module was written for. If you raise it: change
//! `mityu-cpu-baseline.cmake` and this constant in the same commit, publish the
//! requirement, and write an ADR.

/// The floor the shipped binary is compiled for: x86-64 + AVX + AVX2 + FMA.
/// Intel Haswell (2013) and AMD Excavator (2015) onward.
///
/// Kept in lockstep with `cmake/mityu-cpu-baseline.cmake`. Nothing enforces that
/// mechanically — if you find a way to, take it.
pub const BASELINE: &[&str] = &["AVX", "AVX2", "FMA"];

/// Which baseline features this CPU is missing. Empty means the machine clears
/// the floor.
///
/// Non-x86 targets (Apple silicon) return empty: the baseline is an x86 concept,
/// and those builds are compiled for their own architecture's own floor.
#[cfg(target_arch = "x86_64")]
pub fn missing_baseline_features() -> Vec<&'static str> {
    let mut missing = Vec::new();
    if !std::arch::is_x86_feature_detected!("avx") {
        missing.push("AVX");
    }
    if !std::arch::is_x86_feature_detected!("avx2") {
        missing.push("AVX2");
    }
    if !std::arch::is_x86_feature_detected!("fma") {
        missing.push("FMA");
    }
    missing
}

#[cfg(not(target_arch = "x86_64"))]
pub fn missing_baseline_features() -> Vec<&'static str> {
    Vec::new()
}

/// Does this machine clear the floor the binary was built for?
pub fn baseline_ok() -> bool {
    missing_baseline_features().is_empty()
}

/// What to show a user whose CPU is below the floor, or `None` when it is not.
///
/// Rendered verbatim by the UI, so it says what is wrong and what they can do,
/// and does not pretend a retry might help.
pub fn refusal_sentence() -> Option<String> {
    let missing = missing_baseline_features();
    if missing.is_empty() {
        return None;
    }
    Some(format!(
        "This processor does not support {}, which Mityu's transcription engines are built to use. \
         Recording is disabled because starting it would close the app without warning. \
         Mityu needs a processor with {}.",
        missing.join(" or "),
        BASELINE.join(", ")
    ))
}

/// One line for the startup log: what the CPU has, and whether it clears the floor.
///
/// AVX-512 is reported even though nothing requires it. It is not a capability we
/// use — it is the thing whose *absence* killed v1.2.2, so a future report that
/// says `avx512f=no` next to a crash is the first thing anyone should want to see.
pub fn startup_line() -> String {
    #[cfg(target_arch = "x86_64")]
    {
        let yn = |b: bool| if b { "yes" } else { "no" };
        format!(
            "CPU baseline: required=[{}] present: avx={} avx2={} fma={} | not required: avx512f={} | clears baseline: {}",
            BASELINE.join(","),
            yn(std::arch::is_x86_feature_detected!("avx")),
            yn(std::arch::is_x86_feature_detected!("avx2")),
            yn(std::arch::is_x86_feature_detected!("fma")),
            yn(std::arch::is_x86_feature_detected!("avx512f")),
            yn(baseline_ok()),
        )
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        format!(
            "CPU baseline: not applicable on {} (the x86 baseline [{}] does not apply)",
            std::env::consts::ARCH,
            BASELINE.join(",")
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_declared_baseline_is_the_one_we_probe() {
        // If someone adds a feature to BASELINE without adding its check to
        // missing_baseline_features, the floor silently stops being enforced —
        // which is the failure mode this whole module exists to prevent. This is
        // the cheapest available guard: the probe covers exactly three features,
        // so the list must name exactly those three.
        assert_eq!(BASELINE, &["AVX", "AVX2", "FMA"]);
    }

    #[test]
    fn a_machine_that_clears_the_floor_is_not_refused() {
        // Whatever machine runs this test, the two must agree with each other.
        assert_eq!(baseline_ok(), refusal_sentence().is_none());
        assert_eq!(baseline_ok(), missing_baseline_features().is_empty());
    }

    #[test]
    fn the_refusal_names_what_is_missing_and_never_suggests_retrying() {
        // We cannot synthesise a CPU, so assert the shape on the real one and the
        // invariant that holds either way.
        match refusal_sentence() {
            None => assert!(baseline_ok()),
            Some(s) => {
                assert!(s.contains("Recording is disabled"));
                assert!(!s.to_lowercase().contains("try again"));
                for missing in missing_baseline_features() {
                    assert!(s.contains(missing), "refusal must name {missing}");
                }
            }
        }
    }

    #[test]
    fn the_startup_line_reports_avx512_even_though_we_do_not_require_it() {
        // The one line that would have turned a two-dump disassembly into a grep.
        let line = startup_line();
        assert!(line.starts_with("CPU baseline:"));
        #[cfg(target_arch = "x86_64")]
        {
            assert!(line.contains("avx512f="), "line was: {line}");
            assert!(line.contains("avx2="), "line was: {line}");
        }
    }
}
