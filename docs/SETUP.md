# SETUP — Developer environment

Goal: a reproducible local build of the Tauri (Rust) + Next.js app, with local transcription and local LLM working offline. The agent must verify each item, not assume it.

## Prerequisites
- **Rust** (stable, via rustup) + `cargo fmt`, `cargo clippy` components.
- **Node.js** (LTS) + **pnpm**.
- **Tauri 2 system deps** for the OS (WebView2 on Windows; Xcode CLT + WebKit on macOS; webkit2gtk/libsoup on Linux).
- **C/C++ toolchain for whisper-rs / llama-cpp:** the app builds whisper.cpp (via **whisper-rs**) and llama.cpp (via `llama-helper`) from source — needs **CMake** + a C++ toolchain (Windows: VS Build Tools "Desktop development with C++"; macOS: Xcode CLT). See `docs/BUILDING.md`. The `backend/whisper.cpp` git submodule is only needed when working on the archived Python backend — not for the app build.
- **Whisper model** (`large-v3` default): download in-app (model manager) or place it in the models path the engine expects.
- **Parakeet** engine assets per `parakeet_engine/` (verify model/license).
- **Ollama** (for local, offline summarization) with at least one instruct model pulled (e.g. a small local model) so the app works with no cloud key. The app also ships an embedded llama.cpp engine (`llama-helper` + in-app model manager) as a local alternative.
- **System audio capture:** no virtual audio device needed — ScreenCaptureKit or a Core Audio tap (macOS), WASAPI loopback (Windows). macOS: grant Microphone + Screen Recording permissions (macOS 13+). Linux: microphone capture only for now (system audio broken, ADR-0022).

## Build & run
```bash
cd frontend
pnpm install
pnpm run tauri:dev          # dev; Next.js on port 3118 (tauri-auto.js picks the GPU variant)
# GPU variants: pnpm run tauri:dev:cpu | :cuda | :vulkan | :metal | :coreml | :openblas | :hipblas
# clean build+run: ./clean_run.sh [debug] (macOS) · clean_run_windows.bat (Windows)
pnpm run tauri:build        # production build
# (git submodule update --init --recursive — only if working on the archived backend/)
```

## Secrets (never commit)
- Cloud LLM keys (OpenAI/Anthropic/Groq/OpenRouter) are entered by the user at runtime and stored in the **OS keychain / Tauri secure store**, not in files. For local dev without cloud, use **Ollama** only.

## Verify (definition of "environment ready")
1. App launches; you can start/stop a recording and see a live transcript (whisper or Parakeet).
2. A summary can be generated using **Ollama offline** (no network) — proves local-first.
3. `cargo clippy --all-targets` and `pnpm run lint && pnpm tsc --noEmit` run clean.
If any of these fail, fix the environment before writing features.
