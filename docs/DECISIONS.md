# Architecture Decision Records (ADR)

Append a short ADR whenever you make a decision that shapes structure, dependencies, security, or the tenancy/local-first model. Format: Context → Decision → Consequences → Status.

---
## ADR-0001 — Build on the Tauri/Rust core, treat the Python backend as legacy
**Context:** Meetily's supported app is the Tauri (Rust) + Next.js client; the Python/FastAPI `backend/` is archived with unauthenticated CORS.
**Decision:** Build the product on the Tauri core. Do not depend on the legacy backend at runtime; use it as reference (schema/prompts) only. The future server is a NEW clean service.
**Consequences:** One authoritative capture/transcribe/summarize path; a clean, secure server later; some reference code stays unused.
**Status:** Accepted.

---
## ADR-0002 — Local-first, server-optional, tenant-aware-by-design
**Context:** We want enterprise + eventual multi-tenant SaaS without losing the local-first (privacy) value prop.
**Decision:** Ship single-tenant/local first; introduce the four seams (AuthContext, tenant_id on all entities, repository layer, dormant sync) from Phase 1. Server is additive.
**Consequences:** Phase 2 is additive, not a rewrite; a little upfront discipline (tenant_id, repositories) in Phase 1.
**Status:** Accepted.

---
## ADR-0003 — Server language: Rust/Axum
**Context:** Options: Rust/Axum (cohesion with the core, shared models) vs clean FastAPI (velocity, reuse legacy schema/prompts as reference). Decided 2026-07-02 at BOOTSTRAP Step 3, per the default recommendation in docs/ARCHITECTURE.md.
**Decision:** **Rust/Axum** for the Phase-2 sync/collaboration server. Rationale: one language across core+server (the docs/CONTRACTS.md types — AuthContext, Repository, sync protocol — map 1:1 to shared Rust types instead of being re-modeled in Python), one toolchain (already proven on this repo), no second runtime to secure/operate, and the archived FastAPI backend stays reference-only for schema/prompts. FastAPI remains the sanctioned fallback ONLY if Rust server iteration proves measurably slow.
**Consequences:** `server/` (Phase 2, task D1) scaffolds as Axum + Postgres (RLS) with OIDC at the gateway; shared entity types live in a workspace crate reusable by client and server; hiring/onboarding assumes Rust.
**Status:** Accepted (re-confirm briefly at D1 kickoff before the first `server/` commit; server work does not start before gate C8).

---
## ADR-0004 — Audio module: `audio/` is authoritative; `audio_v2/` is dead code
**Context:** Base has both `audio/` and `audio_v2/`. Read-only investigation (audio-pipeline-engineer, 2026-07-02, Meetily v0.4.0) gathered wiring/runtime/history evidence.
**Decision:** **`frontend/src-tauri/src/audio/` is the authoritative module.** `audio_v2/` is an abandoned v0.1.1 rewrite scaffold, **not compiled into the binary**: no `mod audio_v2;` anywhere (`lib.rs:40` declares only `pub mod audio;`), zero `crate::audio_v2::` references (vs **59** refs to `crate::audio::` across 21 files), zero `#[tauri::command]` entry points (all ~30 registered audio commands come from `audio::`), and it is bit-rotted (imports `crate::audio::core::*` which no longer exists; duplicate `MixingMode`; undefined `AudioStream`). Git: `audio/` 125 commits (active through v0.4.0); `audio_v2/` exactly 1 commit (2025-10-23, v0.1.1) then never touched; 19 TODO/"Phase N" stubs in 1,393 LOC. Live-session log targets (`app_lib::audio::pipeline`, `::vad`, `::transcription::worker`) confirm `audio/` is the executing pipeline. The improvements audio_v2 promised (simplified pipeline, EBU R128) were since implemented inside `audio/` itself (`audio/mod.rs:18-26`, `audio_processing.rs`).
**Consequences:** No convergence flag is needed — nothing reachable to converge. `audio_v2/` is FROZEN (no new code, no imports). Delete it in a dedicated audio-cleanup ticket **after the Phase-0 validation gate passes**, together with the other verified-unreferenced dead files (`src/lib_old_complex.rs`, `src/audio/core-old.rs`, `src/audio/recording_saver_old.rs`), followed by `cargo build` + one recording smoke test per platform. All audio work targets `audio/` and routes to audio-pipeline-engineer.
**Status:** Accepted.

---
## ADR-0005 — Audio retention default
**Context:** Storing raw audio increases cost + privacy risk.
**Decision:** Default to **transcript-only** (delete audio after transcription); raw-audio retention is an explicit per-tenant policy.
**Implementation status (2026-07-02, BOOTSTRAP Step 3):** the v0.4.0 base does NOT yet implement this — it unconditionally saves `audio.mp4` per meeting under `~/Music/mityu-recordings/` (verified live). The decision stands; enforcement (configurable retention, default delete-after-transcription) lands with BACKLOG **C6**. Until C6 ships, surface to pilot users that raw audio is kept locally.
**Status:** Accepted (revisit if a customer requires raw audio).

---
## ADR-0006 — Product name (provisional): "Mityu"
**Context:** The base (Meetily) is MIT but its name/brand is used commercially by Zackriya Solutions; MIT does not grant trademark rights, so we must rebrand. We need a working name to proceed with development.
**Decision:** Use **Mityu** as the working codename and bundle identifier `com.bluedev.mityu`. Short, easy to pronounce in both Turkish and English, and trademark-distinct from "Meetily". Because it is short, it is more likely to collide with existing marks/domains, so treat as **provisional** until a TÜRKPATENT/EUIPO/USPTO + domain/app-store availability check clears it before public launch + trademark filing.
**Consequences:** Development unblocked; name is a low-cost, config-level change to swap later; keep the MIT copyright notice (Zackriya Solutions) regardless of our brand.
**Status:** Accepted (provisional).

