# GA readiness audit — commercial launch punch list

Snapshot from a 2026-07-07 commercial-launch-readiness audit: 5 parallel read-only agents plus direct verification (git log, file existence, `cargo fmt --check`, license research) against this repo at commit `b28848d` on `chore/bootstrap-phase0`. Complements `docs/RELEASE_CHECKLIST.md` (brand/keys/supply-chain gates) and the BACKLOG exit gates (C8/D5) — this file is broader: it covers legal, licensing, and commercial-process gaps those two don't.

**Re-verify before trusting**: this is a point-in-time snapshot. File:line citations and pass/fail states may have moved since. Cross-check against current code/CI before acting on an item that's already been checked off elsewhere.

The architecture underneath all of this is mature: tenant-ready schema, SQLCipher at-rest encryption, keychain secret storage, opt-in PII redaction, pre-recording consent, source-linked HITL summary approval, and MD/DOCX/PDF export are all shipped and were re-confirmed at the commit level during this audit. What's below is what's left before "feature-complete" becomes "sellable."

## Tier 0 — Critical blockers

Must be resolved before any commercial launch — product-accuracy, legal exposure, or a conflict with a stated privacy promise.

1. **Phase-0 transcription validation gate is at zero; deferred for v1.0.4 only.** The harness (`eval-harness/`) is fully built and working, but `eval/raw/{quiet,field,multi,jargon}/` all contain nothing but a `.gitkeep`. The product owner accepted this as version-scoped evidence debt in ADR-0027, so it is not a v1.0.4 publication blocker. It remains `NOT EVALUATED`, not PASS: no measured field/noise/jargon/diarization accuracy or pilot-value claim is allowed, and A5-dependent downstream work stays locked. Closure still requires ≥5 consented clips per bucket (2–10 min), human-corrected references, and a human verdict.
2. ~~HEAD fails its own CI (`cargo fmt --all --check`)~~ — **fixed 2026-07-07**, targeted `cargo fmt` on the one drifted file (`whisper_engine.rs`), re-verified clean.
3. ~~Model licensing~~ — **fully resolved, see ADR-0020.** Whisper (MIT) and Parakeet (CC-BY-4.0, credited in `README.md`) are both commercially clean. Both follow-ups landed 2026-07-07: Parakeet download repointed to `istupakov/parakeet-tdt-0.6b-v3-onnx` on Hugging Face, and an in-app credits section added to the About screen.
4. **No Terms of Service / EULA anywhere.** Only a privacy policy and the MIT code license exist. Draft in progress (`TERMS_OF_SERVICE.md`) — **needs real legal review before publication**, especially for KVKK/GDPR-covered users.
5. **Supply chain — mostly resolved, one verification still owed (2026-07-07, ADR-0020 + ADR-0021).** Parakeet now downloads from `huggingface.co/istupakov/parakeet-tdt-0.6b-v3-onnx`. FFmpeg for **Windows and Linux (x64/arm64)** now downloads from a self-hosted GitHub release (`aydogandagidir/mityu`, tag `ffmpeg-deps-8.1-lgpl`), switched from the previous **GPLv3** build (gyan.dev, `--enable-gpl`) to **LGPL-only** static builds — Mityu never uses a GPL-only codec, so this drops a source-redistribution obligation it was carrying for nothing. Still open, in priority order:
   - ~~**The new FFmpeg binaries have never been executed.**~~ **Resolved for the v1.0.4 Windows x64 matrix (2026-07-14/15):** the pinned LGPL binary was freshly acquired, its executable hash and banner were verified, GPL/nonfree markers were rejected, and a 48 kHz AAC/MP4 encode/decode smoke passed. Exact corresponding source/build material/notices are published and independently hash-checked. macOS remains outside this release and still has a separate open provenance/hardware gate.
   - **macOS (Intel + Apple Silicon) still points at `github.com/Zackriya-Solutions/ffmpeg-binaries`** — no equivalently-licensed LGPL-only static macOS build sourced yet. Either find one, or consciously accept GPL there with proper source-availability handling.
