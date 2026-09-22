//! Refuse to hand the transcriber audio that has no speech in it.
//!
//! ## Why this exists
//!
//! Silero opens a speech segment on this machine's near-digital silence. Measured on the
//! owner's real 17-minute Turkish meeting, from his own log: **87 of 106 decoded chunks were
//! logged at `energy: 0.000000`** (about -63 dBFS), they consumed 230 s of the 284 s of GPU
//! decode time, and **83 of them came back as the same 12-character string at "confidence"
//! 0.24**. Whisper hallucinates when handed silence, and it hallucinates the same thing every
//! time.
//!
//! Until now a threshold in `transcription/worker.rs` discarded those after the fact. That
//! threshold is being removed, correctly: the number it tested was never a confidence.
//! `whisper_engine.rs` derives it from text length -- `(len/100).min(0.9)+0.1` -- so `>= 0.3`
//! meant `>= 20 bytes`, and what it actually deleted was short real speech: "Tamam.",
//! "Katiliyorum.", "Sorusu olan var mi?". It filtered in the wrong direction, because a
//! hallucination is fluent and long while agreement is short.
//!
//! But removing it with nothing in its place turns a silent data-loss bug into a loud
//! fabrication bug: on that same meeting the transcript would gain **89 invented lines against
//! 15 real ones**. So the filter has to move, not disappear -- from *after* the decoder, where
//! it judged text by length, to *before* it, where it judges audio by whether anyone spoke.
//! Short real speech then survives on its own merit: it is quiet-but-present, not absent.
//!
//! Whisper's own defence is unavailable. `set_no_speech_thold` is a no-op -- whisper-rs 0.13.2
//! documents it as "currently not implemented" -- and whisper-rs-sys 0.11.1 exposes no
//! per-segment `no_speech_prob`. A gate in front of the decoder is the only lever this
//! dependency set offers.
//!
//! ## Why the loudest window, and not the mean
//!
//! The obvious statistic -- mean energy over the chunk -- is wrong here, and the VAD ceiling is
//! what makes it wrong. A live segment can run to 30 s, so one sentence inside 29 s of room
//! tone is an ordinary shape. Measured in these recordings: a 99.8-second chunk has a mean
//! square of 3.8e-6, which any mean-based gate would discard, while its loudest 100 ms window
//! sits at RMS 0.017 -- plainly someone talking. The mean answers "was this chunk mostly
//! speech"; the question is "did anyone speak in it at all".
//!
//! ## The threshold, and the margin
//!
//! [`SPEECH_ENERGY_FLOOR`] is mean-square, measured across eleven of the owner's real
//! recordings:
//!
//! | measured                                 | mean-square | vs the floor |
//! |------------------------------------------|-------------|--------------|
//! | loudest window of any all-silence chunk  | 2.44e-5     | below        |
//! | **the floor**                            | **3e-5**    | --           |
//! | quietest chunk that might contain speech | 6.46e-5     | 2.2x above   |
//! | quietest *confirmed* speech              | 1.56e-3     | 52x above    |
//!
//! Deliberately NOT reused: `common::SILENCE_RMS_THRESHOLD` (0.02 RMS = 4e-4 mean-square).
//! That is six times higher than the quietest chunk that might hold speech, and it exists for
//! a different job -- trimming, not gating.
//!
//! ## If this threshold is ever wrong
//!
//! The failure this replaces was invisible: words vanished with no marker and no count. This
//! one must not be. Every rejected segment is logged with the energy that rejected it, and
//! [`SilenceGateStats`] keeps a running tally, so a log answers "is the gate eating my speech?"
//! directly rather than by inference. **If you raise this floor, re-measure against the
//! quietest speech you can find -- not against silence.**

use std::sync::atomic::{AtomicU64, Ordering};

/// The window the gate looks through: 100 ms at 16 kHz.
///
/// Long enough that a single click or a codec artefact cannot open the gate; short enough that
/// one word does not have to compete with the silence around it.
pub const GATE_WINDOW_SAMPLES: usize = 1_600;

/// Mean-square energy a 100 ms window must reach for the segment to be worth decoding.
/// 3e-5 mean-square is RMS 0.0055, about -45 dBFS. See the table in the module docs.
pub const SPEECH_ENERGY_FLOOR: f32 = 3e-5;

/// Running tally, so a misconfigured floor is visible in a log rather than inferred from
/// missing words.
#[derive(Debug, Default)]
pub struct SilenceGateStats {
    passed: AtomicU64,
    rejected: AtomicU64,
}

