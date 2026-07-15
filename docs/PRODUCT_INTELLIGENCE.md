# Product Intelligence

Mityu's intelligence layer is built evidence-first: retrieval must be local,
tenant-scoped and source-resolvable before a generative answer is allowed to sit
on top of it. Product intelligence never weakens the offline core, HITL approval
or the A5/C8 evidence gates. ADR-0027 defers those gates for publication of
v1.0.4 only; neither is passed and all downstream dependencies remain locked.

## Delivery order

1. **Ranked Evidence Search** — FTS5/BM25 over transcript segments, direct
   source jump, deterministic offline baseline.
2. **Approved Action Center** — read-only aggregation of human-approved action
   items; separate AI-review status from later work-progress status.
3. **Ask This Meeting** — retrieval-first answer drafts whose claims cite the
   evidence search contract; unsupported claims are refused/flagged.
4. **Local hybrid retrieval** — benchmark a versioned multilingual embedding
   model against the lexical baseline before adding it to the index.
5. **Ask Workspace / commitment graph** — approved data only, source-linked,
   tenant-scoped; no autonomous external action.

## Slice 1 — Ranked Evidence Search v1

### Contract

Each hit contains:

- meeting id/title;
- the persisted transcript segment id (`source_chunk_id`);
- wall-clock and recording-relative timestamps;
- a plain-text snippet from the authoritative transcript row;
- source kind (`transcript` in v1);

BM25 is internal ordering metadata. Its corpus-dependent numeric score is not a
confidence value and is not exposed through the API.

The UI consumes backend order verbatim and opens the matching transcript segment
through the existing jump/highlight path.

### Trust and privacy rules

- Fully local: an indexed document map + SQLite FTS5 inside the SQLCipher
  database; no model, server or network call.
- Every query joins the derived hit back to active `transcripts` and `meetings`
  rows scoped by `AuthContext.tenant_id`.
- User text is tokenized, bounded and quoted before `MATCH`; raw FTS syntax is
  never accepted and one-character prefix tokens are discarded.
- Query text and snippets are meeting content and must not be logged or sent to
  analytics.
- Legacy or unapproved AI summaries are not indexed. Approved summary blocks
  and approved action items require a separate additive HITL-gated slice.
- The FTS index duplicates transcript text inside encrypted SQLite. Until a
  database-wide secure-delete policy is accepted, deletion means logical/index
  removal, not a claim of forensic physical erasure. BACKLOG C6a makes the
  SQLite/FTS/free-page/WAL policy and sentinel verification a C8 release gate.
- Phase 1 has one implicit local workspace. Raw BM25 is not exposed; multiple
  local workspaces require tenant-local/partitioned ranking before enablement.

### Acceptance gates

- Populated-database migration backfills existing active segments and reruns are
  ledger-idempotent.
- Insert, update, retranscription, soft-delete and hard-delete keep the derived
  index coherent.
- Negative cross-workspace tests return zero foreign hits.
- Every returned `source_chunk_id` resolves under the caller's workspace.
- Turkish/English Unicode, prefix queries and operator-like input do not error
  or escape the literal query builder.
- SQLCipher conversion preserves both FTS content and its maintenance triggers.
- UI debounces input, ignores stale responses, preserves backend order and jumps to
  the correct segment.
- A database upgraded through migration `20260714000000` requires an app build
  containing that migration or newer; SQLx application downgrade is unsupported.

## Slice 2 — Approved Action Center v1

### Contract

The Action Center is a local, read-only projection of action items that have an
explicit item-level human approval. Each row contains:

- action id, text, optional assignee and free-text due value;
- meeting id, title and creation timestamp;
- review status (`approved` only; named `reviewStatus` on the wire);
- the persisted transcript segment id plus wall-clock and recording-relative
  source timestamps.

Item approval is independent of whole-summary approval. `edited` means a human
changed the draft and it awaits re-approval, so it is not shown. The existing
`status` column is the HITL review axis, not task progress: v1 has no checkbox,
`open`/`done` state, overdue inference or automatic follow-up. A future work
state requires a separate additive field and workflow.

Pages are bounded (100 by default, 200 maximum) and return `hasMore` plus
`nextOffset`, so the limit never silently hides results. Backend order is newest
meeting first, then extraction position and id; the UI preserves that order.
The free-text `due` value is displayed verbatim and is never treated as a date.

### Trust and privacy rules

- Generation repositories force every incoming summary block and action item
  to `draft`; an LLM or imported payload cannot mint approval by supplying a
  status or forged edit provenance.
- Approval is an atomic compare-and-swap write and succeeds only while an active
  same-workspace transcript source in an active same-workspace meeting exists.
- The read joins `action_items`, `meetings` and `transcripts` on workspace,
  meeting and source identity, requires all three rows to be active, and filters
  exactly `action_items.status = 'approved'`. A stale or corrupt link fails
  closed even during retranscription downgrade timing.
- Legacy `transcripts.action_items` and `summary_processes.result` JSON never
  enter this trusted surface.
- No transcript text is duplicated in the response. Action text, assignee, due,
  meeting title and source metadata are not logged or sent to analytics.
- The surface is fully offline and does not enable `structuredSummaries` or
  invoke an LLM, provider, server or integration.

### Acceptance gates

- Incoming `approved`/`edited`/`rejected` generation states persist as `draft`.
- Draft, edited, rejected, soft-deleted and cross-workspace items are excluded.
- Wrong-meeting, foreign-workspace, stale and soft-deleted sources are excluded;
  soft-deleted meetings are excluded.
- Retranscription downgrades remove an item from the projection.
- Pagination is visible and deterministic; wire fields are camelCase.
- The source button opens and highlights the exact transcript segment through
  the existing meeting deep-link path.
- The UI exposes provenance, loading, empty and retry states but no mutation or
  work-completion control.

## Gate discipline

This quality-independent foundation may be implemented before A5 by explicit
owner direction (ADR-0024 and ADR-0025). ADR-0027 permits v1.0.4 publication
while A5 and C8 are explicitly deferred, but it does not make transcription
quality proven or pilot value demonstrated. A5 human GO and the C8 offline
record→review→approve→export gate remain required before their dependent phases
or any target-environment quality/value claim.
