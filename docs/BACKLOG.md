# BACKLOG — Ordered, executable tasks

The agent works this list **top-to-bottom**, respecting `depends-on`. Each task: an ID, the owning agent, the slash command to use, concrete acceptance criteria, and its gate. "Done" also requires the CLAUDE.md Definition of Done. Do not reorder to chase easy wins; dependencies exist for correctness.

Legend: **Agent** = `.claude/agents/` file · **Cmd** = `.claude/commands/`.

---

## EPIC A — Foundation (Phase 0)

### A1 · Wire the pack & orient
- Agent: (orchestrator) · Cmd: — · depends-on: none
- AC: BOOTSTRAP Step 0 done; contradictions between docs and repo resolved via ADRs.

### A2 · Dev environment reproducible
- Agent: qa-release-engineer · Cmd: — · depends-on: A1
- AC: SETUP.md "environment ready" checklist passes on macOS and Windows (state which were verified); offline summary via Ollama works.

### A3 · Rebrand to Mityu
- Agent: rust-tauri-core-engineer + frontend-nextjs-engineer · Cmd: /feature · depends-on: A2
- AC: productName/identifier/title, package.json, Cargo.toml → Mityu/com.bluedev.mityu/mityu; icons + strings replaced; MIT LICENSE intact; app launches branded.

### A4 · Lock ADR-0003/0004/0005
- Agent: sync-server-architect (0003), audio-pipeline-engineer (0004) · Cmd: — · depends-on: A2
- AC: server language chosen; authoritative audio module identified with evidence; retention default confirmed. All Accepted in DECISIONS.md.

### A5 · Phase 0 transcription validation ⏸ DEFERRED — not a v1.0.4 publication blocker
- Agent: audio-pipeline-engineer + qa-release-engineer · Cmd: /phase0-validate · depends-on: A2
- AC: PHASE0_VALIDATION.md report produced; WER + domain-vocab thresholds met; **human-reviewed go/no-go recorded**. If NO-GO → scope narrows to meeting-room; do not enter EPIC C field features.
- Current evidence (2026-07-15): Whisper `large-v3` and Parakeet v3 int8 are installed/integrity-verified and the harness fails closed correctly. The four consented real-audio buckets remain `0/5`; twenty 2–10 minute recordings, human-corrected references, diarization review and the human verdict are still required. This is NOT EVALUATED, not a measured quality NO-GO.
- v1.0.4 exception (ADR-0027): the product owner accepted this as explicit evidence debt for this patch only. A5 is neither PASS nor waived for field/accuracy claims; it must close before those claims or any downstream phase that depends on proven target-environment quality.
- **Execution runbook: `docs/A5_SPRINT.md`** (2026-07-26). Owner decisions locked there: the `jargon` bucket splits into 5 legal + 5 intralogistics clips (**25 recordings total**, not 20), and the §4 GO/NO-GO numbers are locked *before* measurement. Two pre-sprint code debts are recorded: the harness build recipe (vendored-OpenSSL/perl) and per-clip vocabulary selection — the global ~600-char vocab prompt would otherwise bias only whichever domain is listed first in the jargon file.

---

## EPIC B — Tenant-aware seams (Phase 1, still single-tenant/local)

### B1 · WorkspaceContext / AuthContext seam
- Agent: rust-tauri-core-engineer · Cmd: /feature · depends-on: A3
- AC: `AuthContext { tenant_id, user_id, roles, request_id }` exists (docs/CONTRACTS.md); in local mode resolves to a single local user/workspace; no code reads "current user" any other way.

### B2 · `workspace_id` on all entities + repository layer
- Agent: db-migration-engineer + rust-tauri-core-engineer · Cmd: /db-migration then /prep-multitenant · depends-on: B1
- AC: forward-only migration adds `workspace_id` (+ sync fields) to meetings/transcripts/chunks/summaries/action_items; a tenant-scoped Repository is the ONLY storage access path; migration applies on empty + populated DB.

### B3 · Encrypted local store (SQLCipher)
- Agent: rust-tauri-core-engineer + security-privacy-auditor · Cmd: /feature then /security-review · depends-on: B2
- AC: sensitive local data encrypted at rest; key from OS-protected store; app still opens existing data (migration path documented).

### B4 · Dormant sync module skeleton
- Agent: rust-tauri-core-engineer · Cmd: /add-tauri-command · depends-on: B2
- AC: `sync/` client module with typed protocol messages (docs/CONTRACTS.md) but disabled; app fully works with it off.

---

## EPIC C — Core product value (Phase 1 MVP)

### C1 · Source-linked structured summaries + HITL
- Agent: rust-tauri-core-engineer + frontend-nextjs-engineer · Cmd: /feature · depends-on: B2
- v1.0.4 sequencing exception (ADR-0027): source linkage and HITL are quality-independent safety controls and may ship while A5 remains NOT EVALUATED. This does not establish transcription accuracy.
- AC: summary uses the Block/Section schema; **every block/action item carries `source_chunk_id`**; UI renders drafts with Approve + visible source link; nothing publishes without approval.

