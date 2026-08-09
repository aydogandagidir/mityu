#!/usr/bin/env python3
"""Fetch and VERIFY the sherpa-onnx native archive before cargo can download it.

WHY THIS EXISTS (ADR-0035 decision 4)
-------------------------------------
`sherpa-onnx-sys`' own `build.rs` downloads a ~20-120 MB prebuilt native archive
from a GitHub release and performs **no integrity check of any kind** — no
digest, no size, nothing. That is squarely against ADR-0020, which pins model
downloads by exact byte length *and* SHA-256.

The obvious fix does not work and is worth stating so it is not re-attempted: a
`build/ffmpeg.rs`-style check inside our own crate's build script runs TOO LATE.
Cargo builds a dependency's script before the dependent crate's, and a build
script cannot set environment for a sibling that has already run — the unverified
bytes would be on disk and unpacked before our check executed. So verification
has to happen OUTSIDE cargo, which is what this is.

WHEN IT MUST RUN
----------------
Before `cargo build`/`cargo test` **in any workspace that contains
`sherpa-onnx-sys`** — which is why it is a prerequisite for adding
`diarize-helper` to the workspace, not merely for shipping. `ci.yml` runs
`cargo test --all` on every push and pull request, so from the moment the helper
is a member, every PR would otherwise perform that unverified download on a
runner, unattended.

HOW IT PLUGS IN
---------------
`sherpa-onnx-sys` honours `SHERPA_ONNX_ARCHIVE_DIR`: it looks for
`<dir>/<exact archive name>` and **errors if it is absent** rather than falling
back to downloading. So handing it a directory we have verified is enough, and a
mistake here fails the build instead of silently restoring the download path.

    python tools/diarization/fetch-sherpa-archive.py            # prints the dir
    export SHERPA_ONNX_ARCHIVE_DIR="$(python tools/... )"       # shell
    python tools/diarization/fetch-sherpa-archive.py --github-env   # CI

DIVERGENCE FROM `build/ffmpeg.rs`, ON PURPOSE
---------------------------------------------
That script *warns* and proceeds when a target has no approved checksum ("not
production-release eligible"), because it supports targets we do not release on.
This one **fails**. An unpinned target here means precisely the unverified
download this tool exists to prevent, so a warning would defeat it.
"""

import argparse
import hashlib
import os
import sys
import urllib.request

# The sherpa-onnx-sys version whose `archive_name()` these names mirror. If the
# dependency is bumped, every name and pin below must be recomputed -- the names
# embed the version, so a stale table fails loudly (the archive will not be found
# under the new name) rather than silently verifying the wrong file.
SHERPA_VERSION = "1.13.4"
RELEASE_BASE = f"https://github.com/k2-fsa/sherpa-onnx/releases/download/v{SHERPA_VERSION}"

# STATIC link mode only: ADR-0035 decision 3 ships one self-contained sidecar
# binary with no sibling DLLs. If `shared` is ever needed, it is a different set
# of archives and needs its own pins.
#
# Names mirror `archive_name()` in sherpa-onnx-sys' build.rs exactly; a
# mismatch would make the sys crate look for a file we never wrote and fail.
PINS = {
    "x86_64-pc-windows-msvc": {
        "archive": f"sherpa-onnx-v{SHERPA_VERSION}-win-x64-static-MT-Release-lib.tar.bz2",
        "size": 119847445,
        "sha256": "d81bd1d25112540862d2387072e76b2b6843ef962918d6b5c7db5a19c6276b4c",
    },
    "x86_64-unknown-linux-gnu": {
        "archive": f"sherpa-onnx-v{SHERPA_VERSION}-linux-x64-static-lib.tar.bz2",
        "size": 22276804,
        "sha256": "98b0e31996426f6e78244dbce1955548f2c64e8f01c4be75b85af7cdaa2e8d5c",
    },
    "aarch64-unknown-linux-gnu": {
        "archive": f"sherpa-onnx-v{SHERPA_VERSION}-linux-aarch64-static-lib.tar.bz2",
        "size": 20678521,
        "sha256": "23b33616787cc949d5b1438e9794550f805e208a014c5c2245483207c58bbc0f",
    },
    "x86_64-apple-darwin": {
        "archive": f"sherpa-onnx-v{SHERPA_VERSION}-osx-x64-static-lib.tar.bz2",
        "size": 19336737,
        "sha256": "2bda2c10b31a1cfc45d9f9e14bd4983743ec3779d309e42d99a6c8fa1689043f",
    },
    "aarch64-apple-darwin": {
        "archive": f"sherpa-onnx-v{SHERPA_VERSION}-osx-arm64-static-lib.tar.bz2",
        "size": 19551872,
        "sha256": "57801db2bbb786a5d343f515a38ff210b401842338bdc804fa075312d1cd2404",
    },
}


