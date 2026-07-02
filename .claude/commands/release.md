---
description: Run pre-release quality gates and cut a signed release of the desktop app (and optional server).
argument-hint: <version, e.g. 0.2.0>
---
Prepare release **$ARGUMENTS** (delegate to qa-release-engineer).

1. Enforce CLAUDE.md Definition of Done across the change set.
2. Run /security-review and /tenant-check; block on any BLOCKER.
3. Build + smoke test the matrix: macOS (Metal, CPU) + Windows (CUDA/Vulkan/CPU) → record→transcript→summary→export.
4. Verify **local-first** (fresh install runs fully offline) and **server-optional** (app runs with the server unreachable).
5. Package + sign: Tauri updater, macOS notarization, Windows code signing — signing secrets from CI/keychain, never committed.
6. Changelog, version bump, tag; verify auto-update path.
Report a go/no-go with evidence per step.
