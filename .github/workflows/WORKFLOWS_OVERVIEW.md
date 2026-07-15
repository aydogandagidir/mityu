# GitHub Actions Workflows Overview

This document provides a quick overview of all available CI/CD workflows in this repository.

**Note:** Most workflows in this repository — every build and release workflow below — use **manual triggers only** (`workflow_dispatch`). The exception is **`ci.yml`**, which triggers **automatically** on push (`main`, `feat/**`, `fix/**`, `chore/**`, `docs/**`) and on every pull request, in addition to supporting manual dispatch.

## Workflow Files

### 1. **ci.yml** - Continuous Integration
**Purpose:** Lint, type-check, and test every push and pull request — the automatic quality gate (not a build/release workflow, produces no artifacts)

**Key Features:**
- `rust` job: `cargo fmt --all --check`, `cargo clippy --all-targets`, `cargo test --all`
- `frontend` job: exact Node 20.19.4 + pnpm 10.33.0, `pnpm install --frozen-lockfile`, `pnpm run lint`, `pnpm tsc --noEmit`, `pnpm test`
- `server-isolation` job: fails the build if `server/` exists without a `*cross_tenant*` isolation test (no-op guard until `server/` ships, see CLAUDE.md §5)
- No build artifacts, no signing, no releases

**Triggers:**
- **Automatic** — push to `main`, `feat/**`, `fix/**`, `chore/**`, `docs/**`
- **Automatic** — every pull request
- Also supports manual `workflow_dispatch`

**Use When:**
- N/A for manual use — it runs on its own for every push/PR. Dispatch it manually only to re-run the check without a new commit.

---

### 2. **build-devtest.yml** - DevTest Builds
**Purpose:** Fast builds for development and testing

**Key Features:**
- Signing always OFF; production credentials and updater artifacts are unavailable
- All platforms in parallel
- Verified artifacts uploaded on request

**Triggers:**
- Manual dispatch only

**Use When:**
- Regular development work
- Testing features
- Need fast feedback

---

### 3. **build-macos.yml** - macOS Standalone Builds
**Purpose:** Build and test specifically for Apple Silicon (M1/M2/M3)

**Key Features:**
- Unsigned development package only
- Production credentials and updater artifacts are unavailable
- macOS-focused optimizations

**Triggers:**
- Manual dispatch only

**Use When:**
- macOS-specific development
- Testing Metal GPU acceleration
- Verifying macOS-specific features

**Outputs:**
- `.dmg` installer
- `.app` bundle

---

### 4. **build-windows.yml** - Windows Standalone Builds
**Purpose:** Build and test specifically for Windows x64

**Key Features:**
- Thin wrapper around the same pinned reusable build used by production
- Unsigned development package only
- DigiCert and updater credentials are unavailable
- MSI installer validation

**Triggers:**
- Manual dispatch only

**Use When:**
- Windows-specific development
- Testing CUDA/Vulkan GPU acceleration
- Verifying Windows-specific features

**Outputs:**
- `.msi` installer
- `.exe` NSIS installer

---

### 5. **build-linux.yml** - Linux Standalone Builds
**Purpose:** Build and test for Linux distributions

**Key Features:**
- Support for Ubuntu 22.04 and 24.04
- Multiple bundle formats (DEB, AppImage, RPM)
- No updater signing or production credentials
- AppImage compatibility fixes
- Package verification

**Triggers:**
- Manual dispatch only

**Use When:**
- Linux-specific development
- Testing Vulkan GPU acceleration
- Verifying package formats

**Outputs:**
- `.deb` package (Ubuntu/Debian)
- `.AppImage` portable
- `.rpm` package (Fedora/RHEL)

---

### 6. **build-test.yml** - Multi-Platform Test Builds
**Purpose:** Test builds across all platforms without production signing keys

**Key Features:**
- Signing OFF; production updater keys are unavailable to this workflow
- All platforms in parallel
- Uses reusable `build.yml` workflow
- 30-day artifact retention
- Artifacts prefixed with `mityu-test-`

**Triggers:**
- Manual dispatch only

**Use When:**
- Pre-release testing
- Testing across all platforms simultaneously

---

