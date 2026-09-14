#!/usr/bin/env python3
"""Fail CI if any GitHub Action this repo uses is unpinned, on a dead Node runtime,
or mislabelled.

WHY THIS EXISTS
---------------
The v1.2.0 Release run (2026-09-14) printed GitHub's "Node.js 20 is deprecated"
annotation for five actions. Two things made that worth a check rather than a
one-off repin:

  1. It had already been "fixed" once. PR #40 repinned six workflow files and
     deliberately deferred `build.yml` — the one file the release pipeline
     actually builds with. Nothing noticed, because a Node 20 action is only a
     warning today. The day runners drop the fallback it becomes a hard failure,
     and it will surface first in the release pipeline, at the worst moment.
  2. One of the five was not in any file of ours. `humbletim/install-vulkan-sdk`
     is a *composite* action that nests `uses: actions/cache@v4` — a floating
     tag on Node 20 inside an action we had pinned to a sha. Our file looked
     clean; the runtime was not. That is a supply-chain gap as much as a
     deprecation, and it is invisible to a grep over `.github/workflows`.

So this checks the property that actually regressed, not the symptom:

  - every remote `uses:` is pinned to a 40-hex commit sha (no floating tags);
  - every pinned JavaScript action declares `runs.using: node24`
    (composite and docker actions are fine; node12/16/20 fail);
  - composite actions are opened and their nested `uses:` held to the same
    rules — a nested floating tag fails, with the hint to vendor or bump;
  - the `# vX.Y.Z` comment after a pin names a tag that really points at that
    sha. A stale comment is how `build.yml`'s `upload-artifact` read as current
    when it was two majors behind.

An action that genuinely cannot move yet goes in `action-pins-allowlist.json`
next to this file, keyed `owner/repo@sha`, with a reason. Allowlisted entries
are printed as warnings on every run so they cannot be forgotten.

NETWORK
-------
Resolving tags and reading `action.yml` at a sha needs `git ls-remote` and a
shallow `git fetch` against github.com — ~1 s per distinct action. `--self-test`
uses a fake resolver and needs no network, so the detector's own behaviour is
proven on every run before it is trusted.

USAGE
-----
    python3 tools/ci/check-action-pins.py            # check the repo
    python3 tools/ci/check-action-pins.py --self-test
"""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
import tempfile
from dataclasses import dataclass, field
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
ALLOWLIST_PATH = Path(__file__).resolve().with_name("action-pins-allowlist.json")

SHA_RE = re.compile(r"^[0-9a-f]{40}$")
# `- uses: owner/repo@ref # comment` or `uses: ./local/path # comment`
USES_RE = re.compile(r"^\s*-?\s*uses:\s*['\"]?([^\s'\"#]+)['\"]?\s*(?:#\s*(.*?))?\s*$")
USING_RE = re.compile(r"^\s*using:\s*['\"]?([A-Za-z0-9]+)", re.MULTILINE)
VERSION_COMMENT_RE = re.compile(r"^v?\d+(\.\d+)*$")
OK_RUNTIMES = {"node24", "composite", "docker"}


@dataclass
class Use:
    file: Path
    line: int
    ref: str
    comment: str | None


@dataclass
class Report:
    failures: list[str] = field(default_factory=list)
    warnings: list[str] = field(default_factory=list)
    infos: list[str] = field(default_factory=list)

    def fail(self, msg: str) -> None:
        self.failures.append(msg)

    def warn(self, msg: str) -> None:
        self.warnings.append(msg)

    def info(self, msg: str) -> None:
        self.infos.append(msg)


# --- parsing -----------------------------------------------------------------


def find_workflow_files(root: Path) -> list[Path]:
    files: list[Path] = []
    for pattern in ("*.yml", "*.yaml"):
        files += sorted((root / ".github" / "workflows").glob(pattern))
    actions_dir = root / ".github" / "actions"
    if actions_dir.is_dir():
        for name in ("action.yml", "action.yaml"):
            files += sorted(actions_dir.rglob(name))
    return files


def parse_uses(text: str, file: Path) -> list[Use]:
    uses: list[Use] = []
    for n, raw in enumerate(text.splitlines(), start=1):
        m = USES_RE.match(raw)
        if not m:
            continue
        uses.append(Use(file=file, line=n, ref=m.group(1), comment=m.group(2) or None))
    return uses


def is_reusable_workflow(ref: str) -> bool:
    """`uses: ./.github/workflows/x.yml` or `owner/repo/.github/workflows/x.yml@sha`
    is a job-level reusable-workflow call. It has no action.yml and no runtime of
    its own; its steps are checked when the walk reaches that file."""
    path = ref.rpartition("@")[0] if "@" in ref else ref
    return path.endswith((".yml", ".yaml"))


