#!/usr/bin/env bash
# PreToolUse guard for the (future) server: any server-side SQL/repository code
# touching tenant data MUST be tenant-scoped. This is a heuristic reminder, not a proof.
set -euo pipefail
payload="$(cat)"
PY="$(command -v python3 || command -v python || echo python)"   # Windows Git Bash has no python3 alias
file_path="$(printf '%s' "$payload" | "$PY" -c "import sys,json;d=json.load(sys.stdin);print(d.get('tool_input',{}).get('file_path',''))" 2>/dev/null || true)"
content="$(printf '%s' "$payload" | "$PY" -c "import sys,json;d=json.load(sys.stdin);print(d.get('tool_input',{}).get('content','')+d.get('tool_input',{}).get('new_string',''))" 2>/dev/null || true)"

# Only enforce inside the server component.
case "$file_path" in
  *"/server/"*|*"/sync-server/"*)
    if printf '%s' "$content" | grep -qiE 'SELECT .* FROM (meetings|transcripts|summaries|action_items|documents)'; then
      if ! printf '%s' "$content" | grep -qiE 'tenant_id|org_id|WHERE .*tenant'; then
        echo "WARN: server query over tenant data without a visible tenant_id/org_id scope in $file_path. Confirm RLS is active OR add explicit tenant scoping. See docs/MULTITENANCY.md." >&2
        exit 2
      fi
    fi
  ;;
esac
exit 0