### C2 · Action-item extraction
- Agent: rust-tauri-core-engineer · Cmd: /feature · depends-on: C1
- AC: action items (text, assignee?, due?, status, source_chunk_id) extracted as drafts; editable; approved items persisted.

### C3 · Search across meetings/transcripts
- Agent: rust-tauri-core-engineer + frontend-nextjs-engineer · Cmd: /feature · depends-on: B2
- AC: superseded/strengthened by C3a for transcript evidence. Summary retrieval may index **only human-approved, source-linked** blocks in a later additive slice; legacy or unapproved summary text is prohibited from the trusted search surface.

### C3a · Ranked evidence search (Product Intelligence foundation)
- Agent: rust-tauri-core-engineer + frontend-nextjs-engineer · Cmd: /feature then /db-migration · depends-on: B2
- Owner-directed sequencing exception (2026-07-14, ADR-0024; narrowed for v1.0.4 by ADR-0027): transcription-quality-independent implementation may land while A5 and C8 remain deferred/not passed; no target-environment quality or pilot-value claim follows from it.
- AC: local FTS5/BM25 ranks transcript evidence without a network/LLM dependency; every result resolves to an active same-workspace `source_chunk_id`; query syntax is bounded/escaped and one-character prefix scans are rejected; UI preserves backend relevance order, debounces and rejects stale responses, and opens/highlights the source segment; raw corpus-dependent rank, query and snippet never enter analytics/logs. Legacy/unapproved summaries are excluded; approved-summary retrieval is a later additive slice.

### C3b · Approved Action Center (Product Intelligence slice 2)
- Agent: rust-tauri-core-engineer + frontend-nextjs-engineer + security-privacy-auditor · Cmd: /feature · depends-on: C2, B2
- Owner-directed sequencing exception (2026-07-14, ADR-0025; narrowed for v1.0.4 by ADR-0027): this read-only, quality-independent slice may land while A5 and C8 remain deferred/not passed; no target-environment quality or pilot-value claim follows from it.
- AC: generation cannot persist a non-draft review state; only active same-workspace `action_items.status = 'approved'` rows with an active same-meeting transcript source are returned; draft/edited/rejected, stale, soft-deleted and cross-tenant rows fail closed. The bounded API exposes visible pagination and source metadata; the `/actions` UI preserves backend order, shows AI-extracted/human-approved provenance and opens the exact transcript segment. V1 is offline/read-only and adds no work-progress state, overdue inference, analytics content or automatic external action.

### C4 · Export (PDF / DOCX / Markdown)
- Agent: frontend-nextjs-engineer · Cmd: /feature · depends-on: C1
- AC: a meeting's approved summary + action items export to PDF/DOCX/MD with source timestamps; works offline.

### C5 · Consent + transparency UI
- Agent: frontend-nextjs-engineer + security-privacy-auditor · Cmd: /feature then /security-review · depends-on: C1
- AC: visible "recording active" indicator; analytics opt-in; "AI-generated (review required)" labeling; these cannot be hidden. Backs EU AI Act Art. 50.

### C6 · Retention & redaction policy (local)
- Agent: rust-tauri-core-engineer + security-privacy-auditor · Cmd: /feature · depends-on: B3
- AC: configurable retention (default: delete audio after transcription); basic PII/keyword redaction rules applied before persistence/summary.

### C6a · Verifiable local deletion semantics ✅ CLOSED for v1.0.4
- Agent: rust-tauri-core-engineer + security-privacy-auditor · Cmd: /security-review then /feature · depends-on: B3, C3a
- AC: an accepted ADR defines SQLite/FTS5 deletion semantics across FTS shadow tables, free pages and WAL; the implementation applies the chosen `secure_delete` + checkpoint/vacuum or crypto-erasure policy; an automated sentinel test verifies the documented guarantee after the maintenance cycle; SSD/filesystem limitations are disclosed in-product. This must pass before the C8 security sign-off—logical/index removal alone is insufficient for a forensic-erasure claim.
- Closure evidence (2026-07-14): ADR-0026 accepted; migration `20260714010000` persists FTS5 secure-delete and a content-free crash-resume marker; every SQLite connection enables core secure-delete; tenant-scoped deletion scrubs only canonical-root Mityu-managed artifacts without following symlinks, retains unknown user files, then requires FTS optimize + checked WAL truncate + `VACUUM` + zero free pages + final checkpoint before success. Startup resumes pending maintenance and browser recovery copies are logically purged. `secure_local_deletion.rs` uses a unique sentinel to verify database, FTS, WAL and app-managed artifacts, including a cross-tenant no-op. The product copy explicitly disclaims SSD/COW/snapshot/backup/export/WebView physical-erasure guarantees. A5 and C8 remain open roadmap gates but are deferred/non-blocking for v1.0.4 under ADR-0027; legal, signing and updater-canary gates remain unchanged. The Windows FFmpeg technical publication gate closed on 2026-07-15.

### C7 · Editor convergence to BlockNote
- Agent: frontend-nextjs-engineer · Cmd: /refactor via /feature · depends-on: C1
- AC: canonical editor = BlockNote; no new TipTap/Remirror usage; legacy paths inert.