def split_remote(ref: str) -> tuple[str, str | None, str] | None:
    """`owner/repo[/subpath]@sha` -> (owner/repo, subpath, sha); None for local/docker."""
    if ref.startswith("./") or ref.startswith("docker://"):
        return None
    if "@" not in ref:
        return None
    path, _, version = ref.rpartition("@")
    parts = path.split("/")
    if len(parts) < 2:
        return None
    repo = "/".join(parts[:2])
    subpath = "/".join(parts[2:]) or None
    return repo, subpath, version


# --- resolvers ---------------------------------------------------------------


class GitResolver:
    """Talks to github.com. Cached per repo so shared actions cost one round trip."""

    def __init__(self, timeout: int = 90) -> None:
        self.timeout = timeout
        self._tags: dict[str, dict[str, list[str]]] = {}
        self._env = {**os.environ, "GIT_TERMINAL_PROMPT": "0"}

    def _run(self, args: list[str], cwd: str | None = None) -> str:
        out = subprocess.run(
            args, cwd=cwd, env=self._env, capture_output=True, text=True, timeout=self.timeout
        )
        if out.returncode != 0:
            raise RuntimeError(out.stderr.strip() or f"exit {out.returncode}")
        return out.stdout

    def tags_at(self, repo: str, sha: str) -> list[str]:
        if repo not in self._tags:
            table: dict[str, list[str]] = {}
            listing = self._run(["git", "ls-remote", "--tags", f"https://github.com/{repo}.git"])
            for line in listing.splitlines():
                obj, _, name = line.partition("\t")
                if not name.startswith("refs/tags/"):
                    continue
                tag = name[len("refs/tags/") :]
                # `^{}` is the peeled commit behind an annotated tag — that is the
                # sha a `uses:` must pin, so record it under the plain tag name.
                if tag.endswith("^{}"):
                    tag = tag[:-3]
                table.setdefault(obj, []).append(tag)
            self._tags[repo] = table
        return self._tags[repo].get(sha, [])

    def action_manifest(self, repo: str, sha: str, subpath: str | None) -> str:
        with tempfile.TemporaryDirectory() as tmp:
            self._run(["git", "init", "-q"], cwd=tmp)
            self._run(
                ["git", "fetch", "-q", "--depth", "1", f"https://github.com/{repo}.git", sha],
                cwd=tmp,
            )
            prefix = f"{subpath}/" if subpath else ""
            for name in ("action.yml", "action.yaml"):
                try:
                    return self._run(["git", "show", f"FETCH_HEAD:{prefix}{name}"], cwd=tmp)
                except RuntimeError:
                    continue
        raise RuntimeError(f"no action.yml at {repo}@{sha[:12]}{'/' + subpath if subpath else ''}")


class FakeResolver:
    """Deterministic stand-in for --self-test. No network."""

    def __init__(self, tags: dict[tuple[str, str], list[str]], manifests: dict[tuple[str, str], str]):
        self._tags = tags
        self._manifests = manifests

    def tags_at(self, repo: str, sha: str) -> list[str]:
        return self._tags.get((repo, sha), [])

    def action_manifest(self, repo: str, sha: str, subpath: str | None) -> str:
        try:
            return self._manifests[(repo, sha)]
        except KeyError as e:
            raise RuntimeError(f"no manifest for {repo}@{sha}") from e


# --- the check ---------------------------------------------------------------


def load_allowlist(path: Path) -> dict[str, str]:
    if not path.exists():
        return {}
    data = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(data, dict) or not all(
        isinstance(k, str) and isinstance(v, str) and v.strip() for k, v in data.items()
    ):
        raise SystemExit(f"{path}: must be an object of 'owner/repo@sha': 'reason' with non-empty reasons")
    return data


