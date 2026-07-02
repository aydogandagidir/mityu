---
description: Run a static security & privacy review and map findings to OWASP and KVKK/GDPR/EU AI Act. Read-only.
argument-hint: [optional path or area to focus]
---
Run a security & privacy review (delegate to security-privacy-auditor). Focus: ${ARGUMENTS:-the whole change set}.

1. **Secrets:** scan source + `git log -p` for hardcoded API keys/tokens/private keys; confirm LLM keys live in OS keychain / Tauri store, not SQLite plaintext, logs, analytics, or git.
2. **Encryption:** at rest (client SQLite via SQLCipher for sensitive data; server Postgres field encryption) + TLS in transit + key handling.
3. **Server auth/z:** OIDC validation, RBAC on every sensitive route, no god-mode, no cross-tenant; RLS present and covered by a negative test.
4. **LLM (OWASP LLM Top 10):** prompt injection via transcript/screen content, sensitive-info disclosure, insecure output handling, tool/action abuse — confirm AI output is treated as an untrusted draft gated by HITL.
5. **Privacy:** analytics carries no content; retention/redaction enforced; account+data deletion works; consent captured; audit trail present.
6. **Compliance mapping:** KVKK/GDPR (consent, minimization, EU residency, DPA, deletion) and **EU AI Act Art. 50** (AI disclosure + AI-generated labeling, live 2 Aug 2026); confirm NO workplace emotion recognition and NO automated employment decisions.

Output a risk table: Risk | Component | Severity | Evidence(file:line) | OWASP/legal | Recommended control. Recommend fixes; do not edit code.
