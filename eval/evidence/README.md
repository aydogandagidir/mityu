# Phase 0 evidence manifest

`manifest.template.csv` defines the **25** planned, pseudonymous evaluation records — the jargon bucket carries two domains (`jl*` legal, `ji*` intralogistics) per ADR-0031. Make a local copy named `manifest.csv`, fill it only after the stated human actions occur, and run:

```powershell
pwsh -NoProfile -File tools/phase0/verify-evidence.ps1
```

If `pwsh` (PowerShell 7) is not installed, Windows PowerShell 5.1 runs it too — verified on 2026-08-07:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File tools/phase0/verify-evidence.ps1
```

Keep this script **pure ASCII**. It has no BOM, so 5.1 reads it as ANSI; a single non-ASCII character in a string (an em dash, say) is enough to make the whole file fail to parse.

`manifest.csv`, participant audio, draft/reference transcripts, reports and `eval/evidence/private/` are ignored by Git. Store the actual participant notice/permission record outside the repository in an access-controlled location; use only its opaque identifier in the manifest.

The verifier is fail-closed for file integrity and manifest completeness. It cannot determine whether permission was legally valid or whether a human truthfully performed the review.

## Withdrawal of consent (`consent_withdrawn_at_utc`)

KVKK m.7/m.11 lets a participant withdraw consent at any time. When that happens:

1. Delete **every file named after that clip**, in both `eval/<bucket>/` and `eval/raw/<bucket>/`. That is more than the recording: the original upload (`raw/<bucket>/<id>.<ext>`), the normalized `<id>.wav`, `<id>.draft.txt`, `<id>.ref.txt`, and one verbatim `<id>.<config>.hyp.txt` **per configuration** — six at the default config set. On Windows:

   ```powershell
   Remove-Item "eval\<bucket>\<id>.*", "eval\raw\<bucket>\<id>.*" -Force
   ```

2. Put the withdrawal timestamp (ISO-8601) in `consent_withdrawn_at_utc`. **Keep the row** — it is the record that the withdrawal was honoured.

The verifier then inverts its checks for that row: it requires that **no file bearing that clip id remains** under `eval/` (a rule rather than a filename list, so it cannot go stale as the harness gains new outputs), and stops counting the row toward the ≥5-per-bucket gate. Identity, schema and audit fields are still checked — a withdrawal record is itself an audit record. If that drops a bucket below five, verification **fails** — which is the intended behaviour. A gate must not be closed with a four-clip bucket because someone exercised a right; either record a replacement clip or report the bucket as incomplete.

Schema v2 (this template) adds that column. A v1 manifest is rejected rather than accepted, because it has no way to record a withdrawal.
