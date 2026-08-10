#!/usr/bin/env python3
"""Fail if a built binary carries GPL-3.0 espeak-ng code.

WHY THIS EXISTS
---------------
`sherpa-onnx-sys` upstream fetches archives built with `SHERPA_ONNX_ENABLE_TTS=ON`
and names `espeak-ng` in its static link list. eSpeak NG is **GPL-3.0**. Shipping
it inside Mityu's closed installer would distribute GPL-3.0 object code without
GPL terms or corresponding source.

That was not theoretical. Before the fix, the RELEASE `diarize-helper` carried 51
espeak function symbols; `/OPT:REF` did not discard them, which had to be
measured rather than assumed because the same optimisation *did* discard all of
sherpa in an earlier link spike. The fix — a vendored `sherpa-onnx-sys` pointing
at upstream's own `-no-tts` archives — is one line away from silently reverting
whenever the crate or the pins are bumped, so it is checked here on every build.

DETECTION, AND THE TRAP IN IT
-----------------------------
Do NOT grep case-insensitively for "espeak". A clean binary still matches, ~12
times, because `OfflineSpeakerDiarization` and `wespeaker` (an unrelated speaker
embedding toolkit) contain the letters "eSpeak". A check that cries wolf on a
clean binary gets disabled, which is worse than no check.

So this looks for markers that only real espeak-ng object code produces:
its exported symbols, its runtime error strings, and its Windows registry keys.

USAGE
-----
    python tools/diarization/check-no-gpl-espeak.py path/to/diarize-helper.exe
    python tools/diarization/check-no-gpl-espeak.py --self-test
"""

import argparse
import re
import sys

# Symbols exported by espeak-ng itself. `\bespeak_[A-Za-z]` is case-SENSITIVE and
# anchored, so `OfflineSpeakerDiarization` cannot match.
SYMBOL = re.compile(rb"\bespeak_[A-Za-z][A-Za-z_]*")

# Strings only espeak-ng's own source emits.
MARKERS = [
    b"The espeak-ng library has not been initialized",
    b"Wrong version of espeak-ng-data",
    b"The specified espeak-ng voice does not exist",
    b"Failed to set eSpeak-ng voice",
    b"espeak-ng-data",
    b"ESPEAK_DATA_PATH",
    b"Software\\eSpeak NG",
    b"phonemize_eSpeak",
]


def scan(blob: bytes):
    """Return (symbols, markers) found. Empty both = clean."""
    symbols = sorted({m.decode("ascii", "replace") for m in SYMBOL.findall(blob)})
    markers = [m.decode("ascii", "replace") for m in MARKERS if m in blob]
    return symbols, markers


def self_test() -> int:
    """Prove the detector both fires and stays quiet -- a check nobody has seen
    fail is indistinguishable from a check that cannot fail."""
    # NUL-separated, the way strings actually sit in a binary. Concatenating
    # them without separators is what a real binary never does, and it hides
    # the `\b` anchor: "junkespeak_Initialize" has no word boundary to match.
    dirty = b"\x00espeak_Initialize\x00Wrong version of espeak-ng-data\x00"
    syms, marks = scan(dirty)
    if not syms or not marks:
        print("SELF-TEST FAILED: detector did not fire on planted espeak markers")
        return 1

    # The exact strings a CLEAN binary really contains -- taken from the shipped
    # no-tts build, all of which merely spell "Speaker".
    clean = (
        b".?AVOfflineSpeakerDiarizationPyannoteImpl@sherpa_onnx@@"
        b"SherpaOnnxCreateOfflineSpeakerDiarization"
        b"Expect a wespeaker or a 3d-speaker model, given: %s"
        b"OfflineSpeakerSegmentationPyannoteModelConfig("
    )
    syms, marks = scan(clean)
    if syms or marks:
        print(f"SELF-TEST FAILED: false positive on clean input: {syms} {marks}")
        return 1

    print("self-test ok: fires on espeak markers, silent on 'Speaker' lookalikes")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("binary", nargs="?", help="binary to scan")
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args()

    if args.self_test:
        return self_test()
    if not args.binary:
        ap.error("give a binary, or --self-test")

    with open(args.binary, "rb") as fh:
        blob = fh.read()
    symbols, markers = scan(blob)

    if not symbols and not markers:
        print(f"clean: no espeak-ng code in {args.binary} ({len(blob):,} bytes)")
        return 0

    print(f"FAIL: {args.binary} contains GPL-3.0 espeak-ng code.")
    if symbols:
        print(f"  {len(symbols)} espeak symbols, e.g. {', '.join(symbols[:6])}")
    if markers:
        for m in markers:
            print(f"  marker: {m}")
    print(
        "\neSpeak NG is GPL-3.0. Shipping this in a closed installer would\n"
        "distribute GPL-3.0 object code without GPL terms or source.\n"
        "Most likely cause: the vendored sherpa-onnx-sys patch was lost, or a\n"
        "pin in fetch-sherpa-archive.py points at a TTS-on archive again.\n"
        "See BACKLOG H10."
    )
    return 1


if __name__ == "__main__":
    sys.exit(main())
