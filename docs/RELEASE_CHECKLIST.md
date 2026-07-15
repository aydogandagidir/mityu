# Release checklist — brand & security gates

Operational steps that must pass before Mityu's first public release. Complements the `/release` command and the BACKLOG gates (C8/D5). Local-first + HITL invariants are enforced elsewhere; this file covers the brand / keys / supply-chain items surfaced during the bluedev rebrand (ADR-0006 / 0009 / 0013).

**v1.0.4 validation scope (ADR-0027):** A5 target-environment benchmarking and the C8 human pilot are `DEFERRED / NOT PASSED` and are non-blocking for this patch only. Public copy must not claim measured field/noise/jargon/diarization accuracy, an SLA, or demonstrated pilot value. Signing, same-SHA CI, legal approval, legacy-lead disposition, and updater canary controls are unchanged.

## 0. Version consistency (BLOCKING)

Development may retain the latest published application version until the owner makes an explicit release-version decision. At release time, choose the next SemVer from the latest published GitHub release, not from a stale local tag or manifest. Update these sources together: `frontend/package.json`, `frontend/src-tauri/tauri.conf.json`, `frontend/src-tauri/Cargo.toml`, and the `name = "mityu"` entry in `Cargo.lock`. Browser fallbacks and the analytics example derive their value from `frontend/package.json` through `frontend/src/lib/appVersion.ts`; do not add another literal application-version source. The release workflow fails if the four canonical values differ or if `v<version>` already exists; it never invents a four-part version. User-facing backward-compatible features normally increment MINOR, fixes only increment PATCH, and breaking changes increment MAJOR, but the owner confirms the actual release number before publication.

Dispatch a production release only from `main`; the workflow enforces this and pins the tag to the dispatched commit. If a build fails after the draft release is created, delete that failed draft and its tag with `gh release delete "v<version>" --cleanup-tag --yes` after confirming it is still an unpublished draft, then rerun. Never delete a published release/tag as a retry shortcut.

## 1. Updater signing key (CRITICAL) — key continuity and environment policy pass; secret migration + acceptance remain open

The app auto-updates by verifying a signature against the public key baked into `frontend/src-tauri/tauri.conf.json` (`plugins.updater.pubkey`). **Done already (2026-07-02):** the Mityu keypair was generated (private key at `~/.tauri/mityu_updater.key`, password-protected — keep it OUT of git; losing it or its password breaks the update chain for every existing install) and the baked pubkey was replaced with Mityu's in commit `3468a04`, so the config no longer trusts upstream's key. If it ever needs regenerating:
   ```bash
   # from the mityu-app repo root (NOT the docs-only ../mityu folder):
   cd mityu-app/frontend
   mkdir -p "$HOME/.tauri"    # PowerShell: New-Item -ItemType Directory -Force "$HOME\.tauri" | Out-Null
   pnpm tauri signer generate -w "$HOME/.tauri/mityu_updater.key"
   # prompts for a password; prints the PUBLIC key (also written to <path>.pub)
   # then paste the printed PUBLIC key into tauri.conf.json → plugins.updater.pubkey
   ```
Current state (2026-07-15): both updater secrets exist at repository scope; the local public key exactly matches `tauri.conf.json`, and its key id matches the signature in published v1.0.3 `latest.json`. The `Production` environment now requires reviewer `aydogandagidir`, accepts only `main`, and disallows administrator bypass; `main` itself is protected by strict required CI checks and PR/conversation rules. `Production` still contains zero secrets. The local private key exists, but its password is unavailable; repository secret values cannot be read back. Still OPEN before v1.0.4:
1. Recover the existing private-key password, upload `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` into `Production`, verify a successful environment build, and only then delete the repository-scoped copies. Do not generate a replacement key: that would break updater continuity for v1.0.3 installations. Non-production workflows disable updater artifacts and cannot consume the production key.
2. **Pre-publication:** inspect the draft artifacts and require the production build's `*.sig` files, exact versioned `aydogandagidir/mityu` NSIS URL/signature in `latest.json`, valid updater key chain, and valid timestamped Windows Authenticode signatures. The release workflow performs these fail-closed checks against the unpublished draft, including cryptographic verification of the remotely downloaded NSIS artifact with the public key baked into the exact release commit.
3. **Post-publication canary:** publish only after every pre-publication gate is green, then immediately use a dedicated installed v1.0.3 canary to fetch, verify and apply v1.0.4 before any broad announcement. v1.0.3 uses GitHub's `/releases/latest/download/latest.json`; GitHub excludes drafts from `latest`, so a real draft→installed-client updater E2E test is impossible. If the canary fails, stop rollout/announcement and ship a corrective patch—never delete, recreate or move a published tag.

