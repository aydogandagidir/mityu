---
description: Diagnose and fix a bug with a regression test, minimal blast radius.
argument-hint: <bug description or issue link>
---
Fix this bug: **$ARGUMENTS**

1. **Reproduce first.** Identify exact steps and the failing layer. If audio-related, use /audio-debug. Do not fix what you cannot reproduce — if you cannot, report what evidence is missing.
2. **Root cause, not symptom.** Read surrounding code; explain the cause in 2-3 sentences.
3. **Smallest safe fix.** Preserve local-first + tenant + HITL invariants. Prefer `anyhow` context / user-friendly frontend errors over silent catches.
4. **Add a regression test** proving the fix (and, if server tenant data was involved, a cross-tenant negative test).
5. **Verify Definition of Done** and run the relevant hook checks.
6. Note the branch convention: `fix/<slug>`.