### 7. **build.yml** - Reusable Build Workflow
**Purpose:** Shared workflow used by other workflows

**Key Features:**
- Reusable workflow (called by others)
- Highly configurable inputs
- Used by `build-test.yml` and `release.yml`

**Not directly triggered** - used as a building block

---

### 8. **release.yml** - Production Release
**Purpose:** Create official releases with signed binaries

**Key Features:**
- Signing REQUIRED
- Creates GitHub Release (draft)
- Version tags from `tauri.conf.json`
- Uploads release assets
- **Windows x64 only for v1.0.4**, matching the published v1.0.3 platform scope; macOS remains gated on FFmpeg provenance, signing/notarization and physical smoke tests
- Auto-generates `latest.json` for Tauri updater
- Runs the reusable Rust/frontend CI suite before creating a draft or tag
- Validates package, Tauri, Cargo and Cargo.lock versions are identical
- Runs only from `main` and pins the build to the dispatched commit SHA
- Fails before creating a draft/tag when a required production signing secret is absent
- Rejects an existing tag; release versions are always chosen explicitly as SemVer
- Verifies exact Windows installers, manifest version/repository URLs and the remotely uploaded NSIS updater signature cryptographically against the public key baked into the release commit

**Triggers:**
- Manual dispatch only

**Use When:**
- Ready to publish a new version
- Creating official release artifacts

**Outputs:**
- GitHub Release (draft)
- Windows: MSI installer (signed), NSIS installer (signed), .sig files
- Updater manifest: latest.json
- Release notes auto-generated

**Version Behavior:**
- If all canonical sources say `1.0.4` and `v1.0.4` does not exist, creates draft `v1.0.4`.
- If `v1.0.4` already exists, stops and requires an explicit new SemVer; it never invents a four-part version.
- If a failed build left an unpublished draft/tag, follow `docs/RELEASE_CHECKLIST.md` to remove that failed draft safely before retrying.

**Note:** Linux and macOS are not included in the v1.0.4 production release. Use their platform workflows for development/testing; do not add macOS back to production until its documented release gates pass.

---

### 9. **pr-main-check.yml** - Validation Check
**Purpose:** Quick validation of version and configuration

**Key Features:**
- No builds triggered
- Validates version format
- Shows current branch info
- Provides next steps guidance

**Triggers:**
- Manual dispatch only

**Use When:**
- Quick configuration check
- Before running full builds

---

## How to Run Workflows

1. **Go to Actions tab** in GitHub repository
2. **Select workflow** from left sidebar
3. **Click "Run workflow"** button
4. **Select branch** to run against
5. **Configure the exposed non-secret options** (build type, bundle type, artifact upload)
6. **Click "Run workflow"** to start
7. **Monitor progress** in the Actions tab

---

## Quick Decision Guide

### "I'm developing a new feature..."
- **Use `build-devtest.yml`** (manual dispatch)
- Cross-platform unsigned test packages

### "I need to test macOS-specific code..."
- **Use `build-macos.yml`** (manual dispatch)
- Focus on macOS
- Unsigned development package

### "I need to test Windows-specific code..."
- **Use `build-windows.yml`** (manual dispatch)
- Focus on Windows
- Unsigned development package

### "I need to test Linux packages..."
- **Use `build-linux.yml`** (manual dispatch)
- Choose Ubuntu version
- Choose bundle types

### "I need a signed build..."
- Use `release.yml` only after all release gates pass. Signing is intentionally unavailable to manual development wrappers.
- Production signing remains exclusive to the protected `Production` environment.

### "I'm ready to release..."
- **Use `release.yml`** (manual dispatch)
- Creates GitHub Release
- Windows x64, fully signed
- Production-ready artifacts

---

## Workflow Dependencies

```
ci.yml — automatic (push + pull_request), also manually dispatchable;
         independent of everything below (no build.yml, no artifacts)

build.yml (reusable)
    |-- build-test.yml (calls build.yml)
    |-- release.yml (calls build.yml)

Other callers of `build.yml`:
    |-- build-macos.yml
    |-- build-windows.yml
    |-- build-linux.yml
    |-- build-devtest.yml
    |-- pr-main-check.yml (validation only)
```