def check_remote(
    use: Use,
    resolver,
    allowlist: dict[str, str],
    report: Report,
    *,
    root: Path = REPO_ROOT,
    nested_in: str | None = None,
    depth: int = 0,
) -> None:
    """Check one remote `uses:`; recurse into composite actions' nested uses."""
    where = f"{os.path.relpath(use.file, root) if use.file.is_absolute() else use.file}:{use.line}"
    label = f"{where}  {use.ref}" + (f"  (nested in {nested_in})" if nested_in else "")

    split = split_remote(use.ref)
    if split is None:
        if use.ref.startswith("docker://"):
            report.info(f"{label}: docker image reference, not a JS action — skipped")
        return
    repo, subpath, version = split

    if not SHA_RE.match(version):
        hint = (
            " — this is inside a third-party action we do not control: vendor it "
            "(as .github/actions/install-vulkan-sdk was) or bump to a release that pins it"
            if nested_in
            else " — pin to a 40-hex commit sha, with the tag in a trailing `# vX.Y.Z` comment"
        )
        report.fail(f"{label}: floating ref '{version}' is not a commit sha{hint}")
        return
    sha = version

    # 1. The `# vX.Y.Z` comment must name a tag that really points here.
    try:
        tags = resolver.tags_at(repo, sha)
    except Exception as e:  # network / unknown repo
        report.fail(f"{label}: could not list tags ({e})")
        return
    if use.comment and VERSION_COMMENT_RE.match(use.comment.strip()):
        claimed = use.comment.strip()
        if tags and claimed not in tags:
            report.fail(
                f"{label}: comment says `# {claimed}` but {sha[:12]} is tagged {tags} — "
                "stale label; fix the comment or the sha"
            )
        elif not tags:
            report.info(f"{label}: no tag points at {sha[:12]}; `# {claimed}` cannot be verified")
    elif not tags:
        report.info(f"{label}: pinned to an untagged commit (branch head) — fine, but unverifiable")

    # 2. The runtime.
    try:
        manifest = resolver.action_manifest(repo, sha, subpath)
    except Exception as e:
        report.fail(f"{label}: could not read action.yml at that sha ({e})")
        return
    m = USING_RE.search(manifest)
    using = m.group(1).lower() if m else None
    if using is None:
        report.fail(f"{label}: action.yml has no `runs.using` — cannot tell what runtime it needs")
        return

    key = f"{repo}@{sha}"
    if using in OK_RUNTIMES:
        pass
    elif key in allowlist:
        report.warn(f"{label}: runs on {using} — ALLOWLISTED: {allowlist[key]}")
    else:
        report.fail(
            f"{label}: runs on {using}, which GitHub is retiring — move to a node24 release, "
            f"or add '{key}' to {ALLOWLIST_PATH.name} with a reason"
        )

    # 3. Composite actions hide more `uses:`. Open them.
    if using == "composite":
        if depth >= 3:
            report.warn(f"{label}: composite nesting deeper than 3 — stopped recursing")
            return
        for nested in parse_uses(manifest, use.file):
            if split_remote(nested.ref) is None:
                continue
            nested_use = Use(file=use.file, line=use.line, ref=nested.ref, comment=nested.comment)
            check_remote(
                nested_use,
                resolver,
                allowlist,
                report,
                root=root,
                nested_in=f"{repo}@{sha[:12]}",
                depth=depth + 1,
            )


def check_repo(root: Path, resolver, allowlist: dict[str, str]) -> Report:
    report = Report()
    files = find_workflow_files(root)
    if not files:
        report.fail(f"no workflow files found under {root}/.github")
        return report
    for file in files:
        text = file.read_text(encoding="utf-8")
        for use in parse_uses(text, file):
            where = f"{os.path.relpath(file, root)}:{use.line}  {use.ref}"
            if use.ref.startswith("./"):
                target = root / use.ref[2:]
                if is_reusable_workflow(use.ref):
                    # Reusable workflow: only existence matters here; it is one of
                    # the files being walked, so its own steps get checked.
                    if not target.exists():
                        report.fail(f"{where}: reusable workflow file does not exist")
                    continue
                if not (target / "action.yml").exists() and not (target / "action.yaml").exists():
                    report.fail(f"{where}: local action has no action.yml")
                # Its own nested uses are checked when the walk reaches its action.yml.
                continue
            if is_reusable_workflow(use.ref):
                # Remote reusable workflow: must be sha-pinned, but has no runtime.
                split = split_remote(use.ref)
                if split is None or not SHA_RE.match(split[2]):
                    report.fail(f"{where}: remote reusable workflow is not pinned to a commit sha")
                continue
            check_remote(use, resolver, allowlist, report, root=root)
    return report


# --- self-test ---------------------------------------------------------------


