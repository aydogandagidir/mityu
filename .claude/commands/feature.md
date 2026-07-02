---
description: Implement a new feature end-to-end while preserving local-first and tenant-readiness invariants.
argument-hint: <short feature description>
---
Implement this feature: **$ARGUMENTS**

Follow this workflow strictly:
1. **Orient.** Read CLAUDE.md §0/§2/§7, the relevant docs/ file, and the actual code you will touch (grep for real locations — do not guess). Summarize your plan in ≤8 bullets and the files you will change.
2. **Classify the layer.** Is this UI (frontend-nextjs-engineer), Rust core (rust-tauri-core-engineer), audio (audio-pipeline-engineer), or server/tenancy (sync-server-architect + multitenancy-guardian)? Route accordingly. Ask nothing the code already answers.
3. **Design for the seam.** If it persists data, define the entity with `id/workspace_id(tenant_id)/timestamps` (+ sync fields if it will sync). If it calls an LLM, keep it provider-agnostic and BYOK. If it produces AI output, render it as a human-approved draft linked to the source segment.
4. **Implement in small commits**, behind a flag if risky.
5. **Verify Definition of Done** (CLAUDE.md §5): builds, lints, offline works, tenant-safe, secrets/paths clean, HITL intact.
6. **Invoke multitenancy-guardian** to review the diff before declaring done.
7. **Update docs/** + add an ADR to docs/DECISIONS.md if architecture/schema changed.
State any assumption inline; proceed on the smallest safe interpretation rather than stalling.
