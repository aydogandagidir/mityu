# Release checklist — brand & security gates

Operational steps that must pass before Mityu's first public release. Complements the `/release` command and the BACKLOG gates (C8/D5). Local-first + HITL invariants are enforced elsewhere; this file covers the brand / keys / supply-chain items surfaced during the bluedev rebrand (ADR-0006 / 0009 / 0013).

## 1. Updater signing key (CRITICAL) — regenerate, replace upstream's

The app auto-updates by verifying a signature against the public key baked into `frontend/src-tauri/tauri.conf.json` (`plugins.updater.pubkey`). It currently holds **upstream's** key (ADR-0009) — we cannot sign updates until we own the keypair.

1. Generate a Mityu keypair (keep the private key OUT of git):
   ```bash
   # from the mityu-app repo root (NOT the docs-only ../mityu folder):
   cd mityu-app/frontend
   mkdir -p "$HOME/.tauri"    # PowerShell: New-Item -ItemType Directory -Force "$HOME\.tauri" | Out-Null
   pnpm tauri signer generate -w "$HOME/.tauri/mityu_updater.key"
   # prompts for a password; prints the PUBLIC key (also written to <path>.pub)
   ```
2. Paste the printed **public key** into `tauri.conf.json` → `plugins.updater.pubkey` (replace the existing value).
3. Add GitHub repo secrets (Settings → Secrets and variables → Actions):
   - `TAURI_SIGNING_PRIVATE_KEY` = full contents of `~/.tauri/mityu_updater.key`
   - `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` = the password you chose
4. Verify: a release build emits `*.sig` files; `latest.json` URLs point to `aydogandagidir/mityu` (already fixed in `scripts/generate-update-manifest-github.js`); an older build updates to the new one.

## 2. Upstream PRO licensing (decide — currently dormant)

CI passes `MEETILY_RSA_PUBLIC_KEY` + `SUPABASE_URL` / `SUPABASE_ANON_KEY` — upstream Meetily PRO's **license-activation** system, which Mityu does **not** ship. With those secrets unset the feature is inert (nothing to do to release). If you later build your own licensing, add `MITYU_*` secrets and rename the CI env vars; otherwise you may delete those `env:` lines from `.github/workflows/build*.yml`.

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