### C9 · EU AI Act Art. 50(1) assessment for "Ask This Meeting" ⛔ RELEASE GATE
- Agent: security-privacy-auditor · Cmd: /security-review · depends-on: —
- AC: ADR-0032 states that the conversational Ask surface "would engage [Art. 50(1)] and must be assessed before it ships". The surface is on `main` (`SummaryPanel.tsx` mounts `AskPanel` whenever a transcript exists, with no flag) but is in **no released binary** — the newest tag anywhere is `v1.0.4`, which predates it. So the gate is **due, not breached**, and the cheap window closes at the next tag. Either record the assessment as an ADR-0032 amendment, or put the panel behind a default-off flag until it is done. Also carries ADR-0032's two counsel questions (does Art. 50(2) reach HITL summarisation; Art. 50(1) for the Ask surface).

### C10 · NPU enumeration + OS-native inference backends (ADR-0033)
- Agent: rust-tauri-core-engineer · Cmd: /feature · depends-on: —
- AC: ADR-0033 landed the capability probe and a dormant seam, deliberately without faking the parts it could not implement: Windows/Linux NPU enumeration, the OS-native backend implementations themselves, and the wiring into provider assembly and the UI. `NpuVendor::Undetermined` must stay distinguishable from "none".

### C11 · Learning system (ADR-0030) — in-app live smoke test
- Agent: — (needs a running GUI + a live model) · Cmd: — · depends-on: —
- AC: the learning loop is built and unit-tested, but has never been exercised end-to-end through the app: CPU build, Ollama configured, "look for patterns" on, make ≥3 corrections and approve, and confirm a "Suggested by the model" Proposed rule appears. An agent cannot substitute for this; it needs the GUI and a model.

### C8 · Phase 1 exit ⏸ DEFERRED for v1.0.4 — still a downstream gate
- Agent: qa-release-engineer · Cmd: /release (dry-run) · depends-on: C1–C7 (including C6a)
- AC: app works fully offline; a real pilot user completes record→approve→export; DoD green; multitenancy-guardian + security-privacy-auditor pass.
- Evidence protocol: `docs/PILOT_V1.0.4.md` is ready, but no human pilot has been performed. ADR-0027 makes C8 non-blocking only for publication of v1.0.4; it is not PASS, unlocks none of EPIC D/F/G, and still requires a real user to execute and sign the protocol against an immutable candidate after A5. An AI agent cannot substitute for the pilot user or approve the generated content.

---

## EPIC D — Optional server (Phase 2, only after C8 + a team-customer need)

### D1 · Server skeleton (NEW, clean) — auth + tenancy from commit #1
- Agent: sync-server-architect · Cmd: /feature · depends-on: A4(0003), C8
- AC: `server/` per ADR-0003; OIDC authn; AuthContext derived per request; Postgres + RLS; health/version; NOT the legacy backend.

### D2 · Tenant model + RBAC + audit
- Agent: sync-server-architect + db-migration-engineer · Cmd: /db-migration then /tenant-check · depends-on: D1
- AC: tenants/users/memberships; roles owner/admin/member/viewer enforced on every sensitive route; append-only audit log; **negative cross-tenant test passes**.

### D3 · Tenant-scoped sync API + enable client sync
- Agent: sync-server-architect + rust-tauri-core-engineer · Cmd: /feature then /tenant-check · depends-on: D2, B4
- AC: sync protocol (rev/updated_by/soft-delete, LWW + audit on conflict); client SQLite ↔ server Postgres; app still works with server DOWN.

### D4 · Team features (shared workspaces, admin console, SSO)
- Agent: sync-server-architect + frontend-nextjs-engineer · Cmd: /feature · depends-on: D3
- AC: share a meeting to a workspace; admin console scoped to one tenant; enterprise SSO via OIDC.

### D5 · Phase 2 exit ⛔ GATE
- Agent: qa-release-engineer + security-privacy-auditor · Cmd: /release · depends-on: D1–D4
- AC: cross-tenant isolation verified; app runs with server down; /security-review clean; first team/enterprise customer live.

---

## EPIC E — Managed SaaS (Phase 3, only after D5 + unit-economics validation)
- E1 per-tenant metering + billing · E2 hosted IdP + EU-region deploy · E3 isolation/scale hardening · E4 self-serve onboarding.
- GATE E5: unit economics positive; isolation & audit verified at scale.

---

## EPIC F — On-device AI agents (Phase 1+, optional; only after gate C8)

Backs the About "Coming soon: a library of on-device AI agents." Local-first, **draft-only (HITL)**, source-linked, tenant-scoped, **no autonomous external actions** (ADR-0013). A dormant seam already exists (`frontend/src-tauri/src/agents/`, off by default); these tasks turn it on in sequence. Meeting-platform (Zoom/Meet/Teams) *API* integration is intentionally **not** here — the app captures system audio and is not a meeting bot by default; opt-in integrations live in EPIC G (ADR-0018).

