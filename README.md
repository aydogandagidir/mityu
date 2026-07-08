<div align="center" style="border-bottom: none">
    <h1>
        <img src="docs/mityu-logo.png" width="96" style="border-radius: 20px;" alt="Mityu" />
        <br>
        Mityu
    </h1>
    <h3>Privacy-first, local-first meeting &amp; conversation intelligence</h3>
    <a href="https://github.com/aydogandagidir/mityu/releases"><img src="https://img.shields.io/badge/License-MIT-blue" alt="License: MIT"></a>
    <img src="https://img.shields.io/badge/OS-macOS%20%7C%20Windows-white" alt="Supported OS: macOS, Windows">
    <img src="https://img.shields.io/badge/Local--first-offline%20capable-1E56FF" alt="Local-first, offline capable">
    <br><br>
    <p align="center">
Mityu records meetings and on-site conversations, transcribes them <b>on your device</b>, and turns them into structured, source-linked summaries and action items — with <b>no cloud and no server required</b>. Built for teams that need meeting intelligence without giving up privacy, compliance, or control.
    </p>
    <p align="center"><i>A <a href="https://bluedev.dev">bluedev</a> product.</i></p>
</div>

---

## Table of Contents

- [Introduction](#introduction)
- [Why Mityu?](#why-mityu)
- [Features](#features)
- [Installation](#installation)
- [System Architecture](#system-architecture)
- [Roadmap](#roadmap)
- [For Developers](#for-developers)
- [Contributing](#contributing)
- [License &amp; Acknowledgments](#license--acknowledgments)

## Introduction

Mityu is a **Tauri 2 desktop app** (Rust core + Next.js UI) that captures your meetings, transcribes them locally in real time, and generates summaries — without sending audio or transcripts to anyone else's servers. The capture → transcript → summary → store path runs entirely on your machine and keeps working with **no network and no server**. An optional sync/collaboration server can be added later for teams, but it is strictly additive: turn it off and the desktop app keeps working on your local data.

This makes Mityu a fit for professionals and enterprises who must keep control of sensitive conversations — legal, healthcare, defense, finance, and field work.

## Why Mityu?

- **Privacy-first.** Capture, transcription, and (by default) summarization run on your device. No cloud, no leaks.
- **Use any model.** Prefer a local open-source model? Great. Want to plug in an external API? Also fine. Bring your own key (BYOK) — no lock-in.
- **Cost-smart.** Avoid pay-per-minute bills by running models locally, or pay only for the calls you choose.
- **Any meeting app.** Google Meet, Zoom, Teams — or a face-to-face conversation. Mityu captures system audio, so it records regardless of platform (it is *not* a bot that joins your call). System-audio capture is supported on **macOS** (ScreenCaptureKit / Core Audio tap) and **Windows** (WASAPI loopback); **Linux is experimental** — microphone capture works, system-audio capture does not yet.
- **Human-in-the-loop.** AI summaries and action items are **drafts** bound to their source transcript segment until a human approves them — for trust, dispute evidence, and EU AI Act transparency.

## Features

- **Local transcription** with **Whisper** (`large-v3`) or **NVIDIA Parakeet** — no cloud required.
- **Real-time transcript** of the meeting as it happens.
- **AI summaries, BYOK:** choose Ollama (local), Claude, Groq, OpenRouter, or any OpenAI-compatible endpoint. API keys are stored in the OS keychain — never in plaintext.
- **Import &amp; enhance:** import existing audio to generate a transcript, or re-transcribe a recording with a different model or language — all processed locally.
- **Professional audio mixing:** capture microphone and system audio simultaneously with ducking and clipping prevention.
- **GPU acceleration:** Apple Silicon (Metal) + CoreML on macOS; NVIDIA (CUDA) and AMD/Intel (Vulkan) on Windows/Linux — enabled at build time, no configuration needed.
- **Multi-platform:** macOS and Windows (Linux builds from source).
- **Local-first storage:** recordings and transcripts stay on your machine in a local SQLite store.

<p align="center">
    <img src="docs/summary.png" width="640" style="border-radius: 10px;" alt="Source-linked AI summary" />
</p>
<p align="center">
    <img src="docs/audio.png" width="640" style="border-radius: 10px;" alt="Microphone + system audio device selection" />
</p>

> Screenshots reflect the app UI and are being refreshed for the Mityu brand.

## Installation

### 🪟 Windows

1. Download the latest `x64-setup.exe` from [Releases](https://github.com/aydogandagidir/mityu/releases/latest).
2. Run the installer.

### 🍎 macOS

1. Download the `.dmg` from [Releases](https://github.com/aydogandagidir/mityu/releases/latest).
2. Open it and drag **Mityu** to your Applications folder.

### 🐧 Linux

> **Experimental — microphone only.** Recording, transcription and summarization work, but
> **system-audio capture does not**: it needs a PulseAudio/PipeWire backend that is not built yet
> (see [ADR-0022](docs/DECISIONS.md)). Mityu will record the microphone and log a clear message
> instead of silently capturing nothing. Use it for in-person conversations; for online meetings
> prefer macOS or Windows.

Build from source:

```bash
git clone https://github.com/aydogandagidir/mityu
cd mityu/frontend
pnpm install
./build-gpu.sh
```

See [Building on Linux](docs/building_in_linux.md) and the [general build guide](docs/BUILDING.md).

> If no packaged release is published yet, build from source with the guides above.

## System Architecture

Mityu is a single, self-contained application built with [Tauri](https://tauri.app/): a Rust core handles capture, transcription, summarization, and local storage; a Next.js frontend provides the UI. There is **no required server** — an optional, authenticated multi-tenant server is a later, additive phase.

For details, see the [architecture documentation](docs/architecture.md).

## Roadmap

Mityu is developed local-first, then server-optional, with go/no-go gates (see [docs/ROADMAP.md](docs/ROADMAP.md)):

- **Phase 1 — Enterprise local-first MVP:** encrypted local store, source-linked HITL summaries, action-item extraction, search, export (PDF/DOCX/Markdown), consent &amp; transparency.
- **Phase 2 — Optional self-host server:** authenticated, multi-tenant (OIDC + RBAC + Postgres RLS + audit); shared workspaces; the app still works with the server off.
- **Phase 3 — Managed multi-tenant SaaS.**
- **Coming soon — on-device AI agents:** a library of local agents that draft follow-ups and track action items — draft-only, human-approved, no autonomous actions (see [EPIC F](docs/BACKLOG.md)).

## For Developers

You'll need Rust and Node.js. For detailed build instructions, see the [Building from Source guide](docs/BUILDING.md). Repository conventions, architecture, security/privacy, and the decision log live in [`docs/`](docs/), with contributor rules in [`CONTRIBUTING.md`](CONTRIBUTING.md).

## Contributing

Contributions are welcome. Please open an issue or a pull request and follow the project structure and guidelines in [CONTRIBUTING.md](CONTRIBUTING.md).

## License &amp; Acknowledgments

**MIT License** — see [`LICENSE.md`](LICENSE.md).

Mityu is built on the open-source **[Meetily](https://github.com/Zackriya-Solutions/meeting-minutes)** by **Zackriya Solutions** (MIT). The MIT copyright notice is preserved in `LICENSE.md`. **Mityu is a separate product by [bluedev](https://bluedev.dev) and is not affiliated with, nor endorsed by, Meetily or Zackriya Solutions**; it does not use the Meetily name or branding.

Additional thanks:

- [whisper.cpp](https://github.com/ggerganov/whisper.cpp) and [transcribe-rs](https://crates.io/crates/transcribe-rs) for on-device transcription.
- [Screenpipe](https://github.com/mediar-ai/screenpipe) for audio-capture code we build on.
- **NVIDIA** for the **Parakeet** model, and [istupakov](https://huggingface.co/istupakov/parakeet-tdt-0.6b-v3-onnx) for its ONNX conversion.
- The upstream Meetily contributors whose work (including import &amp; enhance) Mityu inherits.