## 2. Upstream PRO licensing (resolved — dead secrets removed)

CI used to pass `MEETILY_RSA_PUBLIC_KEY` + `SUPABASE_URL` / `SUPABASE_ANON_KEY` — upstream Meetily PRO's **license-activation** system, which Mityu does **not** ship (confirmed zero references anywhere in `frontend/src-tauri/src/`). Those `env:` lines have been removed from every workflow that had them (`build.yml`, `build-windows.yml`, `build-macos.yml`, `build-linux.yml`, `build-devtest.yml`); the matching GitHub repo secrets, if ever configured, are no longer read by anything and can be deleted too. If you later build your own licensing, add fresh `MITYU_*`-prefixed secrets and wire them in explicitly rather than reviving these names.

## 2a. Landing rate-limit secret (BLOCKING for KV-enabled deployment)

Provision `DOWNLOAD_RATE_LIMIT_HMAC_SECRET` as an encrypted Vercel secret in every Preview/Production environment that has Upstash/Vercel KV credentials. It must be cryptographically random and at least 32 UTF-8 bytes; never expose it with a `NEXT_PUBLIC_` name or reuse a signing/API/user password. Missing/partial KV configuration fails closed before any KV write. After provisioning, deploy a fresh preview and run the landing 5-test suite plus the browser request-body/rate-limit smoke before production promotion. See `landing/README.md`.

## 3. Third-party binaries & models (supply-chain)

Windows/Linux binary provenance is technically resolved. Windows v1.0.4's FFmpeg engineering/publication evidence passes; authorised legal approval and the excluded macOS path remain open:
- **FFmpeg** — Windows and Linux (x64/arm64) ~~fetched from `github.com/Zackriya-Solutions/ffmpeg-binaries`~~ **binary path fixed (2026-07-07)**: `frontend/src-tauri/build/ffmpeg.rs` now downloads from a self-hosted release, `github.com/aydogandagidir/mityu` tag `ffmpeg-deps-8.1-lgpl`. This switched the build from GPLv3 (the original gyan.dev-derived binary had `--enable-gpl`, confirmed via its own `-version` banner) to **LGPLv3-only** static builds mirrored from `BtbN/FFmpeg-Builds`; Mityu uses only audio AAC/MP4 and no GPL-only codec. The exact source, build scripts/material, dependency source cache and applicable notices are now durably published and independently hash-verified for Windows v1.0.4 (see ADR-0021). This closes engineering evidence, not counsel approval. **macOS (Intel + Apple Silicon) still fetches from the Zackriya mirror** — no equivalently-licensed LGPL-only static macOS build has been sourced yet, so macOS remains outside v1.0.4.
  > **Windows v1.0.4 technical gate passed (2026-07-14):** release hardening found that an old ignored `gyan.dev`/`--enable-gpl` cache had returned. The verifier now rejects GPL/nonfree markers, verifies the pinned archive SHA-256 and the extracted Windows executable SHA-256, and therefore automatically replaced the stale cache. The observed executable is `ffmpeg version n8.1.2-22-g94138f6973-20260706`, SHA-256 `dd757098407e2ac4920647a2f66f41a6e1006dcf373b0825023948ae1b96912a`; no forbidden markers were present. A 48 kHz one-second source encoded to a 1,228-byte AAC/MP4 and decoded successfully. Runtime executable auto-download is removed, release builds refuse PATH/cwd fallbacks, and CI repeats the policy plus AAC encode/decode smoke. Matching source/build material and the LGPL notice must remain available. macOS remains excluded from v1.0.4.
  >
  > **Windows v1.0.4 technical compliance-publication gate passed (2026-07-15):** the expiring run-28794367607 dependency source cache was recovered before its 2026-07-20 deletion date. `SHA256SUMS` was recomputed against every local payload, and the source-cache manifest matched all 230 archive entries with no missing or unexpected entry. The exact FFmpeg source (`41cc834c…e02937`), exact BtbN build scripts (`319a39ec…67588`), extracted dependency source cache (`5a9a32cd…1b27`, 2,019,293,207 bytes) and build-info/licenses/notices ZIP (`3f8b4508…35a7`) are now durably published beside `win64-lgpl.zip` in the existing `ffmpeg-deps-8.1-lgpl` release. GitHub's recorded size/digest matched all four files; unauthenticated HEAD followed one redirect to HTTP 200, and fresh unauthenticated full downloads matched every expected byte length and SHA-256. The installer resource set includes LGPLv3, GPLv3, FFmpeg/BtbN licensing notes and the linked-component inventory. This closes the engineering/upload/public-download gate only; counsel approval of the distribution package remains an independent legal release gate.
