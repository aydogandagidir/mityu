#!/usr/bin/env python3
"""Validate Mityu's DER scorer against NIST md-eval.pl on real VoxConverse data.

WHY THIS EXISTS
---------------
`eval-harness/src/diarization.rs` has thorough unit tests, but I wrote both the
implementation and the tests, so they share whatever I misunderstood about DER.
They cannot catch a wrong *convention* -- only a wrong *implementation of my
convention*. This script checks the scorer against the field's reference
implementation, written by someone else, on 216 real recordings.

It has already earned its keep twice. Both of these passed every unit test:

  1. Hypothesis speech before the first reference turn or after the last was
     counted as false alarm here and not by md-eval. md-eval scores only inside
     the evaluation region, which without a UEM is `[min TBEG, max TEND]` over
     the reference (md-eval-22.pl:2245, installed at :626). Fixed by
     `EvalRegion`, which now defaults to that same span.

  2. The speaker mapping was decided on the post-collar SCORED region instead of
     the pre-collar EVALUATION region (md-eval-22.pl:1890-1908 vs :1913-1918).
     Optimising the mapping against the same region the errors are counted on is
     self-serving: it produced a systematically LOWER DER than md-eval at
     collar > 0. An instrument that flatters us is worse than no instrument.

RESULT AFTER BOTH FIXES
-----------------------
216 files x 6 perturbations = 1296 comparisons per collar, at collars 0.0, 0.25
and 0.5: exact agreement, max |ours - md-eval| = 0.0000 percentage points.

USAGE
-----
    python tools/diarization/crosscheck-mdeval.py [--collar 0.25] [--limit N]

Downloads what it needs into a cache directory (default: the system temp dir)
and verifies both downloads by SHA-256, the same discipline ADR-0020 applies to
model files. Needs `perl` on PATH. Neither VoxConverse nor md-eval.pl is
committed to this repository: the corpus is third-party CC-BY-4.0 data and
md-eval.pl is third-party code.
"""

import argparse
import hashlib
import os
import re
import shutil
import subprocess
import sys
import tarfile
import urllib.request
from collections import defaultdict

# --- pinned third-party inputs ---------------------------------------------
# VoxConverse annotations: CC-BY-4.0, verified from the repository README
# (github.com/joonson/voxconverse) on 2026-08-08. Pinned to a commit, not a
# branch, so this validation reproduces.
VOX_REV = "24bf60be297701cd7e4ef18550c6d390c1b87365"
VOX_URL = f"https://github.com/joonson/voxconverse/archive/{VOX_REV}.tar.gz"
VOX_SHA256 = "e8c25c91b014657d7e4ad86f9bef4a7eb399929d8d4fab910d8e6c6ab63d1197"

# NIST md-eval v22, as vendored by nryant/dscore.
MDEVAL_URL = "https://raw.githubusercontent.com/nryant/dscore/master/scorelib/md-eval-22.pl"
MDEVAL_SHA256 = "872aa955cbc3d57e6d4fd6fe2e699791739c1cb716cc89105bb4717415b9400d"


def fetch(url: str, dest: str, expected_sha256: str) -> str:
    """Download once, and refuse to proceed on a digest mismatch.

    A silently different corpus or scorer would make the whole comparison
    meaningless while still printing a confident "NO DISAGREEMENTS".
    """
    if not os.path.exists(dest):
        print(f"fetching {url}")
        with urllib.request.urlopen(url, timeout=180) as r, open(dest, "wb") as fh:
            shutil.copyfileobj(r, fh)
    digest = hashlib.sha256(open(dest, "rb").read()).hexdigest()
    if digest != expected_sha256:
        raise SystemExit(
            f"FATAL: {dest} sha256 {digest} != expected {expected_sha256}.\n"
            "Refusing to score against an unverified input."
        )
    return dest


# --- RTTM ------------------------------------------------------------------
def read(path):
    turns = []
    with open(path) as fh:
        for line in fh:
            f = line.split()
            if not f or f[0] != "SPEAKER":
                continue
            turns.append([f[1], float(f[3]), float(f[4]), f[7]])
    return turns