6. **Audio retention target remains deferred and current behavior is disclosed for v1.0.4.** ADR-0005's target is to delete raw audio after transcription, but v1.0.4 keeps `audio.mp4` locally until the meeting is deleted. ADR-0027 avoids a late, untested audio-pipeline change and requires this behavior to be stated in the consent UI, privacy copy, and Terms. C6 remains open roadmap debt; the release must not claim automatic post-transcription deletion.

## Tier 1 — Release engineering

Getting an actually signed, distributable, verified build out the door.

7. **v1.0.4 protected PR/exact-SHA CI remains open.** `origin/main` is the published, rebranded Mityu v1.0.3 commit (`0a503be`), not vanilla upstream. The v1.0.4 work must still be committed, pushed and accepted through the required `rust`, `frontend` and `server-isolation` checks on its exact PR-head SHA.
8. **Windows signing inputs are known missing.** `Production` has zero secrets; repository scope holds the existing updater private key/password only. Five DigiCert KeyLocker inputs remain absent and must be provisioned by the authorised certificate owner. Updater secrets must move to `Production` only after password recovery and a successful continuity-preserving environment build.
9. **macOS has never been physically tested.** Signing infrastructure (`build-macos.yml`) is real and mature, but `docs/BACKLOG.md`'s A2 task ("state which platforms were verified") is still an unfilled placeholder, and only `aarch64-apple-darwin` is targeted (no Intel).
10. **Linux is excluded from the release pipeline.** `build-linux.yml` exists (deb/appimage/rpm) but `release.yml` ships macOS+Windows only. README correctly says "build from source" for Linux today — this just needs an explicit go/no-go decision rather than silent default.
11. Dormant upstream "Meetily PRO" licensing secrets (`MEETILY_RSA_PUBLIC_KEY`, `SUPABASE_URL`/`SUPABASE_ANON_KEY`) cleaned out of CI workflows (2026-07-07 pass) — confirmed zero code references before removal.
12. `WORKFLOWS_OVERVIEW.md`'s stale "all workflows are manual-trigger only" claim corrected (2026-07-07 pass) — `ci.yml` has auto-triggered on push all along.

## Tier 2 — Product completion

Doesn't block a sale directly, but grows maintenance and support cost the longer it's carried.

13. **Raw `invoke()` calls outside `services/`** — ~152 across 37 files (down from a prior 205/43). **The headline number is misleading; it conflates two different problems** (re-analyzed 2026-07-08):
    - **~57 sit inside already-centralized `lib/` API modules** — `lib/analytics.ts` (23, `export class Analytics`), `lib/whisper.ts` (13), `lib/parakeet.ts` (13), `lib/builtin-ai.ts` (8, `export class BuiltInAIAPI`). These already honor the convention's *intent* (typed, single definition, one place to rename) and only violate its *letter* (folder is `lib/`, not `services/`). Migrating them is mostly import-path churn — **low value**.
    - **94 sit in `components/` + `contexts/` + `hooks/` + `app/` — this is the real architectural leak** (UI reaching straight to the Tauri boundary). Heaviest: `ModelSettingsModal.tsx` (16), `OnboardingContext.tsx` (9), `Sidebar/index.tsx` (7).

    **Progress (2026-07-08):** UI-layer raw invokes **94 → 64**. Two services extracted, both by *deleting* duplication rather than adding abstraction:
    - `services/systemService.ts` — the OS-shell commands (`open_external_url` ×7, `open_system_settings` ×4, `open_recordings_folder` ×2, `open_database_folder`, `open_models_folder`). `About.tsx`, `PreferenceSettings.tsx`, `PermissionWarning.tsx` dropped their raw `invoke` import entirely.
    - `services/providerModelsService.ts` — the 9 provider-model/credential commands (`get_ollama_models` had 5 call sites, `api_get_api_key` 6) plus the 5 provider model types. `interface OllamaModel` had been **declared twice** (privately in `ModelSettingsModal`, exported from `ConfigContext`); now single-sourced. Also removed the bypasses where `configService`/`lib/builtin-ai.ts` already wrapped a command but 5 files raw-invoked it anyway. **`ModelSettingsModal.tsx` is now completely invoke-free (16 → 0).** All 13 affected command strings appear exactly once, inside `services/` (grep-verified).

    Remaining, in rough value order: `OnboardingContext.tsx` (9), `Sidebar/index.tsx`, the `builtin_ai_*` / `parakeet_*` / `whisper_*` command families, and `import_and_initialize_database` (4). One known blocker: `BuiltInModelManager.tsx`'s local `ModelInfo` diverges from `lib/builtin-ai.ts`'s `BuiltInModelInfo` (missing `path`; `status` is an inline object vs a union), so unifying it needs real type reconciliation, not a cast.
