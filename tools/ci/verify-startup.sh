#!/usr/bin/env bash
# Launch the built desktop app and prove, at RUNTIME, that it starts and logs.
#
# Why this exists
# ---------------
# v1.2.1 shipped with `tauri-plugin-log` declared only under
# `[target.'cfg(target_os = "macos")'.dependencies]`. On Windows the crate was
# never linked, the plugin could not be registered even in principle, and the
# app wrote no log file anywhere. Nothing caught it: `cargo test` passes because
# the code under test compiles on the platform running the tests, `cargo build`
# passes because a plugin you never registered is not a compile error, and a
# type-check knows nothing about Cargo target tables. The defect was only
# observable by starting the program and looking.
#
# So that is what this does. It is the only check in this repository that can
# see a capability that is silently ABSENT at runtime.
#
# It also pins the shape of the fix. A second defect, found the same way, was
# that `tauri_plugin_log::Builder::new().target(x).target(y)` APPENDS to the two
# targets the builder already carries, so the app wrote every line twice into
# two byte-identical files. Asserting "exactly one log file" is what keeps that
# from coming back.
#
# What it does NOT prove: that recording works. There is no audio device on a CI
# runner. CLAUDE.md §4 still requires a human record→transcript smoke test on
# macOS and Windows before release.
#
# Usage:
#   tools/ci/verify-startup.sh [path-to-binary]
# Default binary: target/debug/mityu
#
# Requires on Linux: xvfb-run (there is no display on a CI runner).
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
BIN="${1:-$REPO_ROOT/target/debug/mityu}"
STARTUP_TIMEOUT="${STARTUP_TIMEOUT:-90}"
# The line the app emits once `setup` has finished — i.e. the plugins are
# registered and the app is past its own initialisation, not merely process-alive.
READY_LINE="Application setup complete"

fail() { echo "FAIL: $*" >&2; exit 1; }
note() { echo "  $*"; }

[ -x "$BIN" ] || fail "no executable at $BIN (build it first: cargo build -p mityu)"

case "$(uname -s)" in
  Linux)
    LOG_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/com.bluedev.mityu/logs"
    command -v xvfb-run >/dev/null || fail "xvfb-run not found; install xvfb"
    LAUNCH=(xvfb-run -a -s "-screen 0 1280x1024x24" "$BIN")
    ;;
  Darwin)
    LOG_DIR="$HOME/Library/Logs/com.bluedev.mityu"
    LAUNCH=("$BIN")
    ;;
  MINGW*|MSYS*|CYGWIN*)
    LOG_DIR="$APPDATA/com.bluedev.mityu/logs"
    LAUNCH=("$BIN")
    ;;
  *) fail "unsupported platform: $(uname -s)" ;;
esac

echo "verify-startup: $BIN"
echo "  log dir: $LOG_DIR"

# Start from a clean slate, or "a log file exists" proves nothing — it could be
# last run's.
rm -rf "$LOG_DIR"

STDOUT_LOG="$(mktemp)"
trap 'rm -f "$STDOUT_LOG"' EXIT

"${LAUNCH[@]}" >"$STDOUT_LOG" 2>&1 &
APP_PID=$!
# xvfb-run execs a wrapper, so the app is a descendant, not $APP_PID itself.
# Kill the whole process group on the way out.
cleanup() {
  kill -- -"$APP_PID" 2>/dev/null || kill "$APP_PID" 2>/dev/null || true
  wait "$APP_PID" 2>/dev/null || true
}
trap 'cleanup; rm -f "$STDOUT_LOG"' EXIT

# Poll rather than sleep a fixed amount: a fast machine should not wait, and a
# slow one should not fail spuriously.
deadline=$((SECONDS + STARTUP_TIMEOUT))
ready=false
while [ $SECONDS -lt $deadline ]; do
  if [ -d "$LOG_DIR" ] && grep -qs "$READY_LINE" "$LOG_DIR"/*.log 2>/dev/null; then
    ready=true
    break
  fi
  if ! kill -0 "$APP_PID" 2>/dev/null; then
    echo "--- captured output ---" >&2
    cat "$STDOUT_LOG" >&2
    fail "the app exited after $SECONDS s without logging '$READY_LINE'"
  fi
  sleep 1
done

if [ "$ready" != true ]; then
  echo "--- captured output ---" >&2
  cat "$STDOUT_LOG" >&2
  [ -d "$LOG_DIR" ] || fail "no log directory after ${STARTUP_TIMEOUT}s — is the log plugin registered on this platform?"
  fail "log file never contained '$READY_LINE' within ${STARTUP_TIMEOUT}s"
fi

note "app started and logged '$READY_LINE' after ${SECONDS}s"

# EXACTLY ONE log file. Two means the target list was appended to rather than
# replaced, which also quietly doubles the rotation ceiling.
shopt -s nullglob
LOG_FILES=("$LOG_DIR"/*.log)
shopt -u nullglob
if [ "${#LOG_FILES[@]}" -ne 1 ]; then
  printf '  found: %s\n' "${LOG_FILES[@]}" >&2
  fail "expected exactly 1 log file, found ${#LOG_FILES[@]} — tauri_plugin_log targets are being appended, not replaced (use .targets([...]), not repeated .target(...))"
fi
note "exactly one log file: ${LOG_FILES[0]}"

# The file must hold real content, not just exist. Do NOT assert a line count:
# the poll above returns the moment the ready line is flushed, so any threshold
# above one is a race against the app still writing — this check failed that way
# on its own second run.
[ -s "${LOG_FILES[0]}" ] || fail "log file is empty"
note "log file holds the startup line ($(wc -c <"${LOG_FILES[0]}") bytes at the moment of the check)"

echo "PASS: the app starts and writes exactly one log file on $(uname -s)"
