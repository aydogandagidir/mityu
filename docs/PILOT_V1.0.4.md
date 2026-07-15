# Mityu v1.0.4 offline pilot protocol

**Gate:** C8 — human-reviewed; one real pilot user must complete the flow. An AI agent may prepare the build, models, checks and evidence template, but must not impersonate the reviewer or approve AI output on the user's behalf.

**v1.0.4 status (ADR-0027):** `DEFERRED / NON-BLOCKING — NOT PERFORMED`. This publication-only exception is not a C8 PASS and does not unlock any C8-dependent roadmap work. The protocol below remains the closure requirement when the gate resumes.

## Entry criteria

- Phase 0 has a human-recorded GO or CONDITIONAL verdict in `docs/DECISIONS.md`.
- Candidate version and commit SHA are fixed and all required CI checks are green.
- Windows x64 candidate is signed with the production Authenticode identity and Tauri updater key.
- Whisper `large-v3` and the selected local summary model are downloaded and integrity-verified.
- The pilot user and every recorded participant have given the required consent.
- No transcript, participant identity, audio, API key or unredacted screenshot is committed to the repository.

## Evidence header

| Field | Value |
|---|---|
| Pilot user (name or internal ID) | |
| Reviewer distinct from AI agent | |
| Date/time/time zone | |
| Device / Windows version | |
| Mityu version | 1.0.4 |
| Commit SHA | |
| Installer SHA-256 | |
| Authenticode signer / timestamp | |
| STT model and verified hash | |
| Summary model and verified hash | |
| Offline method and evidence | |
| Consent evidence location (no participant PII here) | |

## Required flow

Record PASS/FAIL and a content-free evidence reference for every row.

| # | Human action / expected result | PASS/FAIL | Evidence / defect |
|---:|---|:---:|---|
| 1 | Install the reviewed candidate, launch it once, select local STT and local summary models, then disconnect network access. Restart Mityu; it remains usable. | | |
| 2 | Start a new consented 5–10 minute real meeting recording with microphone and, where relevant, Windows system audio. The consent gate appears before capture. | | |
| 3 | Speak naturally. Record speech-to-first-visible-text observations; transcript segments appear without network access. No content is visible in production logs. | | |
| 4 | Stop recording. The meeting, source timestamps and local recording/transcript artifacts persist; restarting offline does not lose segments. | | |
| 5 | Generate a summary with the local model. Every block/action is visibly AI-generated and remains Draft; no item is silently published or added to the approved-only Action Center. | | |
| 6 | Open at least two source links and compare the draft with the exact timestamped transcript/audio. Edit at least one draft item and reject at least one incorrect/irrelevant item. | | |
| 7 | The human reviewer approves only the remaining supported blocks/items. Approval refuses missing/stale source evidence and no concurrent edit is overwritten. | | |
| 8 | Open Action Center. Only the approved active action appears. Its source jump opens the same meeting and exact transcript segment; drafts/edited/rejected items remain absent. | | |
| 9 | Use Evidence Search for a term spoken in the pilot. A ranked result opens and highlights the exact source segment while offline. | | |
| 10 | Export Markdown, PDF and DOCX. Each exported approved AI item includes its source timestamp/link; no unapproved item is represented as final. | | |
| 11 | Create a disposable second meeting, then delete it. Mityu reports success only after its managed DB/FTS/WAL/recovery/recording maintenance completes; the main pilot meeting remains intact. | | |
| 12 | Reconnect the network only after the offline flow is complete. Confirm no unexpected transcript/audio request was made and record any optional provider/update requests separately. | | |

## Human quality observations

- Median/representative live UI TTFT observed: __________________
- Multi-speaker / diarization sanity (PASS / FAIL / N/A): ______
- Transcript corrections that materially changed meaning: ______
- Unsupported/hallucinated draft items found: ___________________
- Source-link accuracy: ________________________________________
- Export provenance accuracy: _________________________________
- Product value achieved by the pilot user: ____________________

## Exit decision

- [ ] All required rows passed, or every exception has an accepted release-blocking decision.
- [ ] Phase 0 verdict and selected STT configuration are recorded in `docs/DECISIONS.md`.
- [ ] No sensitive pilot content was committed or attached to a public issue.
- [ ] Security/privacy and tenant-isolation audits remain green on the same candidate SHA.

**Pilot verdict (GO / CONDITIONAL / NO-GO):** __________________

**Human reviewer:** __________________  **Date:** ______________

**Product owner acceptance:** __________  **Date:** ____________

The C8 gate may be marked PASS only after these human fields are completed against the exact release candidate SHA.
