#!/usr/bin/env bash
# PostToolUse: format + lint Rust only if a .rs file was touched. Non-blocking on warnings.
set -euo pipefail
payload="$(cat)"
PY="$(command -v python3 || command -v python || echo python)"   # Windows Git Bash has no python3 alias
fp="$(printf '%s' "$payload" | "$PY" -c "import sys,json;d=json.load(sys.stdin);print(d.get('tool_input',{}).get('file_path',''))" 2>/dev/null || true)"
[[ "$fp" == *.rs ]] || exit 0
command -v cargo >/dev/null 2>&1 || exit 0
( cd "$CLAUDE_PROJECT_DIR" && cargo fmt --quiet 2>/dev/null || true )
echo "Note: run 'cargo clippy --all-targets' before opening a PR (see CLAUDE.md quality gates)." >&2
exit 0