14. ~~Zero frontend tests~~ — **substantially resolved.** Vitest is wired into CI; the v1.0.4 candidate passes 12 files / 114 tests covering export approval/provenance/sanitization, IndexedDB workspace isolation, recording-consent IPC, recovery and review-state logic. **Still open:** browser-level component/onboarding/settings coverage remains limited; the suite is not a substitute for the signed installer smoke or C8 human pilot.
15. ~~Dead-but-compiled legacy backend commands~~ — **removed 2026-07-07** (`test_backend_connection`, `debug_backend_connection`, `APP_SERVER_URL`, `get_server_address` in `api.rs`; 284 Rust tests still green). Note: the frontend's `SidebarProvider.tsx` `serverAddress` sentinel is a **separate, deliberately-kept** truthy gate for meeting-fetch logic — not touched by this cleanup.
16. **Clippy's ~140-warning baseline is a deliberate, documented non-blocking choice** (ADR-0017, no `-D warnings` in CI) — not hidden debt, just unpaid technical debt with no burn-down schedule yet.
17. **C8 exit gate is deferred/not performed for v1.0.4 only.** ADR-0027 makes it non-blocking for this patch's publication, not PASS. It still requires the Phase-0 verdict and a real pilot record→approve→export cycle on an immutable candidate, and it unlocks no C8-dependent phase until then.

## Tier 3 — Commercial readiness

Non-code work the word "sale" actually requires.

18. ~~**No payment or licensing mechanism exists.**~~ **Resolved technically through Polar.sh (ADR-0023):** hosted checkout plus local-first trial/license state and keychain-backed activation are implemented. Commercial pricing/refund promises, Polar production configuration and authorised legal review remain publication gates; payment data never enters Mityu.
19. **Trademark search for "Mityu" still pending** (`docs/ROADMAP.md:10`, flagged in the CLAUDE.md header as pre-launch work). Not started.
20. **Distribution channel undecided.** Only a self-hosted Tauri updater via GitHub Releases exists today. Microsoft Store / Mac App Store / Gumroad / direct-download-with-license-key are all open options, and the choice interacts directly with item 18.
21. **No CHANGELOG, pricing/plans page, or documented support process.** `PRIVACY_POLICY.md` has a contact address (`info@bluedev.dev`) but nothing else customer-facing exists yet.

## Recommended sequence

1. `cargo fmt` fix — **done** (2026-07-07).
2. For v1.0.4, enforce ADR-0027's claim limits and disclose current raw-audio retention; do not represent A5 or C8 as passed.
3. Close the independent technical release gates: immutable SHA, protected same-SHA CI, Windows Authenticode/updater signing, legal and legacy-lead decisions, signed installer smoke, then updater canary.
4. Resume the evidence gates: collect Phase-0 recordings → run the harness → record a human GO/CONDITIONAL/NO-GO; then run and sign C8 on an immutable candidate.
5. Implement C6's configurable/default retention target as a separately tested audio-pipeline change, with the required Windows and macOS smoke coverage.
6. Only after A5/C8 close, unlock their dependent roadmap phases and target-environment claims.
