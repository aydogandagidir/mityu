# GA readiness audit — commercial launch punch list

Snapshot from a 2026-07-07 commercial-launch-readiness audit: 5 parallel read-only agents plus direct verification (git log, file existence, `cargo fmt --check`, license research) against this repo at commit `b28848d` on `chore/bootstrap-phase0`. Complements `docs/RELEASE_CHECKLIST.md` (brand/keys/supply-chain gates) and the BACKLOG exit gates (C8/D5) — this file is broader: it covers legal, licensing, and commercial-process gaps those two don't.

**Re-verify before trusting**: this is a point-in-time snapshot. File:line citations and pass/fail states may have moved since. Cross-check against current code/CI before acting on an item that's already been checked off elsewhere.

The architecture underneath all of this is mature: tenant-ready schema, SQLCipher at-rest encryption, keychain secret storage, opt-in PII redaction, pre-recording consent, source-linked HITL summary approval, and MD/DOCX/PDF export are all shipped and were re-confirmed at the commit level during this audit. What's below is what's left before "feature-complete" becomes "sellable."

## Tier 0 — Critical blockers

Must be resolved before any commercial launch — product-accuracy, legal exposure, or a conflict with a stated privacy promise.

1. **Phase-0 transcription validation gate is at zero.** The harness (`eval-harness/`) is fully built and working, but `eval/raw/{quiet,field,multi,jargon}/` all contain nothing but a `.gitkeep`. `docs/PHASE0_VALIDATION.md` calls this the project's own "make-or-break" gate. Selling a transcription-intelligence product without ever validating transcription quality on real audio is selling the core promise unproven. **Blocked on a human recording ≥5 clips per bucket (2-10min) + correcting `.ref.txt` files** — not resolvable by an engineering session alone.
2. ~~HEAD fails its own CI (`cargo fmt --all --check`)~~ — **fixed 2026-07-07**, targeted `cargo fmt` on the one drifted file (`whisper_engine.rs`), re-verified clean.
3. ~~Model licensing~~ — **fully resolved, see ADR-0020.** Whisper (MIT) and Parakeet (CC-BY-4.0, credited in `README.md`) are both commercially clean. Both follow-ups landed 2026-07-07: Parakeet download repointed to `istupakov/parakeet-tdt-0.6b-v3-onnx` on Hugging Face, and an in-app credits section added to the About screen.
4. **No Terms of Service / EULA anywhere.** Only a privacy policy and the MIT code license exist. Draft in progress (`TERMS_OF_SERVICE.md`) — **needs real legal review before publication**, especially for KVKK/GDPR-covered users.
5. **Supply chain — mostly resolved, one verification still owed (2026-07-07, ADR-0020 + ADR-0021).** Parakeet now downloads from `huggingface.co/istupakov/parakeet-tdt-0.6b-v3-onnx`. FFmpeg for **Windows and Linux (x64/arm64)** now downloads from a self-hosted GitHub release (`aydogandagidir/mityu`, tag `ffmpeg-deps-8.1-lgpl`), switched from the previous **GPLv3** build (gyan.dev, `--enable-gpl`) to **LGPL-only** static builds — Mityu never uses a GPL-only codec, so this drops a source-redistribution obligation it was carrying for nothing. Still open, in priority order:
   - **The new FFmpeg binaries have never been executed.** Structural checks passed (archive layout, valid PE32+, asset sizes match), but `ffmpeg -version` on the new download — the check that would actually confirm no `--enable-gpl` and that it runs — was blocked by this session's safety controls (twice, correctly). **Before any signed release**: delete the cached `frontend/src-tauri/binaries/ffmpeg-*`, run one real `cargo build`, confirm the banner shows no `--enable-gpl` and no `gyan.dev`. Beware: a normal build hits the stale cache and prints a *passing* verification line for the OLD GPL binary — that line proves nothing.
   - **macOS (Intel + Apple Silicon) still points at `github.com/Zackriya-Solutions/ffmpeg-binaries`** — no equivalently-licensed LGPL-only static macOS build sourced yet. Either find one, or consciously accept GPL there with proper source-availability handling.
6. **Audio retention doesn't match the documented decision.** ADR-0005 decided to delete raw audio after transcription; this was never implemented — `audio.mp4` is kept indefinitely in `~/Music/mityu-recordings/`. This is the deliberately-deferred "C6 retention half" (post-pilot, audio-pipeline risk) — correctly sequenced after Phase-0, but still a real gap between policy and behavior until it lands.

## Tier 1 — Release engineering

Getting an actually signed, distributable, verified build out the door.

7. **Branch never merged, zero PRs ever opened** (`gh pr list --state all` → 0). `origin/main` is still vanilla, unrebranded upstream Meetily; all 52 Mityu commits live only on `chore/bootstrap-phase0`. No code-review process has ever run on this code.
8. **Signing secrets unverified.** `TAURI_SIGNING_PRIVATE_KEY(+PASSWORD)`, Apple certificates, and the Windows DigiCert KeyLocker credentials are referenced in workflows but their actual presence in GitHub can't be confirmed from the repo — no real signed release has ever been attempted end-to-end.
9. **macOS has never been physically tested.** Signing infrastructure (`build-macos.yml`) is real and mature, but `docs/BACKLOG.md`'s A2 task ("state which platforms were verified") is still an unfilled placeholder, and only `aarch64-apple-darwin` is targeted (no Intel).
10. **Linux is excluded from the release pipeline.** `build-linux.yml` exists (deb/appimage/rpm) but `release.yml` ships macOS+Windows only. README correctly says "build from source" for Linux today — this just needs an explicit go/no-go decision rather than silent default.
11. Dormant upstream "Meetily PRO" licensing secrets (`MEETILY_RSA_PUBLIC_KEY`, `SUPABASE_URL`/`SUPABASE_ANON_KEY`) cleaned out of CI workflows (2026-07-07 pass) — confirmed zero code references before removal.
12. `WORKFLOWS_OVERVIEW.md`'s stale "all workflows are manual-trigger only" claim corrected (2026-07-07 pass) — `ci.yml` has auto-triggered on push all along.

