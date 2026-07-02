# PHASE 0 — Transcription Validation Protocol (make-or-break gate)

The entire product depends on turning real-world speech into a reliable transcript. This protocol is **mandatory and human-reviewed**. The agent builds the harness and runs it; a human reads the verdict before feature work proceeds. Do not self-approve past a failing threshold.

## Why this gate exists
whisper/Parakeet perform very differently on clean meeting-room audio vs noisy field audio (HVAC, machinery, forklifts, multiple overlapping speakers, accented technical jargon). If accuracy is unusable in the target environment, the product scope must narrow **before** money/time goes into features.

## 1. Build the evaluation set (YOUR audio, not public benchmarks)
Collect and label reference transcripts for at least these buckets (≥5 clips each, 2–10 min):
- **Q (quiet):** meeting room / office, 1–2 speakers, good mic.
- **F (field):** real on-site noise (machinery/echo/wind), 1–3 speakers.
- **M (multi):** 3+ speakers, some overlap.
- **J (jargon):** domain/technical terms and product/part names (and, for the target markets, **Turkish + English** and code-switching).

For each clip, create a ground-truth transcript (human-corrected). Store as `eval/<bucket>/<id>.wav` + `eval/<bucket>/<id>.ref.txt`.

## 2. Configurations to compare
- whisper.cpp `large-v3` (baseline) — with and without an initial **domain vocabulary / prompt**.
- Parakeet engine — with and without domain vocabulary.
- (If relevant) a Turkish-tuned setting for whisper.
Run each config over every clip; capture the hypothesis transcript and wall-clock latency (for streaming feel).

## 3. Metrics
- **WER** (word error rate) per clip and per bucket (primary). Also **CER** for Turkish (diacritics).
- **Term recall** on a curated jargon list (did the domain/part terms come through?).
- **Diarization sanity** (are speaker turns roughly right?) — qualitative in Phase 0.
- **Latency** (streaming): time from speech to on-screen text.

## 4. Go / No-Go thresholds (tune with the pilot, but decide up front)
Suggested starting bar (record the agreed numbers in DECISIONS.md):
- **GO (full scope incl. field):** median WER ≤ ~15% on Q, ≤ ~25% on F, jargon term-recall ≥ ~80% after vocabulary tuning.
- **CONDITIONAL (meeting-room scope only):** Q meets bar but F does not → ship for Q environments; defer field until improved (better mic/lapel, VAD, per-project vocabulary).
- **NO-GO (rethink):** even Q WER is unusable after tuning → the base STT is not fit; reassess engines/approach before building.
These are starting points, not physics; the pilot refines them.

