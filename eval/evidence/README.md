# Phase 0 evidence manifest

`manifest.template.csv` defines the 20 planned, pseudonymous evaluation records. Make a local copy named `manifest.csv`, fill it only after the stated human actions occur, and run:

```powershell
pwsh -NoProfile -File tools/phase0/verify-evidence.ps1
```

`manifest.csv`, participant audio, draft/reference transcripts, reports and `eval/evidence/private/` are ignored by Git. Store the actual participant notice/permission record outside the repository in an access-controlled location; use only its opaque identifier in the manifest.

The verifier is fail-closed for file integrity and manifest completeness. It cannot determine whether permission was legally valid or whether a human truthfully performed the review.