## Tier 2 — Product completion

Doesn't block a sale directly, but grows maintenance and support cost the longer it's carried.

13. **Raw `invoke()` calls outside `services/`** — ~152 across 37 files (down from a prior 205/43). **The headline number is misleading; it conflates two different problems** (re-analyzed 2026-07-08):
    - **~57 sit inside already-centralized `lib/` API modules** — `lib/analytics.ts` (23, `export class Analytics`), `lib/whisper.ts` (13), `lib/parakeet.ts` (13), `lib/builtin-ai.ts` (8, `export class BuiltInAIAPI`). These already honor the convention's *intent* (typed, single definition, one place to rename) and only violate its *letter* (folder is `lib/`, not `services/`). Migrating them is mostly import-path churn — **low value**.
    - **94 sit in `components/` + `contexts/` + `hooks/` + `app/` — this is the real architectural leak** (UI reaching straight to the Tauri boundary). Heaviest: `ModelSettingsModal.tsx` (16), `OnboardingContext.tsx` (9), `Sidebar/index.tsx` (7).

    Concrete duplication worth fixing first, because it's *deleting* duplication rather than adding abstraction: (a) `ModelSettingsModal.tsx` raw-invokes four commands `configService` **already wraps** (`api_get_model_config`, `api_get`/`api_save_custom_openai_config`, `api_test_custom_openai_connection`) plus one `lib/builtin-ai.ts` already wraps (`builtin_ai_list_models`); (b) `interface OllamaModel` is **defined twice** (`ModelSettingsModal.tsx:48`, non-exported, and `ConfigContext.tsx:11`, exported) — exactly the drift a shared service prevents; (c) the OS-shell commands (`open_external_url` ×7, `open_system_settings` ×4, `open_recordings_folder` ×2) were centralized into `services/systemService.ts` on 2026-07-08.
14. ~~Zero frontend tests~~ — **partially resolved (2026-07-07).** The original finding was worse than stated: four fully-written test files existed (`exportModel`, `exportMarkdown`, `exportDocx`, `recordingConsent`) but had **never executed** — no runner was installed. Vitest is now wired in, all 53 assertions run as real `it()` cases, and `pnpm test` is a CI gate on the `frontend:` job. The suite was mutation-tested (breaking `computeConsentGate` fails exactly the 4 consent tests) rather than trusted on a green run. **Still open**: coverage is limited to pure logic in `src/lib/` — there is still no component/interaction testing (no jsdom, no React Testing Library), so UI/onboarding/settings regressions remain uncaught.
15. ~~Dead-but-compiled legacy backend commands~~ — **removed 2026-07-07** (`test_backend_connection`, `debug_backend_connection`, `APP_SERVER_URL`, `get_server_address` in `api.rs`; 284 Rust tests still green). Note: the frontend's `SidebarProvider.tsx` `serverAddress` sentinel is a **separate, deliberately-kept** truthy gate for meeting-fetch logic — not touched by this cleanup.
16. **Clippy's ~140-warning baseline is a deliberate, documented non-blocking choice** (ADR-0017, no `-D warnings` in CI) — not hidden debt, just unpaid technical debt with no burn-down schedule yet.
17. **C8 exit gate** can't close without a real pilot record→approve→export cycle plus the Phase-0 GO/NO-GO verdict (item 1). Not an independent engineering task — a consequence of item 1.

## Tier 3 — Commercial readiness

Non-code work the word "sale" actually requires.

18. **No payment or licensing mechanism exists at all.** There is currently no technical way to charge a customer. The only adjacent infrastructure — upstream Meetily PRO's Supabase+RSA license-activation system — is confirmed dormant and unused. A pricing model (one-time license / subscription / BYOK-free-app-monetized-elsewhere) and a license-key or entitlement system both need deciding.
19. **Trademark search for "Mityu" still pending** (`docs/ROADMAP.md:10`, flagged in the CLAUDE.md header as pre-launch work). Not started.
20. **Distribution channel undecided.** Only a self-hosted Tauri updater via GitHub Releases exists today. Microsoft Store / Mac App Store / Gumroad / direct-download-with-license-key are all open options, and the choice interacts directly with item 18.
21. **No CHANGELOG, pricing/plans page, or documented support process.** `PRIVACY_POLICY.md` has a contact address (`info@bluedev.dev`) but nothing else customer-facing exists yet.

## Recommended sequence

1. `cargo fmt` fix — **done** (2026-07-07).
2. Collect Phase-0 recordings (pilot user or self-recorded) → run the eval harness → record a GO/NO-GO verdict in `docs/DECISIONS.md`. This is the one gate that unlocks everything downstream in Tier 0/2.
3. In parallel (none of these depend on Phase-0): finish the Parakeet URL repoint + in-app credits screen, draft-then-legal-review the ToS, decide the FFmpeg mirror, start the trademark search.
4. Once GO/CONDITIONAL lands: finish the C6 retention half, close C8, open the first real PR and merge to `main`.
5. Release dry run: verify signing secrets operationally, do one real macOS hardware test, cut the first signed build.
6. Commercial layer: pricing model, license/payment mechanism, distribution channel decision → GA.
