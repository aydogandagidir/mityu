# Diarization evaluation — the instrument, and why it can be trusted

> Scope: how Mityu measures speaker diarization quality, what conventions the
> numbers follow, and what has actually been verified. Decision context is
> `docs/DECISIONS.md` ADR-0034.

## Why an instrument came before an engine

`eval-harness/src/metrics.rs` scores WER, CER and jargon term-recall — nothing
about speakers. Before this existed the project had **no way to tell a good
speaker segmentation from a bad one**, so any engine choice would have been made
blind and its result unfalsifiable.

Waiting for the A5 `multi` bucket would not have helped: `docs/A5_SPRINT.md`
instructs annotators *not* to write speaker labels or timestamps, and
`eval-harness/src/main.rs` records that diarization is not auto-scored in an A5
run. Filling that bucket to 5/5 still yields no diarization number. A5 was never
a diarization instrument.

## Using it

Score one recording:

```bash
cargo run -p eval-harness -- der --reference ref.rttm --hypothesis hyp.rttm
```

Score a whole corpus (pairs files by name):

```bash
cargo run -p eval-harness -- der-suite --reference-dir refs/ --hypothesis-dir hyps/
```

Neither needs the `eval/` directory. Options: `--collar <s>`, `--skip-overlap`,
`--uem <file>`, `--score-everything`, `--file-id <id>`.

## What the number means

DER is the NIST `md-eval` formulation. On each elementary interval, with `R`
reference speakers active, `H` hypothesis speakers, and `C` of them joined by the
speaker mapping:

```
missed      = max(0, |R| - |H|)
false_alarm = max(0, |H| - |R|)
confusion   = min(|R|, |H|) - |C|
DER = Σ dur·(missed + false_alarm + confusion) / Σ dur·|R|
```

DER can exceed 100%: a system that invents speech everywhere is not capped at
"all wrong".

**The components are always reported separately.** The same 20% means very
different things when it is all missed speech (the segmenter is deaf) and when it
is all confusion (the clustering is wrong), and only the split says which part to
fix.

### Conventions, and why these ones

| Choice | Default | Why |
|---|---|---|
| Speaker mapping | Exact maximum-weight assignment (Hungarian) | Diarization labels are arbitrary. A system that segments perfectly but names speakers `X`/`Y` instead of `A`/`B` is **correct** and must score 0. Greedy pairing is the tempting shortcut and it silently **inflates** DER. |
| Mapping region | The evaluation region, **before** the collar | Deciding which speaker is which on the same region the errors are counted on optimises the mapping against the score, and yields a systematically lower DER than the literature. |
| Evaluation region | `[earliest reference start, latest reference end]` | md-eval's default (`uem_from_rttm`). Numbers computed any other way are not comparable with published ones. |
| Collar | `0` | pyannote publishes with 0. Defaulting to NIST's 0.25 s would make our numbers look better than the papers we compare against. |
| Overlap | Included | Excluding it always lowers DER, so such a number must never be compared against one that includes it. |
| Time arithmetic | Exact integer microseconds | The algorithm compares interval boundaries; in `f64`, `0.1 + 0.2 ≠ 0.3`, which leaves phantom slivers of "confusion" between turns that in fact abut. |
| Corpus aggregation | **Pooled**, not averaged | DER is a ratio of times and the mean of ratios is not the ratio of sums. One 30-minute file at 5% and one 10-second file at 100% is **5.5% pooled** and **52.5% averaged**. VoxConverse dev really does mix a 22-second file with a 20-minute one. The macro average is reported alongside, because a large gap means performance depends on file length. |

### Fail-closed behaviours

Each of these would otherwise produce a plausible number that means nothing:

- More than one `file_id` in an RTTM → error unless one is named.
- Reference and hypothesis naming **different** recordings → error. Two unrelated
  recordings whose timelines happen to line up would score a confident 0%.
- A reference with no speech → error. The denominator would be zero, and both
  "0%" and "100%" would be false statements about the system.
- A malformed line → error naming its line number, never skipped. Skipping
  shortens the reference and *improves* the score.
- A reference with no matching hypothesis in `der-suite` → scored as a total
  miss, never skipped, and warned about. Skipping would mean a system that
  crashes on its hardest files improves its corpus number by failing.
- Hypothesis speech outside the evaluation region → not scored (the convention),
  but **always reported** as `unscored hyp`. Under the default region that is
  exactly the speech a diarizer invented in a meeting's leading or trailing
  silence. Not counting it is not a reason to hide it.

## Validation against an independent implementation

Unit tests cannot catch a wrong *convention* — the same author wrote the code and
the tests, so they share whatever that author misunderstood. So the scorer is
checked against **NIST `md-eval.pl` v22**, the field's reference implementation,
on **216 real VoxConverse dev recordings**.

```bash
cargo build -p eval-harness
python tools/diarization/crosscheck-mdeval.py --collar 0.25
```

The tool downloads the corpus (CC-BY-4.0, pinned commit) and md-eval (pinned),
**verifies both by SHA-256**, then builds six non-trivial hypotheses per file —
relabel, jitter, merge, split, drop, inflate — and compares the two scorers.
Self-scoring a reference gives 0% and proves almost nothing; these break the
reference the way a real diarizer breaks it.

### Result (2026-08-08)

| Collar | Comparisons | Max &#124;ours − md-eval&#124; |
|---|---|---|
| 0.00 | 1296 | **0.0000 pp** |
| 0.25 | 1296 | **0.0000 pp** |
| 0.50 | 1296 | **0.0000 pp** |

### What it caught

Both of these passed every unit test and were found only by the cross-check.

1. **Evaluation region.** Hypothesis speech before the first reference turn or
   after the last was counted as false alarm here and not by md-eval. Verified
   from md-eval's source rather than inferred: `uem_from_rttm`
   (md-eval-22.pl:2245) returns `[min TBEG, max TEND]` over the reference, and
   line 626 installs it when no UEM is supplied.

2. **Mapping region.** The speaker mapping was decided on the post-collar scored
   region instead of the pre-collar evaluation region (md-eval-22.pl:1890-1908
   computes `$spkr_overlap` and calls `map_speakers` *before* `$uem_score` is
   built at :1913-1918). Ours was therefore systematically **lower** than
   md-eval's at collar > 0 — an instrument that flatters us.

A third apparent disagreement was **not** a defect in either implementation: the
`split` perturbation originally cut turns exactly in half, giving two hypothesis
speakers bit-identical overlap with the reference speaker (37.020 vs 37.020,
85.920 vs 85.920, …). That is a genuine tie in the assignment problem — both
mappings are optimal and the choice is implementation-defined. Splitting 45/55
instead removed the tie, and all eight vanished.

## What is still NOT verified

- **No diarization engine exists yet.** This measures; nothing has been measured.
  ADR-0034 step (c) is open, and the implementation approach is undecided.
- **No Turkish or in-person audio.** VoxConverse is English-language broadcast and
  YouTube material. Mityu's target — a Turkish meeting room, a noisy site visit —
  is not represented. The instrument is validated; the *engine* will still need
  target-condition evidence.
- **The A5 `multi` bucket holds zero recordings**, so no product claim about
  speaker accuracy may be made (ADR-0034).
- **Audio was never downloaded.** Only the annotations are used, which is all a
  scorer needs.