### F0 · ADR + agent boundaries ⛔ DESIGN GATE
- Agent: rust-tauri-core-engineer + security-privacy-auditor · Cmd: — · depends-on: C8
- AC: ADR-0013 confirmed at kickoff — agents local-first, draft-only (HITL), source-linked, tenant-scoped, no autonomous external actions; trigger = manual/on-demand first. Dormant `agents/` seam already merged; this formalizes scope before code.

### F1 · Agent framework (flag-gated, wired) + `agent_runs` store
- Agent: rust-tauri-core-engineer + db-migration-engineer · Cmd: /add-tauri-command then /db-migration · depends-on: F0
- AC: `AgentRunner` reachable via a flag-gated Tauri command; forward-only migration adds `agent_runs` (`workspace_id` + sync fields) via the tenant-scoped Repository; providers reuse the `summary/` provider layer; app works with agents OFF (default) and fully offline.

### F2 · Follow-up drafter agent
- Agent: rust-tauri-core-engineer + frontend-nextjs-engineer · Cmd: /feature · depends-on: F1, C1, C2
- AC: from an **approved** summary + action items, drafts a follow-up message as a DRAFT in the editor; user edits/approves; "send" is manual export/copy (**never** auto-send); every draft carries `source_chunk_id` links.

### F3 · Action-item tracker agent
- Agent: rust-tauri-core-engineer + frontend-nextjs-engineer · Cmd: /feature · depends-on: F2
- AC: aggregates open action items across meetings into a review list (status, due, source); no auto-notifications; tenant-scoped by construction.

### F4 · Agents panel (UI + transparency)
- Agent: frontend-nextjs-engineer + security-privacy-auditor · Cmd: /feature then /security-review · depends-on: F2
- AC: run-on-demand, draft review/approve, per-run audit; "AI-generated · review required" labels (EU AI Act Art. 50); these cannot be hidden.

### F5 · Opt-in scheduling / automation ⛔ GATE
- Agent: rust-tauri-core-engineer + security-privacy-auditor · Cmd: /feature then /security-review · depends-on: F4
- AC: optional scheduled runs; even then outputs are draft-by-default or require explicit per-action approval; fully offline; `/security-review` + multitenancy-guardian pass; **no autonomous irreversible action ships**.

---

## EPIC G — Opt-in Integrations (ADR-0018; the core stays connectionless)

The app ships deliberately unconnected; an Integrations section lets the user consciously enable each connection after reading its terms. Everything here is OFF by default, per-workspace, and the app must remain fully functional (manual mode) with all of it off.

### G1 · Integrations hub UI + consent framework
- Agent: frontend-nextjs-engineer + security-privacy-auditor · Cmd: /feature then /security-review · depends-on: C8
- AC: an Integrations section lists available integrations, each OFF by default; enabling shows that integration's scope/consent text which the user must explicitly accept (acceptance recorded per-workspace with timestamp); disconnect wipes local tokens/state; with everything off the app behaves exactly as before.

### G2 · Calendar metadata (Google Calendar / Microsoft 365, read-only)
- Agent: rust-tauri-core-engineer + frontend-nextjs-engineer · Cmd: /feature · depends-on: G1
- AC: opt-in read-only calendar connection enriches meetings on-device (title/time/attendees; optional "meeting starting — record?" prompt); OAuth tokens in the OS keychain (ADR-0011 pattern); nothing transits bluedev infrastructure; offline or not-consented → manual naming unchanged; privacy policy gains an Integrations section.

### G3 · Meeting bot (Zoom/Teams/Meet auto-join) ⛔ needs its own ADR before code
- Agent: sync-server-architect + security-privacy-auditor · Cmd: — (design first) · depends-on: G1, D5
- AC (frame only, per ADR-0018 Tier 2): bot joins only meetings the user connected and consented to; announces itself in-call; media path, processor role (DPA), retention and EU residency (E2) documented; per-integration kill switch; a detailed ADR + /security-review precede any implementation.

## EPIC H — Speaker diarization (ADR-0034 + ADR-0035)

