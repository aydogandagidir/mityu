# DevTest build workflow

`build-devtest.yml` creates unsigned, non-production packages for development and testing. It never receives production signing credentials, never creates a tag or GitHub release, and disables Tauri updater artifacts.

## Run it

1. Open GitHub Actions and select **Build and Test - DevTest**.
2. Choose the branch.
3. Leave **Upload verified workflow artifacts** enabled unless compilation-only evidence is enough.
4. Run the workflow.

There is deliberately no signing option. Production Authenticode and Tauri updater signing are available only through `release.yml` in the protected `Production` environment.

## Matrix

| Platform | Target | Package scope |
|---|---|---|
| macOS Apple Silicon | `aarch64-apple-darwin` | Development DMG/app |
| Windows x64 | `x86_64-pc-windows-msvc` | Development MSI/NSIS |
| Ubuntu 22.04 | `x86_64-unknown-linux-gnu` | DEB |
| Ubuntu 24.04 | `x86_64-unknown-linux-gnu` | AppImage/RPM |

All jobs reuse `build.yml`, install with the frozen pnpm lockfile, use pinned GitHub Actions and Rust 1.95.0, and upload artifacts with the `mityu-devtest` prefix when requested.

## Interpreting artifacts

DevTest files are intentionally unsigned and are not release candidates for public distribution. Operating-system trust warnings are expected. Use them for functional and packaging smoke tests only; do not rename or publish them as production artifacts.

For a release, follow `docs/RELEASE_CHECKLIST.md` and dispatch `release.yml` from `main`. That path requires the production signing secrets, verifies Windows signatures and updater metadata, and creates an unpublished draft for final inspection.
