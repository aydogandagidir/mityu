# BOOTSTRAP — Exact Claude Code startup sequence

Hand this repo to Claude Code and run these prompts **in order**. Each step has a stop condition; do not proceed until it is met. This sequence is authoritative — the agent should not invent its own order.

## Step 0 — Orient (once)
> "Read CLAUDE.md, then every file in docs/, then .claude/agents/ and .claude/commands/. Summarize the architecture, the phase gates, and the invariants in ≤15 bullets. List anything in the actual repo that contradicts these docs. Do not change code yet."

**Stop when:** the agent has produced the summary and a contradictions list. Resolve contradictions by updating docs (add an ADR) before coding.

## Step 1 — Environment
> "Follow docs/SETUP.md. Verify toolchains (Rust, Node/pnpm, whisper models, Ollama, platform audio deps). Get `pnpm run tauri:dev` to build and launch. Report what is missing; do not guess versions."

**Stop when:** the app builds and launches locally (record→transcript works on this machine).

## Step 2 — Rebrand to Mityu (Phase 0)
> "Execute the rebrand in docs/ROADMAP.md Phase 0: set productName/identifier/title, package.json name, Cargo.toml crate name to Mityu / com.bluedev.mityu / mityu. Replace icons and user-facing 'meetily' strings. Keep the MIT LICENSE (Zackriya Solutions) intact. Build and verify the app still launches."

**Stop when:** app launches branded as Mityu; `LICENSE` unchanged; build green.

## Step 3 — Lock the three open decisions
> "Fill ADR-0003 (server language), ADR-0004 (authoritative audio module: audio vs audio_v2), ADR-0005 (audio retention). For ADR-0004, inspect both modules and recommend which is authoritative with evidence. Record decisions in docs/DECISIONS.md."

**Stop when:** ADR-0003/0004/0005 are Accepted with rationale.

## Step 4 — Phase 0 transcription validation (MAKE-OR-BREAK GATE)
> "Follow docs/PHASE0_VALIDATION.md exactly. Build the harness, run whisper large-v3 vs Parakeet on the provided audio set, compute WER, test the domain vocabulary. Produce the report and the go/no-go verdict."

**Stop when:** the validation report exists AND the go/no-go threshold is met. **If not met, STOP feature work** and narrow scope (meeting-room only) per the protocol. This is a human-reviewed gate — do not self-approve past a failing WER.

## Step 5 — Introduce the seams (still single-tenant, local-first)
> "Implement docs/CONTRACTS.md: WorkspaceContext/AuthContext, the tenant-scoped Repository layer, and the dormant sync module skeleton. Add the `workspace_id` migration per docs/DATA_MODEL.md and /db-migration. Prove the app still works fully offline. Invoke multitenancy-guardian to review."

**Stop when:** seams exist, migration applies, offline works, guardian passes.

## Step 6 onward — Work the backlog
> "Open docs/BACKLOG.md. Take the next unblocked task in order. Use the slash command it names (/feature, /add-tauri-command, etc.). Satisfy its acceptance criteria and the CLAUDE.md Definition of Done. Run /tenant-check and (before any release) /security-review. Update docs + add ADRs when architecture/schema changes."

**Rule for the whole run:** never skip a gate; never break the local-first or tenant invariants; treat audio and DB schema changes as high-risk (separate changes). When a task is ambiguous, state the assumption inline and proceed on the smallest safe interpretation.
