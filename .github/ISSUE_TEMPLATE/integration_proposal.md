---
name: Integration / third-party dependency proposal
about: Propose adding a third-party crate, library, model, or sidecar binary to Mityu
title: '[INTEGRATION] '
labels: enhancement, dependency
assignees: ''
---

<!--
Open this BEFORE writing code. Mityu is a local-first, commercially licensed
desktop app, so a new dependency is a licence, privacy and maintenance decision
before it is a technical one — and the answers below decide whether a PR is
reviewable at all. A proposal with these filled in can be answered in a day;
one without them cannot.

Read CLAUDE.md (especially §0 Prime Directives, §4 the audio pipeline, §9
licensing) and docs/DECISIONS.md for the decisions already made in this area.
-->

## What problem does this solve?
[The user-visible gap. Link the issue or the ADR that records it, if one exists.]

## What are you proposing to add?
[Name, version, repository URL, and what it would replace or fill in.]

## Which part of Mityu does it touch?
- [ ] Audio capture / mixing / normalization (**highest-risk subsystem — see CLAUDE.md §4**)
- [ ] Transcription
- [ ] Summarization / LLM providers
- [ ] Local storage / database
- [ ] UI
- [ ] Build, packaging, or CI
- [ ] Other: [describe]

## Licensing
- Licence of the dependency itself:
- Licence of its **transitive** tree (any GPL / AGPL / LGPL / SSPL anywhere?):
- Does `cargo deny check` pass without adding an `exceptions` or `allow-git`
  entry to `deny.toml`? [yes / no — if no, say which entry and why]
- If it ships or downloads a **binary or model**: licence of that artefact, and
  how its integrity is verified (Mityu pins URL + byte length + SHA-256; see
  `tools/diarization/fetch-sherpa-archive.py` and ADR-0035).

## Local-first and privacy
- Does anything in the path require the network at runtime? [The core
  capture → transcript → summary → store path must work fully offline —
  CLAUDE.md §0.1]
- Does it send any audio, transcript, or user content anywhere by default?
  [It must not — CLAUDE.md §10]
- Any feature that can be built without network access but ships enabled with
  it? [Name it; it needs to be off by default and consent-gated — ADR-0018]

## Platform scope
- Which platforms does it support, and which does Mityu already support there?
- Does it change behaviour on a platform that currently works? [macOS and
  Windows recording paths are load-bearing; a change there is not a side effect]

## Maturity and maintenance
- Age, release history, number of maintainers, downstream users:
- What happens to Mityu if it is abandoned or breaks?
- Is there a first-party alternative (an OS API we could call directly)?

## Evidence you can provide
[We cannot merge what we cannot verify. For anything in the audio path,
CLAUDE.md §4 requires a manual record → transcript smoke test on real hardware.
Say what you can demonstrate, on which OS and hardware, and how — logs, the
produced transcript, a recording. If you cannot demonstrate it, say that
plainly; it is a useful answer.]

## Scope you are proposing for the first PR
[Smallest reviewable change. Behind a feature flag, off by default, is almost
always the right first step — see CLAUDE.md §0.8.]

## Checklist
- [ ] I searched existing issues and `docs/DECISIONS.md` for prior decisions here
- [ ] I read CLAUDE.md §0, §4 (if audio) and §9
- [ ] I named the licence of the dependency **and** its transitive tree
- [ ] I stated what I can demonstrate and on what hardware
- [ ] I understand contributions are signed off under the DCO (see CONTRIBUTING.md)