- **Parakeet model** — ~~fetched from `meetily.towardsgeneralintelligence.com`~~ **fixed (2026-07-07)**: `parakeet_engine.rs` now downloads the default v3 model from `huggingface.co/istupakov/parakeet-tdt-0.6b-v3-onnx` directly (same pattern already used for v2), verified byte-for-byte identical file names/sizes against the old CDN before switching (see ADR-0020). This also closes the model-licensing question: CC-BY-4.0, matches NVIDIA's official release, already credited in `README.md` and now also in the in-app About screen.
- **Built-in summary models** — Qwen 3.5 2B/4B are Apache-2.0; Gemma 3 1B/4B use Google's Gemma Terms and incorporated prohibited-use policy. The installer now includes `resources/MODEL-NOTICES.txt`, About links to both model families, and the Terms incorporate Gemma's use restrictions. Every built-in Qwen/Gemma artifact is revision-pinned and exact-size/SHA-256 verified before it can be loaded. The Terms remain subject to the separate counsel gate in `TERMS_OF_SERVICE.md`; engineering has closed the notice and integrity work, not the legal approval.

## 4. Build & verify (toolchain)

The Rust core builds whisper.cpp from source, needing a C toolchain:
- Install **LLVM/Clang 18.x** (NOT 22 — bindgen breaks), **CMake**, **VS 2022 Build Tools** (Parakeet's onnxruntime is VS2022-built), and **Strawberry Perl** (the SQLCipher/OpenSSL vendored build, ADR-0014, needs real Perl — git's MSYS perl fails). Windows: `winget install LLVM.LLVM Kitware.CMake StrawberryPerl.StrawberryPerl`.
- Set the env in the shell first — **this is what the earlier "build blocked" was** (tools installed but not on `PATH`), PowerShell:
  ```powershell
  $env:Path += ";C:\Program Files\CMake\bin;C:\Program Files\LLVM\bin;C:\Strawberry\perl\bin;C:\Strawberry\c\bin"
  $env:LIBCLANG_PATH   = "C:\Program Files\LLVM\bin"           # whisper-rs bindgen
  $env:OPENSSL_SRC_PERL = "C:\Strawberry\perl\bin\perl.exe"    # SQLCipher/OpenSSL build
  $env:CMAKE_GENERATOR = "Ninja"                                  # keep the cached generator deterministic
  ```
- Then:
  ```bash
  cd mityu-app/frontend        # the app repo, not the ../mityu docs folder
  pnpm install
  cargo build -p mityu         # from frontend/src-tauri (compiles the whole core incl. agents/)
  cargo test  -p mityu         # includes the dormant sync/ + agents/ invariant tests
  cargo clippy -p mityu --all-targets
  pnpm exec tauri dev --no-watch   # GUI smoke; --no-watch because the repo path contains a space
  ```
- Manual smoke (audio path is the highest-risk subsystem): record → transcript appears → summary drafts → About/tray show Mityu branding.

## 5. Running & packaging on Windows (dev-mode caveats, standalone build, MAX_PATH)

Surfaced during the 2026-07-03 data-visibility debug. `pnpm exec tauri dev` (the dev server) is **slow and flaky on this machine**: Next.js compiles routes on demand (~11 s for `/`), and in that window the meeting-list fetch sometimes never fires — so the UI looks empty even though the backend opened the (encrypted) DB and read the rows (`Database opened successfully` → `Successfully got N meetings` in the log). It is **not** a data or code bug, just dev-mode compilation friction. For **actually using** the app, build a standalone binary that embeds the production frontend.

### Fast standalone binary (recommended for local use)
```powershell
# env from §4 first, then:
cd mityu-app/frontend
pnpm exec tauri build --debug --no-bundle
# → target\debug\mityu.exe  (production frontend EMBEDDED; no dev server / no port 3118)
```
- `--debug` reuses the already-compiled **debug** whisper.cpp (`target/debug/build/whisper-rs-sys-*`) → ~3–4 min, and keeps the path short (see MAX_PATH below). The heavy compute (whisper.cpp, ONNX) is natively optimized regardless of the Rust profile, so runtime is fine.
- `--no-bundle` skips installer creation and thus **updater signing** (no `TAURI_SIGNING_PRIVATE_KEY` needed for a local build).
- Launch by double-clicking `Desktop\Mityu.bat` (a one-line `start "" "<repo>\target\debug\mityu.exe"`) — single-instance, no terminal, no port, no exe-lock.

