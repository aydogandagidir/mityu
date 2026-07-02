---
description: Diagnose an audio capture/transcription problem safely (highest-risk subsystem).
argument-hint: <symptom, platform>
---
Diagnose this audio issue: **$ARGUMENTS** (delegate to audio-pipeline-engineer; read-first, minimal change).

1. Confirm platform + permissions: macOS needs microphone AND screen-recording (macOS 13+) for system audio; Windows needs WASAPI loopback / virtual device. BlackHole (macOS) present?
2. Verify the 48kHz assumption and where resampling happens; check mic vs "system" device naming.
3. Enable verbose audio logging; trace the buffer/threading path in `audio*`/`recording_manager.rs`; identify where the signal is lost or corrupted.
4. Propose the smallest fix; preserve thread-safety/lock ordering.
5. **Manual smoke test** (record→transcript) on the affected platform; state which platforms were verified and which need QA.
Do not refactor `audio/` and `audio_v2/` together; if both exist, note which is authoritative first.