---

## Comparison Matrix

| Workflow | Platforms | Default Signing | Speed | Retention | Use Case |
|----------|-----------|----------------|-------|-----------|----------|
| `build-devtest.yml` | All | OFF | Fast | 30 days | Development |
| `build-macos.yml` | macOS | OFF | Medium | 30 days | macOS dev |
| `build-windows.yml` | Windows | OFF | Medium | 30 days | Windows dev |
| `build-linux.yml` | Linux | OFF | Medium | 30 days | Linux dev |
| `build-test.yml` | All | OFF | Slow | 30 days | Cross-platform test |
| `release.yml` | Windows x64 | REQUIRED | Slow | Permanent | v1.0.4 release |

---

## Artifact Naming Convention

Artifact containers use `{asset-prefix}-{target}` on Windows/macOS and `{asset-prefix}-{runner}-{target}` on Linux. Versioned installer filenames inside those containers are generated by Tauri.

**Examples:**
- `mityu-devtest-aarch64-apple-darwin`
- `mityu-test-x86_64-pc-windows-msvc`
- `mityu-devtest-ubuntu-22.04-x86_64-unknown-linux-gnu`

---

## Required Secrets

Configure only the secrets required by the workflow/platform being run. The v1.0.4 production release requires the Windows and Tauri Updater groups below. macOS secrets are reserved for a future signed macOS workflow after that platform returns to the production matrix; current macOS workflows are unsigned and cannot consume them.

### macOS Signing
- `APPLE_CERTIFICATE` - Developer ID certificate (base64)
- `APPLE_CERTIFICATE_PASSWORD` - Certificate password
- `APPLE_ID` - Apple ID email
- `APPLE_PASSWORD` - App-specific password
- `APPLE_TEAM_ID` - Team ID
- `KEYCHAIN_PASSWORD` - Temporary keychain password

### Windows Signing (DigiCert)
- `SM_HOST` - DigiCert host URL
- `SM_API_KEY` - API key
- `SM_CLIENT_CERT_FILE_B64` - Client cert (base64)
- `SM_CLIENT_CERT_PASSWORD` - Client cert password
- `SM_CODE_SIGNING_CERT_SHA1_HASH` - Certificate hash

### Tauri Updater (All Platforms)
- `TAURI_SIGNING_PRIVATE_KEY` - Ed25519 private key
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` - Key password

### Application Configuration
- `MITYU_POSTHOG_API_KEY` - deliberately unavailable to v1.0.4 workflows; production telemetry remains a local no-op until processor/DPA, region, retention and erasure governance are approved
- `MITYU_POLAR_ORG_ID` - Optional Polar.sh organization-id override; unset builds use the public production Mityu organization id embedded per ADR-0023
- `NEXT_PUBLIC_MITYU_CHECKOUT_URL` - **repo variable** (not secret): Buy-button destination, inlined into the frontend at build time; unset = falls back to the live pricing page (ADR-0023)

---

## Performance Tips

1. **Use devtest workflow** for routine development (fastest)
2. **Use `release.yml` for signing** only after release approval; development workflows cannot access signing credentials
3. **Test specific platforms** when working on platform-specific code
4. **Run full builds** (`build-test.yml`) before releases
5. **Cache is enabled** - subsequent builds are faster

---

## Troubleshooting

### Build fails with version error (Windows MSI)
- Ensure version in `tauri.conf.json` doesn't contain non-numeric pre-release identifiers
- Use `0.1.3` not `0.1.2-pro-trial`

### Signing fails
- Verify all required secrets are configured
- Check secret expiration dates
- Review workflow logs for specific errors

### Artifacts not available
- Check build succeeded completely
- Artifacts expire based on retention period
- Ensure `upload-artifacts` is enabled

### Workflow not appearing in Actions
- Verify YAML syntax is valid
- Check file is in `.github/workflows/` directory
- Ensure file extension is `.yml` or `.yaml`

---

## Support

For issues with workflows:
1. Check workflow logs in Actions tab
2. Review this documentation
3. Check `README_DEVTEST.md` for devtest-specific help
4. Check `ACCELERATION_GUIDE.md` for GPU/performance info