impl SilenceGateStats {
    pub fn record(&self, passed: bool) {
        if passed {
            self.passed.fetch_add(1, Ordering::Relaxed);
        } else {
            self.rejected.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn passed(&self) -> u64 {
        self.passed.load(Ordering::Relaxed)
    }

    pub fn rejected(&self) -> u64 {
        self.rejected.load(Ordering::Relaxed)
    }

    /// One line for the end of a recording: what the gate did over the whole session.
    pub fn summary(&self) -> String {
        let (p, r) = (self.passed(), self.rejected());
        format!(
            "silence gate: {} segments decoded, {} rejected as silent ({} total, floor {:.1e} mean-square over {} ms)",
            p,
            r,
            p + r,
            SPEECH_ENERGY_FLOOR,
            GATE_WINDOW_SAMPLES * 1000 / 16_000
        )
    }
}

/// The mean-square energy of the loudest [`GATE_WINDOW_SAMPLES`]-long window.
///
/// A rolling sum in `f64`: exact enough over the 480 000 samples a 30-second segment carries,
/// where repeated `f32` add-and-subtract would drift. Linear in the segment length, so it costs
/// nothing next to a decode.
///
/// A segment shorter than one window is measured whole rather than rejected for being short --
/// deciding that is the length filter's job, not this one's.
pub fn loudest_window_energy(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }

    let window = GATE_WINDOW_SAMPLES.min(samples.len());
    let mut sum: f64 = samples[..window]
        .iter()
        .map(|s| (*s as f64) * (*s as f64))
        .sum();
    let mut best = sum;

    for i in window..samples.len() {
        let entering = samples[i] as f64;
        let leaving = samples[i - window] as f64;
        sum += entering * entering - leaving * leaving;
        if sum > best {
            best = sum;
        }
    }

    (best.max(0.0) / window as f64) as f32
}

/// Did anyone speak in this segment?
///
/// Returns the verdict and the energy behind it, so the caller can log the number that decided
/// it. A gate that only says "no" teaches nobody anything when it is wrong.
pub fn carries_speech(samples: &[f32]) -> (bool, f32) {
    let energy = loudest_window_energy(samples);
    (energy >= SPEECH_ENERGY_FLOOR, energy)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(len: usize, amplitude: f32) -> Vec<f32> {
        (0..len)
            .map(|i| amplitude * (i as f32 * 0.1).sin())
            .collect()
    }

    #[test]
    fn digital_silence_is_rejected() {
        let (speech, energy) = carries_speech(&vec![0.0f32; 16_000]);
        assert!(!speech);
        assert_eq!(energy, 0.0);
    }

    #[test]
    fn the_loudest_all_silence_chunk_measured_on_the_owners_machine_is_rejected() {
        // 2.44e-5 mean-square was the loudest window of any chunk confirmed to hold no speech.
        // The floor sits above it on purpose; if this test starts failing, the floor has been
        // lowered into the noise and the gate has stopped gating.
        let amplitude = (2.44e-5f32 * 2.0).sqrt(); // sine: mean-square = A^2 / 2
        let (speech, energy) = carries_speech(&tone(16_000, amplitude));
        assert!(!speech, "measured silence at {energy:.2e} must not pass");
    }

    #[test]
    fn the_quietest_confirmed_speech_passes_with_room_to_spare() {
        // 1.56e-3 mean-square. The margin is the whole safety case for this gate.
        let amplitude = (1.56e-3f32 * 2.0).sqrt();
        let (speech, energy) = carries_speech(&tone(16_000, amplitude));
        assert!(speech, "quietest real speech at {energy:.2e} must pass");
        assert!(
            energy / SPEECH_ENERGY_FLOOR > 10.0,
            "margin collapsed to {}x -- re-measure before shipping",
            energy / SPEECH_ENERGY_FLOOR
        );
    }

    #[test]
    fn a_short_utterance_inside_a_long_silence_survives() {
        // THE case the mean would fail, and the whole reason for a windowed statistic: the
        // 30-second VAD ceiling makes "one sentence in half a minute of room tone" ordinary.
        let mut segment = vec![0.0f32; 16_000 * 30];
        let utterance = tone(1_600, 0.08);
        segment[16_000 * 12..16_000 * 12 + 1_600].copy_from_slice(&utterance);

        let mean_square: f32 = segment.iter().map(|s| s * s).sum::<f32>() / segment.len() as f32;
        assert!(
            mean_square < SPEECH_ENERGY_FLOOR,
            "the premise of this test is that a mean gate would drop it"
        );

        let (speech, energy) = carries_speech(&segment);
        assert!(speech, "windowed energy {energy:.2e} should have caught it");
    }

    #[test]
    fn a_segment_shorter_than_the_window_is_still_measured() {
        // The 50 ms length filter decides what is too short; this must not reject on length.
        let (speech, _) = carries_speech(&tone(800, 0.2));
        assert!(speech);
        let (silent, _) = carries_speech(&vec![0.0f32; 800]);
        assert!(!silent);
    }

    #[test]
    fn empty_input_is_silence_and_does_not_panic() {
        let (speech, energy) = carries_speech(&[]);
        assert!(!speech);
        assert_eq!(energy, 0.0);
    }

    #[test]
    fn the_rolling_sum_agrees_with_the_naive_computation() {
        // The rolling sum is the only clever thing in this file, so it is pinned against the
        // obvious implementation on a signal whose loudest window is not at the start.
        let mut samples = tone(5_000, 0.01);
        samples.extend(tone(2_000, 0.3));
        samples.extend(tone(5_000, 0.01));

        let naive = samples
            .windows(GATE_WINDOW_SAMPLES)
            .map(|w| w.iter().map(|s| (*s as f64) * (*s as f64)).sum::<f64>() / w.len() as f64)
            .fold(0.0f64, f64::max) as f32;

        let rolling = loudest_window_energy(&samples);
        assert!(
            (naive - rolling).abs() / naive < 1e-4,
            "rolling {rolling:.6e} disagrees with naive {naive:.6e}"
        );
    }

    #[test]
    fn the_tally_is_what_makes_a_wrong_floor_visible() {
        let stats = SilenceGateStats::default();
        stats.record(true);
        stats.record(false);
        stats.record(false);
        assert_eq!((stats.passed(), stats.rejected()), (1, 2));
        let s = stats.summary();
        assert!(s.contains("1 segments decoded"), "{s}");
        assert!(s.contains("2 rejected"), "{s}");
    }
}
