#!/usr/bin/env bash
# PreToolUse guard: block writing obvious live secrets into source.
# Reads the tool payload from stdin (Claude Code hook contract).
set -euo pipefail
payload="$(cat)"
PY="$(command -v python3 || command -v python || echo python)"   # Windows Git Bash has no python3 alias
file_path="$(printf '%s' "$payload" | "$PY" -c "import sys,json;d=json.load(sys.stdin);print(d.get('tool_input',{}).get('file_path',''))" 2>/dev/null || true)"
content="$(printf '%s' "$payload" | "$PY" -c "import sys,json;d=json.load(sys.stdin);print(d.get('tool_input',{}).get('content','')+d.get('tool_input',{}).get('new_string',''))" 2>/dev/null || true)"

# Never allow real API keys / private keys committed to source.
if printf '%s' "$content" | grep -qE 'sk-[A-Za-z0-9]{20,}|AKIA[0-9A-Z]{16}|AIza[0-9A-Za-z_-]{30,}|-----BEGIN (RSA |EC )?PRIVATE KEY-----|xoxb-[0-9A-Za-z-]{20,}'; then
  echo "BLOCKED: potential live secret detected in edit to $file_path. Use env vars / OS keychain / Tauri secure store, never hardcode." >&2
  exit 2   # exit code 2 => Claude Code blocks the tool call and feeds stderr back to the agent
fi
exit 0
