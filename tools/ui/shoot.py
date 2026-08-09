#!/usr/bin/env python3
"""Screenshot the built UI, and refuse to call a blank page a screenshot.

WHY THIS EXISTS
---------------
The owner's standing rule is that a UI change is not done until it has been
seen. `tsc`, lint and a mockup all pass on a screen that renders nothing.

The obvious approach fails in two ways that both LOOK like success, which is why
this is a script and not a remembered command line:

1. **`file://` gives an unstyled shell.** The Next export references assets by
   absolute path (`/_next/static/css/...`), and under `file://` the leading `/`
   resolves to the filesystem root, so no CSS or JS loads. You get a valid PNG
   of an unstyled page. Measured: 11 KB via `file://` versus 113 KB over HTTP
   for the same route. So the export is served over HTTP here.

2. **Tauri-dependent pages cannot render outside the app.** `/meeting-details`
   fetches through `invoke()`, which throws in a browser, so it renders a
   loading state forever. Screenshot a `/design/*` route with fixture data
   instead — that is what those routes are for.

So this fails on a screenshot that is suspiciously small or a DOM missing the
markers you asked for, rather than handing back a picture of nothing.

USAGE
-----
    python tools/ui/shoot.py design/report design/hitl
    python tools/ui/shoot.py --build design/report --expect "Speaker" --expect "talk time"
    python tools/ui/shoot.py --out-dir shots design/report

Routes are given without the `.html` suffix, relative to the export root.

LIGHT MODE: `--force-prefers-color-scheme=light` does NOT flip this app. The
theme is a class on `<html>`, not a media query, so the flag changes nothing and
you get a dark screenshot that looks like the flag was ignored -- it was. To
check light mode, set the class first and read the computed colours:

    html.classList.remove('dark'); html.classList.add('light')
    getComputedStyle(el).color
"""

import argparse
import http.server
import os
import shutil
import socket
import socketserver
import subprocess
import sys
import threading

REPO = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
FRONTEND = os.path.join(REPO, "frontend")
EXPORT = os.path.join(FRONTEND, "out")

# Below this, a PNG is almost certainly an unstyled shell or an error page. The
# unstyled `file://` render of a real route measured 11 KB; a correctly rendered
# one measured 72-113 KB.
MIN_PNG_BYTES = 25_000

CHROME_CANDIDATES = [
    r"C:\Program Files\Google\Chrome\Application\chrome.exe",
    r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
    os.path.expanduser(r"~\AppData\Local\Google\Chrome\Application\chrome.exe"),
    "/usr/bin/google-chrome",
    "/usr/bin/chromium",
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
]


def find_chrome() -> str:
    env = os.environ.get("MITYU_CHROME")
    if env and os.path.isfile(env):
        return env
    for c in CHROME_CANDIDATES:
        if os.path.isfile(c):
            return c
    which = shutil.which("chrome") or shutil.which("chromium")
    if which:
        return which
    raise SystemExit(
        "FATAL: no Chrome found. Set MITYU_CHROME to the executable."
    )


def build() -> None:
    print("building the static export (this is the only step that needs the network:")
    print("  next/font downloads at BUILD time and self-hosts into the export)")
    r = subprocess.run(
        ["pnpm", "exec", "next", "build"],
        cwd=FRONTEND,
        shell=os.name == "nt",
        capture_output=True,
        text=True,
        errors="replace",
    )
    if r.returncode == 0:
        return
    out = (r.stdout or "") + (r.stderr or "")
    print(out[-4000:])
    # Worth naming, because the message Next prints for it is just a stack trace
    # and the real cause is almost always a server left running on the export.
    if "EBUSY" in out or "EPERM" in out:
        raise SystemExit(
            f"FATAL: something is holding {EXPORT} open, so the build cannot replace it.\n"
            "Usually a leftover static server or a shell sitting in that directory.\n"
            "On Windows: Get-CimInstance Win32_Process | Where CommandLine -like '*http.server*'"
        )
    raise SystemExit("FATAL: next build failed; nothing to screenshot")