def self_test() -> int:
    """Prove each rule both fires and stays quiet. A check nobody has seen fail
    is indistinguishable from a check that cannot fail."""
    SHA_OK = "a" * 40
    SHA_OLD = "b" * 40
    SHA_COMP = "c" * 40
    SHA_UNTAGGED = "d" * 40
    fake = FakeResolver(
        tags={
            ("acme/good", SHA_OK): ["v7", "v7.0.1"],
            ("acme/old", SHA_OLD): ["v4", "v4.6.2"],
            ("acme/comp", SHA_COMP): ["v1.2"],
        },
        manifests={
            ("acme/good", SHA_OK): "name: x\nruns:\n  using: 'node24'\n  main: dist/index.js\n",
            ("acme/old", SHA_OLD): "name: x\nruns:\n  using: node20\n  main: dist/index.js\n",
            ("acme/comp", SHA_COMP): (
                "name: x\nruns:\n  using: composite\n  steps:\n"
                "    - uses: actions/cache@v4\n      with: {path: x, key: y}\n"
            ),
            ("acme/untagged", SHA_UNTAGGED): "runs:\n  using: node24\n",
        },
    )

    def run(ref: str, comment: str | None, allow: dict[str, str] | None = None) -> Report:
        r = Report()
        check_remote(Use(Path("wf.yml"), 1, ref, comment), fake, allow or {}, r)
        return r

    cases = [
        ("clean node24 with matching comment passes", run(f"acme/good@{SHA_OK}", "v7.0.1"), False, None),
        ("floating tag fails", run("acme/good@v7", "v7"), True, "floating ref"),
        ("node20 fails", run(f"acme/old@{SHA_OLD}", "v4.6.2"), True, "runs on node20"),
        (
            "node20 allowlisted passes with a warning",
            run(f"acme/old@{SHA_OLD}", "v4.6.2", {f"acme/old@{SHA_OLD}": "dormant, see ADR"}),
            False,
            None,
        ),
        ("stale comment fails", run(f"acme/good@{SHA_OK}", "v4.6.2"), True, "stale label"),
        (
            "nested floating tag inside a composite fails",
            run(f"acme/comp@{SHA_COMP}", "v1.2"),
            True,
            "nested in acme/comp",
        ),
        ("untagged commit is info, not failure", run(f"acme/untagged@{SHA_UNTAGGED}", None), False, None),
    ]
    # Bug class 1: a job-level reusable-workflow call is not an action and must
    # not be reported as "local action has no action.yml". Bug class 2: scanning
    # a checkout that is NOT the script's own repo (`--root`) must work — that is
    # how the pre-fix state on main was reproduced.
    import tempfile as _tf

    with _tf.TemporaryDirectory() as tmp:
        root = Path(tmp)
        (root / ".github" / "workflows").mkdir(parents=True)
        (root / ".github" / "workflows" / "build.yml").write_text(
            "jobs:\n  b:\n    steps:\n      - uses: acme/good@" + SHA_OK + " # v7.0.1\n",
            encoding="utf-8",
        )
        (root / ".github" / "workflows" / "release.yml").write_text(
            "jobs:\n  gates:\n    uses: ./.github/workflows/build.yml\n"
            "  missing:\n    uses: ./.github/workflows/nope.yml\n",
            encoding="utf-8",
        )
        repo_report = check_repo(root, fake, {})
    cases.append(
        (
            "reusable-workflow calls are not treated as actions; a missing one fails; --root works",
            repo_report,
            True,
            "nope.yml: reusable workflow file does not exist",
        )
    )
    if any("build.yml: local action has no action.yml" in f for f in repo_report.failures):
        print("  [FAIL] existing reusable workflow was wrongly reported as an action")
        return 1

    bad = 0
    for name, report, expect_fail, needle in cases:
        failed = bool(report.failures)
        ok = failed == expect_fail and (needle is None or any(needle in f for f in report.failures))
        print(f"  [{'ok' if ok else 'FAIL'}] {name}")
        if not ok:
            bad += 1
            for f in report.failures:
                print(f"         failure: {f}")
    allow_case = cases[3][1]
    if not allow_case.warnings:
        print("  [FAIL] allowlisted entry must still print a warning")
        bad += 1
    if bad:
        print(f"self-test: {bad} case(s) wrong", file=sys.stderr)
        return 1
    print("self-test: all rules fire when they should and stay quiet when they should")
    return 0


# --- main --------------------------------------------------------------------


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--self-test", action="store_true", help="prove the detector on fixtures; no network")
    ap.add_argument("--root", type=Path, default=REPO_ROOT, help="repo root (default: this checkout)")
    args = ap.parse_args(argv)

    if args.self_test:
        return self_test()

    allowlist = load_allowlist(ALLOWLIST_PATH)
    report = check_repo(args.root.resolve(), GitResolver(), allowlist)

    for msg in report.infos:
        print(f"info: {msg}")
    for msg in report.warnings:
        print(f"::warning::{msg}")
    for msg in report.failures:
        print(f"::error::{msg}")
    print(
        f"\n{len(report.failures)} failure(s), {len(report.warnings)} allowlisted, "
        f"{len(report.infos)} note(s)"
    )
    return 1 if report.failures else 0


if __name__ == "__main__":
    sys.exit(main())