## 5. Harness (starter — the agent completes it)
Create `eval/run_eval.py` (or a Rust bin) that: iterates clips, invokes each engine (via the app's transcription path or the engine binaries directly), writes hypotheses, computes WER/CER/term-recall, and emits `eval/report.md` + `eval/report.json`.

```python
# eval/run_eval.py  (skeleton — fill in engine invocation for whisper.cpp & Parakeet)
import json, glob, subprocess, statistics
from pathlib import Path
try:
    from jiwer import wer, cer          # pip install jiwer
except ImportError:
    raise SystemExit("pip install jiwer")

CONFIGS = {
    "whisper_large_v3":        {"engine": "whisper", "vocab": False},
    "whisper_large_v3_vocab":  {"engine": "whisper", "vocab": True},
    "parakeet":                {"engine": "parakeet", "vocab": False},
    "parakeet_vocab":          {"engine": "parakeet", "vocab": True},
}
JARGON = [t.strip() for t in Path("eval/jargon.txt").read_text(encoding="utf-8").splitlines() if t.strip()]

def transcribe(wav: str, cfg: dict) -> str:
    # TODO: call whisper.cpp / Parakeet (CLI or the app command) with/without vocab; return hypothesis text.
    raise NotImplementedError

def term_recall(hyp: str, ref: str) -> float:
    h = hyp.lower()
    present = [t for t in JARGON if t.lower() in ref.lower()]
    if not present: return 1.0
    hit = sum(1 for t in present if t.lower() in h)
    return hit / len(present)

rows, per_bucket = [], {}
for ref_path in glob.glob("eval/*/*.ref.txt"):
    p = Path(ref_path); bucket = p.parent.name; wav = str(p.with_suffix("")).replace(".ref","") + ".wav"
    ref = p.read_text(encoding="utf-8")
    for name, cfg in CONFIGS.items():
        hyp = transcribe(wav, cfg)
        r = {"clip": p.stem, "bucket": bucket, "config": name,
             "wer": round(wer(ref, hyp), 4), "cer": round(cer(ref, hyp), 4),
             "term_recall": round(term_recall(hyp, ref), 3)}
        rows.append(r); per_bucket.setdefault((name, bucket), []).append(r["wer"])

summary = {f"{n}|{b}": round(statistics.median(v), 4) for (n, b), v in per_bucket.items()}
Path("eval/report.json").write_text(json.dumps({"rows": rows, "median_wer": summary}, indent=2, ensure_ascii=False))
lines = ["# Phase 0 Transcription Report", "", "## Median WER by config|bucket"]
lines += [f"- {k}: {v}" for k, v in sorted(summary.items())]
Path("eval/report.md").write_text("\n".join(lines), encoding="utf-8")
print("Wrote eval/report.md and eval/report.json")
```

## 6. Deliverable of this gate
- `eval/report.md` + `eval/report.json` with per-bucket WER/CER, jargon term-recall, latency.
- A one-paragraph **verdict**: GO / CONDITIONAL(meeting-room) / NO-GO, the chosen thresholds, and the recommended STT config (engine + vocabulary) to standardize on.
- Record the verdict + thresholds in DECISIONS.md. Only then does the agent enter field-dependent features (BACKLOG EPIC C field items).

## 7. Harness usage (Rust bin)

The implemented harness is the workspace bin **`eval-harness/`**. It links the app core
(`frontend/src-tauri`, lib `app_lib`) and drives the app's **own** engines — whisper via
`whisper-rs` and Parakeet via `ort` — with no Tauri app running. External whisper CLIs /
pip packages are never used. The Python skeleton in §5 stays as historical reference only.

Windows build env (Git Bash), from the repo root:

```bash
export PATH="/c/Program Files/CMake/bin:/c/Program Files/LLVM/bin:$PATH"
export LIBCLANG_PATH="C:\\Program Files\\LLVM\\bin"
```

Flow:
1. Put recordings under `eval/raw/<bucket>/` (see `eval/README.md`), then
   `cargo run -p eval-harness -- prep`
   → `eval/<bucket>/<id>.wav` (16 kHz mono s16, converted with the app's ffmpeg sidecar).
2. `cargo run -p eval-harness -- draft [--engine whisper|parakeet] [--model <name-or-ggml-path>]`
   → `<id>.draft.txt` for every clip that has no reference yet.
3. **Human** corrects each draft and saves it as `<id>.ref.txt` (mandatory human-verified reference).
4. `cargo run -p eval-harness -- run [--configs whisper_large_v3,whisper_large_v3_vocab,whisper_large_v3_turbo,whisper_large_v3_turbo_vocab,parakeet,parakeet_vocab] [--quick N] [--model <name-or-path>] [--model-turbo <name-or-path>]`
   → `eval/report.json` + `eval/report.md` (per-clip rows, medians per config|bucket, §4
   threshold check pre-filled with computed numbers; the verdict line is filled by a human).
   All six configs above are the default set.

Details:
- Optional per clip: `<id>.lang.txt` containing `tr` or `en` (default: whisper auto-detect;
  Parakeet v3 is multilingual and takes no language hint).
- Whisper model resolution is per config pair: `whisper_large_v3(_vocab)` resolves `large-v3`
  (`ggml-large-v3.bin`, override with `--model`); `whisper_large_v3_turbo(_vocab)` resolves
  `large-v3-turbo` (`ggml-large-v3-turbo.bin`, override with `--model-turbo`). Auto-discovery in
  the app model dirs (`%APPDATA%\com.bluedev.mityu\models`, `%APPDATA%\Mityu\models`, repo
  fallbacks). Missing `large-v3` aborts with the in-app download instruction (Settings →
  Transcription, Turkish) and a nonzero exit; a missing/incomplete `large-v3-turbo` does NOT
  abort — the turbo configs are skipped and report.md carries an "ATLANDI" note. Only one whisper
  model is held in memory at a time (configs run grouped per model). Parakeet reuses the
  app-downloaded `parakeet-tdt-0.6b-v3-int8`.
- Vocab configs: whisper gets an `initial_prompt` built from `eval/jargon.txt` (capped to the
  whisper prompt budget). The app's Parakeet integration has **no** hotword/vocab biasing, so
  `parakeet_vocab` runs plain and the report notes it.
- Metrics: strict **and** diacritic-folded WER/CER (Turkish-aware normalization: NFC, `I`→`ı` /
  `İ`→`i`, apostrophes removed, punctuation→space); term recall uses folded substring matching;
  wall-clock secs + RTF per clip.