def host_target() -> str:
    """Best-effort host triple, in the spelling cargo uses."""
    import platform

    machine = platform.machine().lower()
    arch = {
        "amd64": "x86_64",
        "x86_64": "x86_64",
        "arm64": "aarch64",
        "aarch64": "aarch64",
    }.get(machine)
    system = platform.system().lower()
    if arch is None:
        return f"UNKNOWN-{system}-{machine}"
    if system == "windows":
        return f"{arch}-pc-windows-msvc"
    if system == "linux":
        return f"{arch}-unknown-linux-gnu"
    if system == "darwin":
        return f"{arch}-apple-darwin"
    return f"UNKNOWN-{system}-{machine}"


def verify(path: str, expected_size: int, expected_sha256: str) -> None:
    """Size first, then digest. Raises on any mismatch.

    The manifest entry itself is validated before it is trusted: a typo'd or
    truncated pin would otherwise silently never match, or worse, be mistaken
    for a deliberate "no pin". Mirrors `utils::verify_file_integrity`.
    """
    if len(expected_sha256) != 64 or not all(
        c in "0123456789abcdefABCDEF" for c in expected_sha256
    ):
        raise SystemExit(
            f"FATAL: pin for this target is not a 64-character hex SHA-256: "
            f"{expected_sha256!r}. Refusing to 'verify' against a malformed pin."
        )
    if expected_size <= 0:
        raise SystemExit(
            f"FATAL: pin for this target has no byte length ({expected_size}). "
            "ADR-0020 requires exact size AND digest."
        )

    actual_size = os.path.getsize(path)
    if actual_size != expected_size:
        raise SystemExit(
            f"FATAL: {path} is {actual_size} bytes, expected {expected_size}. "
            "Refusing to hand an unexpected archive to the build."
        )

    h = hashlib.sha256()
    with open(path, "rb") as fh:
        while True:
            chunk = fh.read(1 << 20)
            if not chunk:
                break
            h.update(chunk)
    actual = h.hexdigest()
    if actual.lower() != expected_sha256.lower():
        raise SystemExit(
            f"FATAL: {path} SHA-256 is {actual}, expected {expected_sha256.lower()}. "
            "Refusing to hand an unverified archive to the build."
        )


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--target", default=None, help="rust target triple (default: host)")
    ap.add_argument(
        "--dir",
        default=None,
        help="cache directory for the archive (default: <repo>/target/sherpa-archive)",
    )
    ap.add_argument(
        "--github-env",
        action="store_true",
        help="append SHERPA_ONNX_ARCHIVE_DIR to $GITHUB_ENV as well as printing it",
    )
    ap.add_argument(
        "--list-targets", action="store_true", help="print the pinned targets and exit"
    )
    args = ap.parse_args()

    if args.list_targets:
        for t, p in sorted(PINS.items()):
            print(f"{t}\t{p['archive']}\t{p['size']}\t{p['sha256']}")
        return 0

    target = args.target or host_target()
    pin = PINS.get(target)
    if pin is None:
        # Deliberately fatal -- see the module docstring. A warning here would
        # let the build fall back to the unverified download.
        raise SystemExit(
            f"FATAL: no pinned sherpa-onnx archive for target {target}.\n"
            f"Pinned targets: {', '.join(sorted(PINS))}\n"
            "Add a pin (archive name, exact byte length, SHA-256) before building "
            "for this target -- an unpinned target means an unverified download."
        )

    cache = args.dir or os.path.join(
        os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__)))),
        "target",
        "sherpa-archive",
    )
    os.makedirs(cache, exist_ok=True)
    # The name must match sherpa-onnx-sys' `archive_name()` exactly; that is what
    # it looks for inside SHERPA_ONNX_ARCHIVE_DIR.
    dest = os.path.join(cache, pin["archive"])

    if not os.path.exists(dest):
        url = f"{RELEASE_BASE}/{pin['archive']}"
        print(f"fetching {url}", file=sys.stderr)
        part = dest + ".part"
        with urllib.request.urlopen(url, timeout=600) as r, open(part, "wb") as fh:
            while True:
                chunk = r.read(1 << 20)
                if not chunk:
                    break
                fh.write(chunk)
        # Rename only after the transfer completes, so an interrupted download
        # can never be picked up as a cached archive on the next run.
        os.replace(part, dest)

    # Re-verified on EVERY run, not only after downloading: a cached file can be
    # replaced, truncated or corrupted between runs, and the whole point is that
    # nothing unverified reaches the build.
    verify(dest, pin["size"], pin["sha256"])
    print(f"verified {pin['archive']} ({pin['size']} bytes)", file=sys.stderr)

    if args.github_env:
        gh = os.environ.get("GITHUB_ENV")
        if not gh:
            raise SystemExit("FATAL: --github-env given but GITHUB_ENV is not set")
        with open(gh, "a", encoding="utf-8") as fh:
            fh.write(f"SHERPA_ONNX_ARCHIVE_DIR={cache}\n")

    # stdout is the directory alone, so a shell can capture it directly.
    print(cache)
    return 0


if __name__ == "__main__":
    sys.exit(main())
