---
description: Run pre-release quality gates and cut a signed release of the desktop app (and optional server).
argument-hint: <version, e.g. 0.2.0>
---
Prepare release **$ARGUMENTS** (delegate to qa-release-engineer).

1. Enforce CLAUDE.md Definition of Done across the change set.
2. Run /security-review and /tenant-check; block on any BLOCKER.
3. Build + smoke test the approved release matrix: normally macOS (Metal, CPU) + Windows (CUDA/Vulkan/CPU) → record→transcript→summary→export. A version-scoped ADR/release plan may narrow the shipped matrix; v1.0.4 is Windows x64 only and must not imply macOS validation.
4. Verify **local-first** (fresh install runs fully offline) and **server-optional** (app runs with the server unreachable).
5. Package + sign every platform in the approved matrix: Tauri updater plus platform signing/notarization as applicable. For Windows-only v1.0.4 this means the updater signature and Windows Authenticode; signing secrets come from CI/keychain and are never committed.
6. Changelog, version bump, tag; verify auto-update path.
Report a go/no-go with evidence per step.