> Ordering is not cosmetic here: H1 gates everything because a licence failure would throw away H3–H6 with no partial credit (ADR-0034 bans diarization-quality claims until A5's `multi` bucket exists, so there is nothing to bank early). H2 gates H3 *mechanically* — the moment `diarize-helper` is a workspace member, `ci.yml`'s `cargo test --all` on `ubuntu-latest` performs an unverified native download on every PR.

### H1 · Embedding + segmentation model licence verification ⛔ GATE — DONE 2026-08-09
- Agent: — · Cmd: — · depends-on: —
- AC: **met.** All three verified from primary sources and recorded in the ADR-0034 amendment (2026-08-09): pyannote segmentation-3.0 = MIT with the CNRS copyright, read from the `LICENSE` **inside** the sherpa tarball; CAM++ = Apache-2.0 from ModelScope's API and the 3D-Speaker repo LICENSE; TitaNet = CC-BY-4.0 from the HuggingFace API, not gated. Redistribution through sherpa's release assets preserves the required notices — verified by unpacking the artifact, not by argument.

### H2 · sherpa-onnx archive integrity bootstrap ⛔ PREREQUISITE FOR WORKSPACE MEMBERSHIP — DONE 2026-08-09
- Agent: qa-release-engineer · Cmd: /feature · depends-on: H1
- AC: **met.** `tools/diarization/fetch-sherpa-archive.py` fetches the archive from a pinned release tag and URL and verifies **exact byte length and SHA-256** for all five static targets (Windows x64, Linux x64/aarch64, macOS x64/arm64), re-verifying on every run rather than trusting the cache. It fails closed on a wrong digest, a wrong size, a malformed pin, and — unlike `build/ffmpeg.rs`, deliberately — on an **unpinned target**, because a warning there would restore the unverified download it exists to prevent. The `ci.yml` step is self-activating: a no-op until `Cargo.lock` contains `sherpa-onnx-sys`, so it costs nothing today and cannot be forgotten in the PR that adds `diarize-helper`.
- Proven end-to-end, with a control: with the verified directory supplied and `sherpa-onnx-sys` cleaned, the build completes and **never prints its download line**; from the *same* clean state with an empty directory it **fails** with `SHERPA_ONNX_ARCHIVE_DIR does not contain expected archive` — which is what proves the build script really re-ran, really consults the variable, and does not fall back to downloading.

### H3a · `diarize-helper` binary (ADR-0034 step c, engine half) — DONE 2026-08-09
- Agent: rust-tauri-core-engineer · Cmd: /feature · depends-on: H2
- AC: **met.** New workspace member on sherpa-onnx default (`static`) features. Audio in, anonymous turns out; no state, no database, no network. Emits JSON in exactly the `speaker_turns` shape *or* RTTM, so its output is scoreable by `eval-harness der` with no conversion step in between that could itself be wrong. Refuses a non-16 kHz or non-mono WAV rather than resampling or downmixing silently — a hidden conversion there would move every timestamp. `num_clusters` defaults to `-1` (unknown, cluster by threshold), so no speaker count is imposed on a two-person meeting.
- Proven on real audio: 16 s, two-speaker English sample with the real pyannote segmentation and CAM++ embedding models → **found exactly 2 speakers**, 4 turns, correct units and 1-based labels; its RTTM output re-scores 0.00% through `eval-harness der`. Fail-closed paths exercised: missing model, 8 kHz audio, stereo audio — all exit 1.
- `externalBin` is deliberately **not** added here: `tauri-build` fails when a listed sidecar binary is absent, so declaring it before anything invokes it would break every developer build for no benefit. That belongs with H3b.

### H3b-1 · Model manifest, verification and acquisition — DONE 2026-08-09
- Agent: rust-tauri-core-engineer · Cmd: /feature · depends-on: H3a
- AC: **met.** `src/diarization/models.rs` pins both artifacts by exact byte length and SHA-256 and verifies through `utils::verify_file_integrity` — the Parakeet shape, including its rule that an artifact **absent from the manifest is an error**. The segmentation archive's contents carry their own pins, because verifying the download says nothing about what is on disk a month later, and `status()` re-verifies on every call: a wrong model does not crash, it produces plausible and wrong speakers. The upstream MIT licence (`Copyright (c) 2022 CNRS`) is extracted and kept beside the model, since MIT requires the notice to travel with the copy. `tar` + `bzip2` add **no crate to the graph** — `sherpa-onnx-sys` already puts both in the lock file.
- Proven end to end: the real 6,958,444 B archive and 28,281,138 B embedding download, verify, extract; a second call is a no-op rather than a re-download; the extracted licence still carries the CNRS line. And the digests the manifest pins are **byte-identical** to the model files the helper demonstrably found 2 speakers with — so the manifest describes the artifacts that actually work, not merely artifacts that hash consistently. The network-dependent half is `#[ignore]`d so the suite does not fail for reasons unrelated to the code.

### H3b-2 · Wire the helper into the app — DONE 2026-08-09
- Agent: rust-tauri-core-engineer · Cmd: /add-tauri-command · depends-on: H3b-1
- AC: **met.** `externalBin` declared plus build steps in `ci.yml` and both `build.yml` platform paths (each running the archive bootstrap first). Four Tauri commands; availability is a four-state answer (`NoAudio` / `ModelsMissing` / `Ready` / `Done`) because collapsing any two makes the UI lie — in particular an empty result is an ANSWER and never-run is an OFFER, which only `diarized_at` separates. Availability uses `find_audio_file` itself, now `pub(crate)`, rather than a second copy that would eventually disagree with retranscription. Audio is decoded by the app's own decoder (already 16 kHz mono with a sinc resampler) rather than an FFmpeg subprocess — the helper refuses to resample, so the conversion has to be right, and this is the path retranscription already proves.
- Verified end to end with real models, the real sidecar and a real two-speaker recording: `ModelsMissing` → acquire → `Ready` → diarize → **4 turns, Speaker 1 + Speaker 2, stored** → `Done`. Plus 6 repository tests (tenant scoping in both directions, replace-not-append, empty pass still stamps, a refused write leaves prior turns intact) and 5 sidecar wire-contract tests.
- **Not exercised here:** `build.yml` runs only on dispatch/release, so its two new steps are unverified until a release. `ci.yml`'s step is exercised by this PR. A CONTROL was run for `externalBin`: with it declared and the binary absent, `tauri-build` fails with `resource path binaries\diarize-helper-… doesn't exist` — which is what makes those build steps mandatory rather than optional.
- No UI: speaker labels and talk-time are H6.

### H4 · Cross-platform CI for `diarize-helper`
- Agent: qa-release-engineer · Cmd: /release · depends-on: H3a
- AC: the helper is built on `windows-latest` and on macOS. Note the current gap: `ci.yml` builds and tests **only `ubuntu-latest`**, `release.yml` is Windows-only, and `build-macos.yml` is dispatch-only — so today a macOS break surfaces at release and a Windows break surfaces only in the release job.
- **Also fix, found 2026-08-09:** `build.yml`'s **llama-helper** macOS/Linux step builds for the runner host and copies the result under `inputs.target`'s name. On an Intel runner asked for `aarch64-apple-darwin` that puts an x86_64 binary inside an ARM app under an ARM filename — it looks right and fails to launch. The diarize-helper step was written from that same template and has been corrected to build with `--target`; llama-helper's has not, and is left as its own change because the macOS release path cannot be exercised from here.

### H5 · Diarization runtime + memory on a 60–90 minute recording — ✅ MEASURED 2026-08-10 (60 min)
- Agent: qa-release-engineer · Cmd: /audio-debug · depends-on: H3a
- AC: wall-clock and peak RSS measured on a long recording; decides whether H3 needs chunked progress reporting or can stay fire-and-forget. Clustering is superlinear in segment count, so a short-clip measurement does not answer this.

**Measured on Windows x64, release build, a 60.1-minute 16 kHz mono recording:**

| | |
|---|---|
| wall time | **14 min 27 s** (~4x faster than real time) |
| peak working set | **600 MB** |
| turns produced | 1491, none malformed, covering 0.3 s to 60.0 min |
| speech detected | 40.0 min of the 60.1 min file |

The 60-minute case completes and is not pathological. **The 90-minute case is being measured separately and is not inferred from this one** -- the acceptance criterion above says clustering is superlinear in segment count, so a linear extrapolation would contradict the very risk it is meant to settle.

**It answers ADR-0035's open question: yes, the pass needs progress reporting.** Fourteen minutes behind an "Analysing…" label with no percentage, no elapsed time and no cancel reads as a hung app. That is a UX gap, not a correctness one — filed as H12.

**Observed, and deliberately NOT reported as an accuracy result:** the test file alternates two voices and the pass reported **four** speakers — 18.9 / 16.1 / 4.8 / 0.3 minutes, so two spurious clusters holding ~13% of the speech. The material is not representative: the "second voice" is the same recording resampled to a higher pitch, which also speeds it up and adds artifacts, so it is not two humans. ADR-0034 forbids accuracy claims until the A5 `multi` bucket exists, and this measurement does not change that — it is a runtime and memory result only. It does suggest the `multi` bucket (H9) matters more than a nice-to-have.

**A methodology note worth keeping.** The first harness reported the helper "hung" for 70 minutes. It had not: the harness redirected stdout to a PIPE and only read it after the process exited, so the child blocked writing 187 KB of JSON into a full kernel buffer while the parent waited for an exit that could not come. The app is unaffected — `sidecar.rs` uses tokio's `Command::output()`, which drains both pipes concurrently. Any future harness must redirect to a file or drain concurrently, or it will measure its own deadlock.

### H6 · Speaker labels + talk-time UI (ADR-0034 step d) — ✅ DONE (ADR-0036)
- Agent: frontend-nextjs-engineer · Cmd: /feature · depends-on: H3b-2
- AC: a transcript row overlapping more than one turn shows more than one speaker (rows and turns do not align — see the migration's own note); talk-time is descriptive only, never a score or ranking; labels stay anonymous `Speaker N` and naming a speaker is a manual human action; the four states are distinguished — no audio / never ran / ran-inconclusive / has turns.

### H10 · GPL-3.0 espeak-ng in `diarize-helper` — ✅ FIXED 2026-08-10 (pin swap + vendored crate patch)
- Agent: qa-release-engineer · Cmd: /security-review · blocks: H7 (sherpa half), and shipping diarization at all
- **Measured, not argued.** `sherpa-onnx-sys` 1.13.4's static link list names `espeak-ng` and `piper_phonemize` (`build.rs:23-24`), and the pinned prebuilt archive carries `lib/espeak-ng.lib`. The **release** binary (`cargo build -p diarize-helper --release`, 18,465,280 B) contains **51 espeak function symbols** (`espeak_Initialize`, `espeak_CompileDictionary`, …), 3 espeak runtime error strings and 2 espeak Windows registry keys. `/OPT:REF` does **not** discard it — this was checked precisely because it discarded all of sherpa in the earlier link spike.
- **eSpeak NG is GPL-3.0** — verified from its own `COPYING` ("GNU GENERAL PUBLIC LICENSE Version 3, 29 June 2007"). Shipping this binary inside a closed commercial installer distributes GPL-3.0 object code without GPL terms or corresponding source.
- **The clean fix exists upstream:** sherpa-onnx gates both libraries behind one CMake option — `option(SHERPA_ONNX_ENABLE_TTS "Whether to build TTS related code" ON)`; espeak-ng and piper-phonemize are pulled in only inside `if(SHERPA_ONNX_ENABLE_TTS)`. Diarization needs neither.
- **But the crate cannot reach it:** `sherpa-onnx-sys` is prebuilt-only (`download_prebuilt_libs`, `SHERPA_ONNX_ARCHIVE_DIR`); it has no CMake path, so there is nowhere to pass `TTS=OFF`. Fixing this therefore needs BOTH a TTS-off archive AND a link list that omits `espeak-ng`/`piper_phonemize`.
- **Checked, and it is far smaller than first recorded: k2-fsa already publishes `no-tts` prebuilts.** Queried the v1.13.4 release (303 assets). A `-no-tts-lib` twin exists for every target we pin except one:

  | pinned target | current asset | `no-tts` twin |
  |---|---|---|
  | `x86_64-pc-windows-msvc` | `win-x64-static-MT-Release-lib` (119,847,445 B) | **yes** — `...-no-tts-lib` (116,684,776 B) |
  | `x86_64-unknown-linux-gnu` | `linux-x64-static-lib` | **yes** (21,142,120 B) |
  | `x86_64-apple-darwin` | `osx-x64-static-lib` | **yes** (18,236,816 B) |
  | `aarch64-apple-darwin` | `osx-arm64-static-lib` | **yes** (18,353,357 B) |
  | `aarch64-unknown-linux-gnu` | `linux-aarch64-static-lib` | **NO — upstream publishes none** |

  So the fix is a **pin swap plus a ~6-line crate patch**, not a from-source build, and ADR-0035's "pinned upstream immutable release" story survives intact. Two hardcoded things in `sherpa-onnx-sys` must change: `archive_name()` builds the TTS-on filename with no switch, and `STATIC_LIBS` names `piper_phonemize`, `espeak-ng` and `ucd`. Vendor a patched copy, or upstream a `no-tts` cargo feature.
- **The one gap is `aarch64-unknown-linux-gnu`**, for which upstream publishes no `no-tts` build at all. That target is pinned by `fetch-sherpa-archive.py` but is built by neither `ci.yml` nor `build.yml`, and ADR-0022 already records Linux system audio as broken — so the honest choices are to drop the pin (the bootstrap already fails closed on unpinned targets) or build that one target from source. Decide it explicitly rather than letting it ship TTS-on.
- If the above is rejected: build every target from source with `SHERPA_ONNX_ENABLE_TTS=OFF` (H2's `SHERPA_ONNX_ARCHIVE_DIR` indirection makes this mechanically easy, but the pin then covers *our* artifact and H2's integrity story needs rewriting), or move to a different engine. **Owner decision — it touches ADR-0035.**
- AC: the shipped `diarize-helper` binary contains zero espeak/piper symbols, proven the same way this was found (`strings` over the release binary), with that check wired into CI so it cannot regress. — **MET.** Release binary went 18,465,280 → 17,737,216 B; espeak symbols 51 → **0**, espeak runtime markers 3 → **0**, registry keys 2 → **0**, piper **0**. The 12 remaining case-insensitive "espeak" hits are all the word *Speaker* (`OfflineSpeakerDiarization`, `wespeaker`) — which is why `tools/diarization/check-no-gpl-espeak.py` matches anchored symbols and espeak's own runtime strings instead of a naive substring; a check that cries wolf on a clean binary gets switched off.
- **How it was done:** `vendor/sherpa-onnx-sys/` (22 files, upstream 1.13.4 with two declarations changed) wired via `[patch.crates-io]`, pins in `fetch-sherpa-archive.py` swapped to the `-no-tts` archives, and `aarch64-unknown-linux-gnu` dropped — the vendored `archive_name()` returns an explicit error for it rather than silently falling back to a TTS-on build. The link list was not guessed: it is exactly the 11 `.lib` files present in the no-tts archive.
- **Verified beyond linking.** `--help` proves only the CLI, which is the same false pass ADR-0035 records from the first link spike. Pointed at existing-but-invalid model files, the helper aborts with `Rust cannot catch foreign exceptions` — a C++ exception thrown from inside sherpa/ONNX Runtime, which is proof the native library is live and executing rather than discarded.
- **Guard wired into all three places that build the sidecar:** `ci.yml`, and both the Windows and macOS/Linux steps of `build.yml` — the latter matter most, since that workflow produces the installer, and the check runs BEFORE the binary is copied into the bundle. Each run does `--self-test` first, because a detector nobody has seen fail is indistinguishable from one that cannot fail. Controls: the old TTS-on binary fails it with 50 symbols and 8 markers; the new one passes.

### H7 · Third-party notices for the sidecar and models — ✅ DONE 2026-08-10
- Agent: qa-release-engineer · Cmd: /release · depends-on: H3b-2
- AC: `resources/MODEL-NOTICES.txt` gains the MIT text + `Copyright (c) 2022 CNRS` for segmentation and the Apache-2.0 attribution for CAM++ (published as a bare `.onnx` with no licence file, so the notice is ours to supply) — **DONE 2026-08-10**, guarded by `models::tests::every_downloaded_model_is_attributed_in_the_notice_file`, which fails if a pin is added without a notice. The sidecar half was **BLOCKED on H10** — the binary was not purely Apache-2.0, it also carried GPL-3.0 espeak-ng, so an Apache-2.0 notice would have been a false notice. H10 removed the espeak, and the notice is now written: `resources/SIDECAR-NOTICES.txt` plus the full `resources/COPYING.Apache-2.0.txt`, both bundled.
- **Every linked component's licence was verified from its own upstream repository, not assumed:** sherpa-onnx, kaldi-decoder, kaldifst, OpenFST (v1.8.5 fork), kaldi-native-fbank and simple-sentencepiece are Apache-2.0; ONNX Runtime is MIT; KISS FFT is BSD-3-Clause (SPDX identifier read from its own `COPYING`). GitHub classified kaldifst, kissfft and openfst as `NOASSERTION`, so those three were resolved by reading their licence files directly. All permissive; no GPL.
- The prebuilt archive ships **no licence files of its own** (only `lib/*.lib`), so the notices are Mityu's to supply — the same situation as CAM++.
- **The notice quotes the archive's SHA-256, not the helper binary's.** Mityu compiles the helper per release so its hash moves between builds; quoting one would be false in the shipped installer the moment it was written.
- Guarded by four tests in `diarization::sidecar::notice`, each confirmed load-bearing by mutation: bumping the pin while leaving the notice stale, writing the notice without bundling it, dropping a component from the list, and deleting the `no-tts` justification while leaving the no-GPL claim standing — all four fail the suite.

### H11 · `diarize-helper` aborts instead of erroring on an unreadable model
- Agent: rust-tauri-core-engineer · Cmd: /fix-bug · found while verifying H10
- Pointed at a file that exists but is not a valid ONNX model, the helper dies with `fatal runtime error: Rust cannot catch foreign exceptions, aborting` — a C++ exception from sherpa crossing the FFI boundary with no catch. Low severity in practice: models are SHA-256 verified before use (H3b-1), and the sidecar is a separate process so it cannot take the app down (ADR-0035). But the app sees a crash rather than a message, so the user is told nothing useful.
- AC: an unreadable model produces a clear error on stderr and a non-zero exit, not an abort.

### H12 · A diarization pass needs progress reporting
- Agent: rust-tauri-core-engineer · Cmd: /feature · found by H5
- H5 measured 14.5 minutes for a 60-minute recording. The UI shows "Analysing…" on a disabled button for that whole time — no percentage, no elapsed time, no cancel. That reads as a hung app, and a user will kill it.
- ADR-0035 flagged exactly this as the thing the measurement would decide: "this determines whether the sidecar needs chunked progress reporting." It does.
- AC: the pass reports progress the UI can show, and can be cancelled; a long pass never looks indistinguishable from a hang.

### H8 · DER instrument hardening
- Agent: qa-release-engineer · Cmd: /fix-bug · depends-on: —
- AC: `parse_rttm`/`parse_uem` strip a UTF-8 BOM (today a BOM silently drops the first `SPEAKER` record — and `Set-Content -Encoding utf8`, which the harness's own error message recommends, writes one); `secs_to_micros` range-checks instead of saturating; a negative `tbeg` is rejected. Cross-check: assert each perturbation actually changed the turn list (22 of 216 files are single-speaker, so `merge` currently self-scores), default `--tolerance` to 0, and extend md-eval coverage to `der-suite` pooling, `--skip-overlap` and `--uem`, none of which has ever been put in front of md-eval. Add a `--limit N` cross-check CI job or a committed expected-DER fixture — today, reverting either convention bug caught in PR #20 leaves `cargo test` fully green.

### H9 · A5 `multi` bucket: decide whether a speaker-labelled reference is produced
- Agent: — (owner decision) · Cmd: /phase0-validate · depends-on: —
- AC: `eval/raw/multi/` holds zero clips and `A5_SPRINT.md` instructs annotators **not** to write speaker labels or timestamps, so filling the bucket as specified can never yield a DER. Either the A5 protocol gains an RTTM reference for the `multi` clips, or ADR-0034's ban on speaker-accuracy claims becomes permanent rather than temporary. Until this is decided, H3–H6 ship best-effort with no accuracy claim.

---

## Cross-cutting (apply on every task)
- Run the PreToolUse/PostToolUse hooks (auto). Before any release: `/security-review` + `/tenant-check`.
- Add/adjust tests (server endpoints + non-trivial Rust logic). CI (.github/workflows/ci.yml) must be green.
- Update the relevant docs/ file and add an ADR when architecture/schema changes.
