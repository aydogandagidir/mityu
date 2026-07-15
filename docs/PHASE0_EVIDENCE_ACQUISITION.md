# Phase 0 evidence acquisition and C8 pilot closure pack

**Research date:** 2026-07-15
**Decision:** Internet datasets may be used only as a separately reported supplemental benchmark. They do **not** satisfy the Phase 0 `YOUR audio` gate, the human-corrected-reference requirement, or the C8 human pilot.

This document turns the remaining human work into a bounded collection protocol. It does not replace legal review and must not be used to claim that a human action occurred when it did not.

## 1. Internet research verdict

No official dataset found in the research combines all of the following:

- Turkish speech in five distinct 2–10 minute clips for each required bucket;
- real target-site machinery, wind, echo, or warehouse noise;
- at least three speakers with natural overlap;
- target-domain Turkish–English terminology;
- a human-corrected reference aligned to the audio;
- documented participant permission plus commercial evaluation/artifact-retention rights; and
- capture through Mityu's real device and audio path.

The closest official or primary-source candidates are below. `Supplemental` means the source may expose an engine weakness, but its results must never increment the canonical `eval/<bucket>` gate counters.

| Source | Useful coverage | Rights / provenance | Decision |
|---|---|---|---|
| [Common Voice Scripted Speech 26.0 — Turkish](https://mozilladatacollective.com/datasets/cmqinosfq00x4nr07gnk0rdf9) | Large Turkish read-speech set; useful for clean ASR regression | CC0; the prompt/audio match is community-validated, not a post-hoc verbatim correction of a natural meeting. Mozilla forbids speaker identification and dataset re-hosting. | Supplemental quiet regression only |
| [Common Voice Spontaneous Speech 4.0 — Turkish](https://mozilladatacollective.com/datasets/cmqi24rxo0046mf07yo82y3ng) | 52 short spontaneous clips from 12 speakers | CC0 with the same identity/re-hosting restrictions; the total validated material is too small and clips are too short for five independent target meetings. | Supplemental spontaneous regression only |
| [Google FLEURS](https://huggingface.co/datasets/google/fleurs) (`tr_tr`) | 16 kHz Turkish read speech with text fields | CC BY 4.0; utterances are short read sentences, not target meetings, field speech, overlap, or domain code-switching. | Supplemental quiet regression only |
| [METU Spoken Turkish Corpus](https://std.metu.edu.tr/en/) | Natural face-to-face, telephone, and media interactions with aligned transcripts | Access requires an agreement; use is non-commercial research and redistribution is forbidden. The [official FAQ](https://std.metu.edu.tr/en/sikca-sorulan-sorular/) describes participant consent for face-to-face/telephone material. | Do not commit or use for the commercial release gate without separate written permission |
| [IARPA Babel Turkish, LDC2016S10](https://catalog.ldc.upenn.edu/LDC2016S10) | Turkish telephone speech from varied environments with human transcripts | Paid/restricted LDC terms; mostly two-party, 8 kHz telephone audio and not Mityu capture. Commercial rights require the applicable LDC agreement. | Licensed supplemental field robustness only |
| [AMI Meeting Corpus](https://www.idiap.ch/webarchives/sites/www.amiproject.org/ami-scientific-portal/meeting-corpus/) | 100 hours of four-person meetings with orthographic transcripts | English; [OpenSLR's official resource page](https://www.openslr.org/16/) lists a modified CC BY-NC-SA 2.0 licence. | Supplemental overlap/meeting regression only; not Turkish and not commercial gate evidence |
| [MIMII](https://zenodo.org/records/3384388) | Factory machine sound at 16 kHz, CC BY-SA 4.0 | Contains machine/noise audio, not consented Turkish speech or transcripts. | May be mixed with separately consented speech for a synthetic stress test; never count as a real field clip |
| [DEMAND](https://zenodo.org/records/1227121) | Real environmental noise including office, meeting room, traffic, vehicle, metro and field recordings | The record's description specifies CC BY-SA 3.0; it contains no speech reference. | Synthetic stress test only; never count as a real field clip |
| [TEDH](https://doi.org/10.25592/uhhfdm.8222) | Restricted Turkish/English/German heritage-speaker interviews; strongest public lead for natural code-switching | Restricted access; commercial evaluation, redistribution and participant-permission scope are not granted on the public page. | Use only after written permission from the data owner; still supplemental |
| [TuGeBiC paper](https://arxiv.org/abs/2205.00868) | Manually prepared Turkish–German code-switching transcripts | The paper states that the original audio is no longer available. | Excluded from audio evaluation |

Additional public corpora may improve model research, but they cannot validate the actual Mityu microphone/system-audio path. They may also have appeared in model training data, so they are not guaranteed independent holdouts.

## 2. Canonical 20-recording plan

Use willing adults, non-sensitive role-play, and the real target acoustic setting. Do not record a customer meeting, confidential work, a moving vehicle, or an active hazardous zone. A prompt is an agenda, not a script: participants must speak naturally rather than read prepared sentences.

Target 3–5 minutes per recording (the accepted range remains 2–10 minutes).

| ID | Bucket | People | Safe natural-conversation prompt | Required characteristic |
|---|---|---:|---|---|
| `q01` | quiet | 2 | Plan a one-week implementation sprint | Natural turn-taking, dates and owners |
| `q02` | quiet | 2 | Review a fictional incident and preventive actions | Causes, uncertainty and corrections |
| `q03` | quiet | 1 | Give a spoken project handover | Continuous monologue, numbers and names invented for the test |
| `q04` | quiet | 2 | Compare two fictional procurement options | Trade-offs and a clear decision |
| `q05` | quiet | 2 | Review a fictional customer-support case | Questions, clarifications and action items |
| `f01` | field | 2 | Inspect a warehouse dock from an authorised safe position | Real echo and distant handling noise |
| `f02` | field | 2 | Discuss a conveyor/fan inspection from a designated safe area | Real steady machinery noise |
| `f03` | field | 2 | Conduct a safe outdoor site walk | Wind and changing microphone distance |
| `f04` | field | 3 | Perform a shift handover in a reverberant warehouse safe zone | Echo, background voices and three speakers |
| `f05` | field | 2 | Review a stationary service vehicle/equipment checklist | Intermittent noise; nobody drives during recording |
| `m01` | multi | 3 | Daily coordination meeting | At least two natural brief overlaps |
| `m02` | multi | 4 | Fictional incident retrospective | Disagreement, clarification and a decision |
| `m03` | multi | 3 | Design trade-off review | Interruptions without staged simultaneous reading |
| `m04` | multi | 4 | Shift handover and risk review | Short turns, names/roles and action ownership |
| `m05` | multi | 3 | Prioritise five backlog items | Ranking, voting and final actions |
| `j01` | jargon | 2 | WMS–WCS–ERP integration review | Turkish with natural English abbreviations and product terms |
| `j02` | jargon | 2 | PLC/HMI/sensor fault diagnosis | Use terms from `eval/jargon.txt` in meaningful context |
| `j03` | jargon | 3 | AS/RS, AGV and AMR layout review | Similar abbreviations, part names and code-switching |
| `j04` | jargon | 2 | OEE, throughput and cycle-time review | Numbers, units, percentages and English terms |
| `j05` | jargon | 2 | Predictive-maintenance and spare-part handover | At least 15 curated terms, naturally spoken |

Collection constraints:

- `quiet`: 1–2 speakers and a good microphone in a quiet room/office.
- `field`: 1–3 speakers and actual target-site noise. Playing a noise file through a speaker does not qualify.
- `multi`: at least 3 speakers and some natural overlap.
- `jargon`: Turkish-dominant speech with Turkish–English code-switching and target product/part names. Remove customer names and confidential codes from `eval/jargon.txt` before use.
- Use distinct recordings. One long session cut into arbitrary chunks is not five independent situations.

## 3. Permission and privacy evidence

Voice and verbatim transcripts can be personal data. Before recording, give participants a separate notice covering purpose, controller/contact, data categories, method, legal basis, recipients, retention/deletion, withdrawal/objection route and rights. If the chosen legal basis is consent, record it separately from the notice and make refusal consequence-free.

The separation is consistent with the [KVKK disclosure notice regulation](https://www.kvkk.gov.tr/Icerik/4132/aydinlatma-yukumlulugunun-yerine-getirilmesinde-uyulacak-usul-ve-esaslar-hakkinda-teblig) and the [KVKK Board's 2026 announcement on separate disclosure and explicit-consent texts](https://www.kvkk.gov.tr/Icerik/8710/veri-sorumlulari-tarafindan-acik-riza-ve-aydinlatma-metinlerinin-ayri-ayri-duzenlenmesi-gerektigi-hakkinda-kisisel-verileri-koruma-kurulunun-18-02-2026-tarihli-ve-2026-347-sayili-ilke-kararina-iliskin-kamuoyu-duyurusu). GDPR Articles 5–7, 13, 17 and 35 cover principles, consent evidence, transparency, erasure and DPIA respectively in the [official GDPR text](https://eur-lex.europa.eu/legal-content/EN/TXT/?uri=CELEX:32016R0679). The [EDPB consent guideline](https://www.edpb.europa.eu/our-work-tools/our-documents/guidelines/guidelines-052020-consent-under-regulation-2016679_en) explains why employment power imbalance can make consent non-freely given.

Operational minimum (not legal advice):

1. Use adults who volunteer and can refuse or stop without disadvantage.
2. Obtain counsel-approved notice/permission for the jurisdiction where recording occurs.
3. Keep names, signatures and contact details in an encrypted private evidence store outside Git.
4. Put only an opaque `consent_evidence_id` in `eval/evidence/manifest.csv`.
5. Define a deletion date and honour withdrawal before the permitted cut-off.
6. Do not upload raw audio/transcripts to GitHub, an issue, analytics, or a public benchmark.

## 4. Human-corrected reference workflow

1. Copy source files to `eval/raw/<bucket>/<id>.<ext>`.
2. Run `cargo run -p eval-harness -- prep`.
3. Run `cargo run -p eval-harness -- draft --engine whisper`.
4. A human listens to the complete audio at least once, corrects omissions/substitutions/insertions and saves `<id>.ref.txt`. Renaming the machine draft is not review.
5. The reference contains spoken words only. Do not add speaker labels, timestamps or `[overlap]` tokens: the WER/CER normaliser would count those as words. Put diarization/overlap observations in the human report section instead.
6. Preserve actually spoken fillers, repetitions, false starts and code-switches consistently. Do not “correct” the speaker's grammar or replace spoken wording with the meeting agenda.
7. A human reviewer records the approval timestamp and the audio/reference SHA-256 values in `eval/evidence/manifest.csv`.
8. Run `tools/phase0/verify-evidence.ps1`, then `cargo run -p eval-harness -- run`.
9. A human completes diarization/TTFT observations and records GO / CONDITIONAL / NO-GO. An AI may calculate metrics but may not sign this decision.

The local manifest template has exactly five planned rows per bucket. The verifier rejects public/synthetic sources, missing permission evidence, invalid durations/participant counts, absent files, empty references and hash mismatches. It verifies evidence integrity, not the truth or legal sufficiency of a person's permission.

## 5. C8 closure after Phase 0

C8 is not an audio-dataset benchmark. It verifies a real user's offline product journey and judgment against one immutable release candidate. After the Phase 0 human verdict:

1. Fix the exact v1.0.4 commit SHA; obtain green same-SHA CI and the signed Windows installer.
2. Have a real pilot user follow `docs/PILOT_V1.0.4.md` with the network disabled.
3. The human must inspect source links, edit/reject unsupported AI drafts, approve only supported items, test Action Center/search/export/delete, assess value and sign the result.
4. Store only content-free evidence references in the repo; keep participant/audio/transcript evidence in the approved private store.

Until those human actions occur, the correct evidence status remains. ADR-0027 makes both rows non-blocking for publication of v1.0.4 only; it does not convert either row to PASS:

- Phase 0/A5: **DEFERRED / NON-BLOCKING for v1.0.4 — NOT EVALUATED, 0/5 in each bucket**.
- C8: **DEFERRED / NON-BLOCKING for v1.0.4 — NOT PERFORMED on an immutable signed RC**.
