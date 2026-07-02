---
description: Run the Phase 0 transcription validation gate and produce a go/no-go verdict.
---
Execute docs/PHASE0_VALIDATION.md exactly (delegate to audio-pipeline-engineer + qa-release-engineer):

1. Confirm the eval set exists (eval/<bucket>/<id>.wav + .ref.txt for quiet/field/multi/jargon) and eval/jargon.txt. If missing, tell the user precisely what audio to record — do NOT fabricate data or use only public benchmarks.
2. Complete eval/run_eval.py (wire whisper.cpp `large-v3` and Parakeet, with/without domain vocabulary). Compute WER/CER, jargon term-recall, and streaming latency.
3. Produce eval/report.md + eval/report.json.
4. Compare against the agreed thresholds; output the verdict: GO / CONDITIONAL(meeting-room) / NO-GO, plus the recommended STT config.
5. Record the verdict + thresholds in docs/DECISIONS.md.
This is a **human-reviewed gate**: present the verdict; do not self-approve past a failing WER, and do not enter field-dependent features on a NO-GO.