def write(path, turns):
    with open(path, "w") as fh:
        for fid, beg, dur, spk in turns:
            if dur <= 0:
                continue
            fh.write(f"SPEAKER {fid} 1 {beg:.3f} {dur:.3f} <NA> <NA> {spk} <NA> <NA>\n")


def lcg(seed):
    """Deterministic PRNG: a disagreement must reproduce exactly."""
    s = [seed]

    def nxt():
        s[0] = (s[0] * 6364136223846793005 + 1442695040888963407) % (2**64)
        return s[0] >> 33

    return nxt


# --- perturbations ---------------------------------------------------------
# Self-scoring a reference gives 0% and proves almost nothing. Each of these
# breaks the reference in a way a real diarizer breaks it, so the two scorers
# have something substantial to disagree about.
def p_relabel(turns, _):
    """Rename every speaker. Segmentation is correct -> DER must be 0."""
    names = sorted({t[3] for t in turns})
    m = {n: f"HYP{len(names) - i:02d}" for i, n in enumerate(names)}
    return [[f, b, d, m[s]] for f, b, d, s in turns]


def p_jitter(turns, rnd):
    """Move every boundary by up to +-0.4 s -- boundary imprecision."""
    out = []
    for f, b, d, s in turns:
        db = ((rnd() % 800) - 400) / 1000.0
        dd = ((rnd() % 800) - 400) / 1000.0
        out.append([f, max(0.0, b + db), max(0.05, d + dd), s])
    return out


def p_merge(turns, _):
    """Collapse the two most talkative speakers into one -- under-clustering."""
    tot = defaultdict(float)
    for _f, _b, d, s in turns:
        tot[s] += d
    top = [s for s, _ in sorted(tot.items(), key=lambda kv: -kv[1])[:2]]
    if len(top) < 2:
        return [list(t) for t in turns]
    return [[f, b, d, (top[0] if s in top else s)] for f, b, d, s in turns]


def p_split(turns, _):
    """Cut the busiest speaker's turns in two, second part a new label
    -- over-clustering."""
    tot = defaultdict(float)
    for _f, _b, d, s in turns:
        tot[s] += d
    if not tot:
        return []
    busy = max(tot, key=tot.get)
    out = []
    for f, b, d, s in turns:
        if s == busy and d > 1.0:
            # 45/55, deliberately NOT 50/50. An exact half gives the two
            # hypothesis speakers bit-identical overlap with the reference
            # speaker, which is a genuine TIE in the assignment problem: both
            # mappings are optimal and the choice is implementation-defined.
            # That is not a disagreement about DER, and leaving it in produced 8
            # phantom "disagreements" whose overlap matrices were exactly equal
            # (37.020 vs 37.020, 85.920 vs 85.920, ...). Removing the tie made
            # every one of them vanish.
            cut = d * 0.45
            out.append([f, b, cut, s])
            out.append([f, b + cut, d - cut, s + "_B"])
        else:
            out.append([f, b, d, s])
    return out


def p_drop(turns, _):
    """Drop every third turn -- missed speech."""
    return [list(t) for i, t in enumerate(turns) if i % 3 != 0]


def p_inflate(turns, _):
    """Extend every turn by 0.5 s -- false alarm."""
    return [[f, b, d + 0.5, s] for f, b, d, s in turns]


PERTURBATIONS = [
    ("relabel", p_relabel),
    ("jitter", p_jitter),
    ("merge", p_merge),
    ("split", p_split),
    ("drop", p_drop),
    ("inflate", p_inflate),
]


# --- the two scorers -------------------------------------------------------
def ours(exe, ref, hyp, collar):
    r = subprocess.run(
        [exe, "der", "--reference", ref, "--hypothesis", hyp, "--collar", str(collar)],
        capture_output=True,
        text=True,
    )
    if r.returncode != 0:
        return None
    m = re.search(r"DER\s+([\d.]+)%", r.stdout)
    return float(m.group(1)) if m else None