---
## ADR-0007 — Authoritative repo root: `mityu-app` (fork clone); pack folder archived
**Context:** BOOTSTRAP Step 0 (2026-07-02) found the Claude Code pack in a code-less staging folder (`../mityu`: docs + `.claude/` only, not a git repo) while the sibling `mityu-app` is the real project: a clone of fork `aydogandagidir/mityu` (upstream `Zackriya-Solutions/meetily` @ v0.4.0, branch `main`, whisper.cpp submodule initialized) with the pack already copied on top, uncommitted. The copy, on Windows' case-insensitive filesystem, overwrote two upstream files in the working tree: `CLAUDE.md` (upstream engineering notes) and `docs/architecture.md` (case-collision with the pack's `ARCHITECTURE.md`). Upstream originals are intact at `HEAD`.
**Decision:** `mityu-app` is the single authoritative repo root: all BOOTSTRAP steps, agent work, and Claude Code sessions run here; the `../mityu` pack folder is a frozen archive — do not edit it further. Overwritten upstream files are preserved as reference docs: `docs/UPSTREAM_CLAUDE.md` and `docs/UPSTREAM_ARCHITECTURE.md`. The pack's architecture doc keeps the git path `docs/architecture.md` (optional case-only rename at commit time: `git mv docs/architecture.md docs/ARCHITECTURE.md`). The pack overlay should be committed on a branch (e.g. `chore/claude-pack`) before feature work.
**Consequences:** One source of truth; upstream engineering notes remain available to agents; future Claude Code sessions must be opened in `mityu-app` so `.claude/` settings, hooks, and agents bind to the correct project dir.
**Status:** Accepted.

---
## ADR-0008 — Docs corrected to match the Meetily v0.4.0 base (Step 0 contradiction sweep)
**Context:** Step 0 verification of docs against the actual code found drift: (1) app transcription uses **whisper-rs** (builds whisper.cpp from source; features cpu/cuda/vulkan/metal/coreml/openblas/hipblas) — the `backend/whisper.cpp` submodule serves only the archived Python backend; (2) an embedded local-LLM path exists: workspace crate `llama-helper` (wrapping `llama-cpp-2`) plus an in-app model manager — local summarization is not Ollama-only; (3) actual Tauri plugins are `dialog, fs, log, notification, process, single-instance, store, updater` (docs claimed an `os` plugin); (4) `pnpm run tauri:dev` routes through `scripts/tauri-auto.js` (GPU auto-detect) and Windows scripts exist (`clean_run_windows.bat`, `build.ps1`, `build-gpu.ps1`); (5) stray heredoc `EOF` artifacts ended `docs/BACKLOG.md` and `docs/PHASE0_VALIDATION.md`; (6) the MIT license file is `LICENSE.md` (not `LICENSE`) — keep it intact under that name.
**Decision:** Corrected CLAUDE.md (§2 diagram/table, §3 stack, §8 commands) and docs/SETUP.md accordingly; removed the stray lines. `docs/UPSTREAM_CLAUDE.md` is the detailed upstream build/run reference.
**Consequences:** Agents orient on accurate ground truth; no unnecessary submodule build in the app path; Windows dev flow documented.
**Status:** Accepted.

---
## ADR-0009 — Rebrand to Mityu executed (BOOTSTRAP Step 2 / BACKLOG A3)
**Context:** Working name Mityu (ADR-0006) applied to the fork on 2026-07-02.
**Decision:** `productName`/`identifier`/window title → `Mityu` / `com.bluedev.mityu` / `Mityu`; `frontend/package.json` name → `mityu`; `src-tauri` crate name → `mityu`; user-facing strings & notification titles → Mityu; NEW recordings go to `~/Music/mityu-recordings` (old absolute paths in DB stay valid); data dirs migrated by copy (`com.meetily.ai` → `com.bluedev.mityu`, 3.2 GB incl. models+DB; `Roaming/Meetily` → `Roaming/Mityu`) — old dirs kept as rollback, delete after stabilization; **updater endpoint switched** from Zackriya releases to `github.com/aydogandagidir/mityu` releases (prevents self-update into upstream brand) — upstream updater `pubkey` retained temporarily and **must be regenerated before first release**; placeholder icon (blue "M") generated for tauri icons + public logos — replace with final brand assets before public launch.
**Deliberately kept as "meetily":** legacy-import UI/paths (HomebrewDatabaseDetector, LegacyDatabaseImport, homebrew db path) — they refer to the legacy product's data; IndexedDB `MeetilyRecoveryDB` (rename needs a data migration); model CDN `meetily.towardsgeneralintelligence.com` (upstream model hosting — supply-chain review before GA); macOS CoreAudio tap label `meetily-audio-tap` (audio module — separate audio-pipeline ticket); `lib_old_complex.rs` (legacy, do-not-extend).
**Consequences:** App runs branded as Mityu; `LICENSE.md` (MIT © Zackriya Solutions) untouched. Pre-existing QA tooling debt surfaced and tracked separately: `next lint` unconfigured / `eslint` missing; `tests/lib/blocknote-markdown.test.ts` (bun:test) breaks `tsc --noEmit`.
**Status:** Accepted.