class Quiet(http.server.SimpleHTTPRequestHandler):
    def log_message(self, *_args):
        pass


def serve(directory: str):
    """Serve the export on an ephemeral port, in a background thread."""
    handler = lambda *a, **k: Quiet(*a, directory=directory, **k)  # noqa: E731
    with socket.socket() as probe:
        probe.bind(("127.0.0.1", 0))
        port = probe.getsockname()[1]
    httpd = socketserver.TCPServer(("127.0.0.1", port), handler)
    threading.Thread(target=httpd.serve_forever, daemon=True).start()
    return httpd, port


def shoot(chrome: str, url: str, png: str, width: int, height: int) -> int:
    # Delete first. Otherwise a Chrome that fails or crashes leaves the PREVIOUS
    # run's PNG in place, and every check below then validates a stale file --
    # the tool would report "all routes rendered" having rendered nothing, which
    # is the exact failure it exists to prevent.
    if os.path.exists(png):
        os.remove(png)
    r = subprocess.run(
        [
            chrome,
            "--headless",
            "--disable-gpu",
            "--hide-scrollbars",
            f"--window-size={width},{height}",
            # Let hydration and client-side effects settle; without this the
            # shot is of the pre-hydration markup.
            "--virtual-time-budget=6000",
            f"--screenshot={png}",
            url,
        ],
        capture_output=True,
    )
    return r.returncode


def dump_dom(chrome: str, url: str) -> str:
    r = subprocess.run(
        [
            chrome,
            "--headless",
            "--disable-gpu",
            "--virtual-time-budget=6000",
            "--dump-dom",
            url,
        ],
        capture_output=True,
        text=True,
        errors="replace",
    )
    return r.stdout


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("routes", nargs="+", help="routes without .html, e.g. design/report")
    ap.add_argument("--build", action="store_true", help="run `next build` first")
    ap.add_argument("--out-dir", default=os.path.join(REPO, "target", "ui-shots"))
    ap.add_argument("--width", type=int, default=1280)
    ap.add_argument("--height", type=int, default=900)
    ap.add_argument(
        "--expect",
        action="append",
        default=[],
        help="text that must appear in the rendered DOM; repeatable. Without it, "
        "the only check is that the page is not blank.",
    )
    ap.add_argument("--min-bytes", type=int, default=MIN_PNG_BYTES)
    args = ap.parse_args()

    if args.build:
        build()
    if not os.path.isdir(EXPORT):
        raise SystemExit(
            f"FATAL: no export at {EXPORT}. Run with --build, or `pnpm exec next build`."
        )

    chrome = find_chrome()
    os.makedirs(args.out_dir, exist_ok=True)
    httpd, port = serve(EXPORT)
    failures = []
    try:
        for route in args.routes:
            url = f"http://127.0.0.1:{port}/{route.lstrip('/')}.html"
            png = os.path.join(args.out_dir, route.replace("/", "-") + ".png")
            code = shoot(chrome, url, png, args.width, args.height)

            if code != 0:
                failures.append(f"{route}: chrome exited {code} and rendered nothing")
                continue
            if not os.path.exists(png):
                failures.append(f"{route}: chrome produced no file")
                continue
            size = os.path.getsize(png)
            dom = dump_dom(chrome, url)

            # An error page or an unstyled shell is small AND short on markup.
            if size < args.min_bytes:
                failures.append(
                    f"{route}: {size} bytes is below {args.min_bytes} -- almost certainly "
                    "an unstyled or empty render, not a screenshot of the feature"
                )
            missing = [m for m in args.expect if m.lower() not in dom.lower()]
            if missing:
                failures.append(f"{route}: rendered DOM is missing {missing}")
            print(f"{route}  {size} bytes  ->  {png}")
    finally:
        httpd.shutdown()

    if failures:
        print("\nFAILED:")
        for f in failures:
            print(f"  {f}")
        return 1
    print("\nall routes rendered")
    return 0


if __name__ == "__main__":
    sys.exit(main())
