# Security & Privacy (architectural requirements)

Security and privacy are the product's differentiator (local-first vs cloud recorders). They are requirements, not features.

## Secrets
- LLM/API keys: **OS keychain / Tauri secure store** only. Never in SQLite plaintext, source, logs, analytics, or git. (`guard-secrets.sh` hook blocks a first pass; `security-privacy-auditor` reviews.)
- Signing keys / server credentials: CI secrets or a vault; never committed.

## Encryption
- **At rest (client):** SQLCipher (encrypted SQLite) for sensitive data; OS-protected key.
- **At rest (server, Phase 2+):** Postgres field-level encryption (pgcrypto) for sensitive columns; per-tenant KMS key seam.
- **In transit:** TLS everywhere the sync client talks to the server.

## AuthN/Z & tenant isolation (server)
- OIDC only; RBAC on every sensitive route; RLS + repository tenant scoping (see MULTITENANCY.md); mandatory negative cross-tenant test.

## LLM-specific risks (OWASP LLM Top 10)
- **Prompt injection (LLM01):** transcript/screen text is untrusted input to the LLM. Treat all LLM output as an **untrusted draft**; never let it trigger actions without human approval; keep a strict tool allowlist if/when tools are added.
- **Sensitive info disclosure (LLM02/06):** never send content to analytics; scope RAG/context to the tenant; redact per policy.
- **Insecure output handling:** render summaries as data, not executable content.

## Privacy & data lifecycle
- **Consent:** explicit, visible "recording active" indicator; capture consent where required; multi-party consent guidance for meetings.
- **Analytics:** PostHog is **opt-in** and content-free (no transcripts/meeting text). Disableable for enterprise.
- **Retention/redaction:** per-tenant retention (default: delete audio after transcription); redaction rules for PII/sensitive terms.
- **Deletion & portability:** working account+data deletion and export.
- **Audit:** append-only audit trail on sensitive actions (server).

## Compliance mapping (technical/operational, not legal advice)
- **KVKK / GDPR:** lawful basis + consent, data minimization, EU data residency option, DPA/subprocessor transparency, deletion/portability, DPIA for systematic recording.
- **EU AI Act:** Article 50 transparency obligations are **live from 2 Aug 2026** → disclose AI interaction and **label AI-generated content**; keep the "AI-generated, review required" affordance. Position as a **limited-risk, human-approved documentation assistant**: do **NOT** implement workplace emotion recognition (prohibited) or automated employment/performance decisions (high-risk, Annex III, deadline 2 Dec 2027). Speaker identity from voice can implicate biometric categorization — keep speaker naming manual, avoid automated biometric identity claims.
