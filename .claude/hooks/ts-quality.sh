#!/usr/bin/env bash
# PostToolUse: reminder to typecheck/lint when TS/TSX touched. Non-blocking.
set -euo pipefail
payload="$(cat)"
PY="$(command -v python3 || command -v python || echo python)"   # Windows Git Bash has no python3 alias
fp="$(printf '%s' "$payload" | "$PY" -c "import sys,json;d=json.load(sys.stdin);print(d.get('tool_input',{}).get('file_path',''))" 2>/dev/null || true)"
case "$fp" in
  *.ts|*.tsx) echo "Note: run 'pnpm run lint' and 'pnpm tsc --noEmit' in /frontend before PR." >&2 ;;
esac
exit 0
