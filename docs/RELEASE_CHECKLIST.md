# Release checklist — brand & security gates

Operational steps that must pass before Mityu's first public release. Complements the `/release` command and the BACKLOG gates (C8/D5). Local-first + HITL invariants are enforced elsewhere; this file covers the brand / keys / supply-chain items surfaced during the bluedev rebrand (ADR-0006 / 0009 / 0013).

## 1. Updater signing key (CRITICAL) — secrets + release verification still open

The app auto-updates by verifying a signature against the public key baked into `frontend/src-tauri/tauri.conf.json` (`plugins.updater.pubkey`). **Done already (2026-07-02):** the Mityu keypair was generated (private key at `~/.tauri/mityu_updater.key`, password-protected — keep it OUT of git; losing it or its password breaks the update chain for every existing install) and the baked pubkey was replaced with Mityu's in commit `3468a04`, so the config no longer trusts upstream's key. If it ever needs regenerating:
   ```bash
   # from the mityu-app repo root (NOT the docs-only ../mityu folder):
   cd mityu-app/frontend
   mkdir -p "$HOME/.tauri"    # PowerShell: New-Item -ItemType Directory -Force "$HOME\.tauri" | Out-Null
   pnpm tauri signer generate -w "$HOME/.tauri/mityu_updater.key"
   # prompts for a password; prints the PUBLIC key (also written to <path>.pub)
   # then paste the printed PUBLIC key into tauri.conf.json → plugins.updater.pubkey
   ```
Still OPEN before the first release:
1. Add GitHub repo secrets (Settings → Secrets and variables → Actions):
   - `TAURI_SIGNING_PRIVATE_KEY` = full contents of `~/.tauri/mityu_updater.key`
   - `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` = the password you chose
2. Verify: a release build emits `*.sig` files; `latest.json` URLs point to `aydogandagidir/mityu` (already fixed in `scripts/generate-update-manifest-github.js`); an older build updates to the new one.

## 2. Upstream PRO licensing (resolved — dead secrets removed)

CI used to pass `MEETILY_RSA_PUBLIC_KEY` + `SUPABASE_URL` / `SUPABASE_ANON_KEY` — upstream Meetily PRO's **license-activation** system, which Mityu does **not** ship (confirmed zero references anywhere in `frontend/src-tauri/src/`). Those `env:` lines have been removed from every workflow that had them (`build.yml`, `build-windows.yml`, `build-macos.yml`, `build-linux.yml`, `build-devtest.yml`); the matching GitHub repo secrets, if ever configured, are no longer read by anything and can be deleted too. If you later build your own licensing, add fresh `MITYU_*`-prefixed secrets and wire them in explicitly rather than reviving these names.

## 3. Third-party binaries & models (supply-chain)

Two dependencies still download from upstream-controlled hosts (ADR-0009 flagged the model CDN):
- **FFmpeg** — `frontend/src-tauri/build/ffmpeg.rs` fetches from `github.com/Zackriya-Solutions/ffmpeg-binaries`.
- **Parakeet model** — `frontend/src-tauri/src/parakeet_engine/parakeet_engine.rs` fetches from `meetily.towardsgeneralintelligence.com`.

Before GA, either accept the dependency or **mirror** each to infrastructure you control (a `bluedev` / `aydogandagidir` GitHub release for the ffmpeg zips; your own host or the official HF repo `istupakov/parakeet-tdt-0.6b-v3-onnx` for the model), then repoint those URLs and verify checksums.

## 4. Build & verify (toolchain)

The Rust core builds whisper.cpp from source, needing a C toolchain:
- Install **LLVM/Clang 18.x** (NOT 22 — bindgen breaks), **CMake**, **VS 2022 Build Tools** (Parakeet's onnxruntime is VS2022-built), and **Strawberry Perl** (the SQLCipher/OpenSSL vendored build, ADR-0014, needs real Perl — git's MSYS perl fails). Windows: `winget install LLVM.LLVM Kitware.CMake StrawberryPerl.StrawberryPerl`.
- Set the env in the shell first — **this is what the earlier "build blocked" was** (tools installed but not on `PATH`), PowerShell:
  ```powershell
  $env:Path += ";C:\Program Files\CMake\bin;C:\Program Files\LLVM\bin;C:\Strawberry\perl\bin"
  $env:LIBCLANG_PATH   = "C:\Program Files\LLVM\bin"           # whisper-rs bindgen
  $env:OPENSSL_SRC_PERL = "C:\Strawberry\perl\bin\perl.exe"    # SQLCipher/OpenSSL build
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

## 6. Telemetry (PostHog) project key — pre-GA gate

Until 2026-07-04 `frontend/src-tauri/src/analytics/commands.rs` hardcoded **upstream Meetily's** PostHog key — an opted-in user's telemetry went to the upstream vendor's project (ADR-0016). The key is now **build-time injected** and absent by default:

- Source: `MITYU_POSTHOG_API_KEY`, read at **compile time** (`option_env!`) in `analytics/commands.rs`. Unset/empty ⇒ opting in still records the preference, but telemetry is a local no-op (nothing is sent anywhere).
- Local/dev builds: leave it unset — telemetry-silent by construction.
- Release builds: create a **bluedev-owned** PostHog project, then add the GitHub secret `MITYU_POSTHOG_API_KEY` (already wired into the `env:` of every `build*.yml` tauri-action step). Deciding NOT to set it is acceptable (binaries ship telemetry-off) — but decide deliberately.
- Invariants that must hold either way (CLAUDE.md §3): analytics is opt-in only (default OFF), no transcript/meeting content in events (`SENSITIVE_ANALYTICS_KEYS` strip), and fully disableable in-app.