### Producing a real installer (.msi/.exe) — the MAX_PATH gotcha
A full `pnpm run tauri:build:cpu` (release profile) FAILS in whisper.cpp's CMake step here:
```
error MSB6003: link.exe … DirectoryNotFoundException:
  '…\target\release\build\whisper-rs-sys-<hash>\out\build\CMakeFiles\CMakeScratch\TryCompile-<x>\…\<x>.tlog'
```
That `TryCompile-…\…tlog` path exceeds Windows **MAX_PATH (260)** — the repo path is already deep and contains a space, and `release` is 2 chars longer than `debug`, tipping it over. Fix: build the release into a **short target dir**:
```powershell
$env:CARGO_TARGET_DIR = "C:\t\mr"
pnpm run tauri:build:cpu      # installer lands in C:\t\mr\release\bundle\{nsis,msi}\
```
(Same MAX_PATH workaround used for the SQLCipher/B3 build.) It is a fresh from-scratch compile (~30–40 min) since the short target dir has no cache.

### The B3 one-time-conversion lock (first launch after SQLCipher landed)
The plaintext→SQLCipher conversion (`encryption::ensure_encrypted`) checkpoints the plaintext WAL, which needs an **exclusive** open. If a **stale `mityu.exe` still holds the DB** (a previous `tauri dev`/run not fully closed — orphans are common here), that checkpoint open fails and the conversion aborts. The ADR-0014 guardrailed fallback then degrades to a loud plaintext open (data stays visible) rather than hiding it — but a clean process state lets the conversion complete on the first try. Always clear lingering processes before a build or a first post-B3 launch:
```powershell
taskkill /F /IM mityu.exe 2>$null ; taskkill /F /IM cargo.exe 2>$null
```

## 6. Telemetry (PostHog) project key — disabled for v1.0.4

Until 2026-07-04 `frontend/src-tauri/src/analytics/commands.rs` hardcoded **upstream Meetily's** PostHog key — an opted-in user's telemetry went to the upstream vendor's project (ADR-0016). The key is now **build-time injected** and absent by default:

- Source: `MITYU_POSTHOG_API_KEY`, read at **compile time** (`option_env!`) in `analytics/commands.rs`. Unset/empty ⇒ opting in still records the preference, but telemetry is a local no-op (nothing is sent anywhere).
- Local/dev builds: leave it unset — telemetry-silent by construction.
- v1.0.4 release workflows deliberately do **not** expose `MITYU_POSTHOG_API_KEY`; production telemetry is therefore a local no-op even if a repository secret exists. Re-enabling remote telemetry requires a separately approved bluedev-owned processor, DPA, region, retention and erasure policy plus a reviewed workflow change.
- Invariants that must hold either way (CLAUDE.md §3): analytics is opt-in only (default OFF), no transcript/meeting content in events (`SENSITIVE_ANALYTICS_KEYS` strip), and fully disableable in-app.

## 7. Verifiable local deletion (C6a) — implementation gate closed; disclosure remains mandatory

ADR-0026 and migration `20260714010000_enable_verifiable_local_deletion` close the implementation/test gate for v1.0.4. Before release, retain all of these invariants in the final diff:

- Both layers are enabled: every SQLite connection uses core `secure_delete=ON`, while the FTS5 table persistently uses its separate `secure-delete=1` setting.
- A tenant-scoped delete atomically marks content-free maintenance pending; its disposable close-on-drop connection disables FK cascades and explicitly deletes only caller-workspace children, without reading or deleting a malformed foreign child. Success requires FTS optimization, checked WAL truncation, `VACUUM`, `freelist_count = 0` and a final checkpoint. Startup retries a pending marker.
- New recording folders use a collision-free workspace namespace and a Rust-resolved `metadata.json.workspace_id`. Filesystem removal validates that marker before touching known Mityu-managed artifacts under canonical configured/default recording roots; foreign markers, same-workspace duplicate references and outside-root targets fail closed. An unmarked legacy folder is local-only, symlinks are never followed and unknown user-added entries survive.
- IndexedDB recovery schema v2 obtains the trusted workspace from Rust, stores meetings/transcripts under workspace-compound keys and indexes, and migrates v1 records atomically into the implicit local workspace. Recovery data is logically purged only within that workspace after SQLite save, retried at startup and removed during explicit meeting deletion; retain the real v1→v2/cross-workspace integration test.
- `secure_local_deletion.rs` keeps its sentinel, foreign no-op, same-workspace collision and malformed-foreign-child preservation coverage; marker unit tests keep foreign ownership fail-closed and local-only legacy compatibility. Re-run them with the final Rust gate set.
- Keep the in-product limitation text: Mityu cannot promise physical erasure from SSD wear-leveling, copy-on-write storage, snapshots, backups, exports, swap or WebView/browser remnants. Do not replace this with “forensic,” “complete,” or “guaranteed” erasure language.
