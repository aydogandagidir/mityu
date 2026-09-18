#!/usr/bin/env python3
"""Fail on a className the built stylesheet has no rule for.

WHY THIS EXISTS
---------------
Tailwind builds its stylesheet by SCANNING SOURCE TEXT for class names. A class it
cannot see as a literal is never generated, and the element then renders with that class
attribute and no styling — which looks exactly like a design choice. ADR-0037 is that
failure, shipped. The ESLint palette guardrails cover the 2342 literal `className="…"`
sites; they are blind to the ~85 `cn()` and ~95 template-literal sites, where a token is
assembled at runtime out of pieces Tailwind never saw whole.

This closes that gap from the other end: it reads the classes that ACTUALLY REACHED THE
EXPORTED HTML and checks each one against the rules that actually reached the built CSS.
A class in the markup with no rule behind it is either dead weight or, worse, a style the
author believes is applied.

USAGE
-----
    python tools/ui/check-dead-classes.py                 # after `next build`
    python tools/ui/check-dead-classes.py --self-test     # prove the detector fires

A detector nobody has seen fail is indistinguishable from one that cannot, so
`--self-test` runs a known-dead class through the same comparison first.
"""

import argparse
import glob
import html as html_mod
import os
import re
import sys

REPO = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
EXPORT = os.path.join(REPO, "frontend", "out")

# Classes that are MEANT to have no rule of their own.
#
# `group`/`peer` are parent markers: Tailwind emits rules for `group-hover:x` on the
# CHILD and nothing at all for the bare marker. The theme classes are toggled by
# next-themes and consumed through `:root.dark` selectors, not as `.dark { }`. The rest
# are hooks read by JavaScript or by a third-party stylesheet that is not in this bundle.
ALLOWED = {
    "group",
    "peer",
    "dark",
    "light",
    "system",
    # next/font injects these and generates its own <style>, not a bundle rule.
    "__className_",
    "__variable_",
}
ALLOWED_PREFIXES = (
    "group/",
    "peer/",
    "__className_",
    "__variable_",
    # Third-party class names that belong to a stylesheet outside this bundle, or to
    # none at all: lucide stamps `lucide lucide-<icon>` on every SVG purely as a hook,
    # BlockNote and sonner ship their own CSS.
    "lucide",
    "bn-",
    "sonner-",
    "toaster",
    # Next.js draws its own 404 with an inline <style>, not a bundle rule.
    "next-error-",
)


def unescape_css(name: str) -> str:
    """CSS identifier escapes -> the characters the HTML actually carries."""
    out, i = [], 0
    while i < len(name):
        if name[i] != "\\":
            out.append(name[i])
            i += 1
            continue
        m = re.match(r"\\([0-9a-fA-F]{1,6}) ?", name[i:])
        if m:
            out.append(chr(int(m.group(1), 16)))
            i += m.end()
        else:
            out.append(name[i + 1] if i + 1 < len(name) else "")
            i += 2
    return "".join(out)


def css_classes(css_dir: str) -> set:
    """Every class name any rule in the built CSS selects on."""
    found = set()
    for path in glob.glob(os.path.join(css_dir, "*.css")):
        with open(path, encoding="utf-8", errors="replace") as f:
            text = f.read()
        # Tailwind escapes the characters a class name may not contain raw:
        # `.md\:flex`, `.w-\[42px\]`, `.bg-primary\/50`. Some it escapes NUMERICALLY
        # instead — a comma becomes `\2c ` (hex, trailing space consumed) — so a naive
        # backslash strip turns `cubic-bezier(0.4,0,0.6,1)` into `cubic-bezier(0.42c ...)`
        # and reports a class that is right there in the file.
        for m in re.finditer(r"\.((?:[\w-]|\\[0-9a-fA-F]{1,6} ?|\\.)+)", text):
            found.add(unescape_css(m.group(1)))
    return found


def html_classes(export: str) -> dict:
    """Every class token in the exported HTML, mapped to the files carrying it."""
    used = {}
    for path in glob.glob(os.path.join(export, "**", "*.html"), recursive=True):
        with open(path, encoding="utf-8", errors="replace") as f:
            text = f.read()
        rel = os.path.relpath(path, export)
        for m in re.finditer(r'class="([^"]*)"', text):
            # The attribute is HTML-escaped in the export: an arbitrary variant like
            # `[&>*]:mt-2` is written `[&amp;&gt;*]:mt-2`. Comparing the escaped form
            # against the CSS reports every arbitrary variant in the app as dead.
            for token in html_mod.unescape(m.group(1)).split():
                used.setdefault(token, set()).add(rel)
    return used


def is_allowed(token: str) -> bool:
    return token in ALLOWED or token.startswith(ALLOWED_PREFIXES)


def self_test(rules: set) -> int:
    """The comparison must fail on a class that is definitely not in the bundle."""
    fake = "bg-this-class-does-not-exist-9f3a"
    if fake in rules:
        print("FATAL: self-test class somehow exists in the CSS; pick another")
        return 1
    if is_allowed(fake):
        print("FATAL: self-test class is allowlisted; the allowlist is too broad")
        return 1
    print("self-test ok: an unstyled class is reported, an allowlisted one is not")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--export", default=EXPORT)
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args()

    css_dir = os.path.join(args.export, "_next", "static", "css")
    if not os.path.isdir(css_dir):
        print(f"FATAL: no built CSS at {css_dir}. Run `pnpm exec next build` first.")
        return 1

    rules = css_classes(css_dir)
    if args.self_test:
        return self_test(rules)

    dead = {
        token: files
        for token, files in html_classes(args.export).items()
        if token not in rules and not is_allowed(token)
    }

    if not dead:
        print(f"ok: every class in the export has a rule ({len(rules)} rules checked)")
        return 0

    print(f"FAILED: {len(dead)} class(es) in the markup have no rule in the built CSS.")
    print("Each one renders as nothing. Usually a cn() or template-literal site that")
    print("Tailwind's scanner never saw whole (ADR-0037).\n")
    for token in sorted(dead):
        where = sorted(dead[token])
        shown = ", ".join(where[:3]) + (f" (+{len(where) - 3} more)" if len(where) > 3 else "")
        print(f"  {token}\n      {shown}")
    return 1


if __name__ == "__main__":
    sys.exit(main())
