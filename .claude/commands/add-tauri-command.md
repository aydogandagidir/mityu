---
description: Add a new Tauri command (Rust) with its typed frontend binding, the safe way.
argument-hint: <command name and purpose>
---
Add a Tauri command for: **$ARGUMENTS**

1. Define the Rust `#[tauri::command]` with **typed args and return** (serde). Use `anyhow::Result` internally; map to a serializable error for the frontend.
2. Register it in `src-tauri/src/lib.rs` command handler list.
3. Enforce invariants: no hardcoded secrets/paths; if it persists data, include `workspace_id/tenant_id/timestamps`; if it calls an LLM, provider-agnostic + BYOK from secure store; degrade gracefully offline.
4. Add a **typed TS wrapper** in `frontend/src/services/` (no raw `invoke` in components).
5. Add a Rust test for non-trivial logic.
6. `cargo build` + `cargo clippy` + `pnpm tsc --noEmit` clean. Update docs if it changes the API surface.