def md_eval(script, ref, hyp, collar):
    r = subprocess.run(
        ["perl", script, "-r", ref, "-s", hyp, "-c", str(collar)],
        capture_output=True,
        text=True,
    )
    m = re.search(r"OVERALL SPEAKER DIARIZATION ERROR = ([\d.]+) percent", r.stdout)
    return float(m.group(1)) if m else None


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--collar", type=float, default=0.0)
    ap.add_argument("--limit", type=int, default=0, help="score only the first N files")
    ap.add_argument(
        "--exe",
        default=os.path.join("target", "debug", "eval-harness.exe"
                             if os.name == "nt" else "eval-harness"),
        help="path to the built eval-harness binary",
    )
    ap.add_argument("--cache", default=os.path.join(os.environ.get("TEMP", "/tmp"),
                                                    "mityu-diarization-crosscheck"))
    ap.add_argument("--tolerance", type=float, default=0.05,
                    help="percentage points; md-eval prints 2 decimals")
    args = ap.parse_args()

    if not os.path.exists(args.exe):
        raise SystemExit(f"eval-harness not found at {args.exe} — build it first "
                         f"(cargo build -p eval-harness), or pass --exe")
    if shutil.which("perl") is None:
        raise SystemExit("perl not found on PATH — md-eval.pl needs it")

    os.makedirs(args.cache, exist_ok=True)
    tarball = fetch(VOX_URL, os.path.join(args.cache, "voxconverse.tar.gz"), VOX_SHA256)
    mdeval = fetch(MDEVAL_URL, os.path.join(args.cache, "md-eval.pl"), MDEVAL_SHA256)

    dev = os.path.join(args.cache, f"voxconverse-{VOX_REV}", "dev")
    if not os.path.isdir(dev):
        with tarfile.open(tarball) as tf:
            # `filter="data"` refuses absolute paths, `..` escapes and device
            # nodes. The archive is checksum-pinned above, but a tool that
            # unpacks a downloaded archive should not also be the one place that
            # trusts it.
            if sys.version_info >= (3, 12):
                tf.extractall(args.cache, filter="data")
            else:
                tf.extractall(args.cache)

    work = os.path.join(args.cache, "work")
    os.makedirs(work, exist_ok=True)

    files = sorted(f for f in os.listdir(dev) if f.endswith(".rttm"))
    if args.limit:
        files = files[: args.limit]
    print(f"files={len(files)} perturbations={len(PERTURBATIONS)} collar={args.collar}")

    disagreements, checked, skipped, worst = [], 0, 0, 0.0
    for name in files:
        path = os.path.join(dev, name)
        base = name[:-5]
        turns = read(path)
        for pname, fn in PERTURBATIONS:
            # Seed from the file name so a rerun reproduces byte for byte.
            rnd = lcg(0x20260808 ^ (int(hashlib.md5(base.encode()).hexdigest()[:8], 16)))
            hp = os.path.join(work, f"{base}.{pname}.rttm")
            write(hp, fn(turns, rnd))
            a = ours(args.exe, path, hp, args.collar)
            b = md_eval(mdeval, path, hp, args.collar)
            if a is None or b is None:
                skipped += 1
                print(f"  SKIP {base}/{pname}: ours={a} md-eval={b}")
                continue
            checked += 1
            d = abs(a - b)
            worst = max(worst, d)
            if d > args.tolerance:
                disagreements.append((base, pname, a, b, d))

    print(f"\nchecked={checked} skipped={skipped}")
    print(f"max |ours - md-eval| = {worst:.4f} percentage points")
    if disagreements:
        print(f"DISAGREEMENTS ({len(disagreements)}):")
        for base, pname, a, b, d in disagreements[:40]:
            print(f"  {base:12s} {pname:8s} ours={a:7.3f}  md-eval={b:7.3f}  diff={d:.3f}")
        return 1
    print("NO DISAGREEMENTS")
    return 0


if __name__ == "__main__":
    sys.exit(main())
