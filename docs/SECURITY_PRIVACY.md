# Security & Privacy (architectural requirements)

Security and privacy are the product's differentiator (local-first vs cloud recorders). They are requirements, not features.

## Secrets
- LLM/API keys: **OS keychain / Tauri secure store** only. Never in SQLite plaintext, source, logs, analytics, or git. (`guard-secrets.sh` hook blocks a first pass; `security-privacy-auditor` reviews.)
- Signing keys / server credentials: CI secrets or a vault; never committed.

## Encryption
- **At rest (client):** SQLCipher (encrypted SQLite), AES-256 (BACKLOG B3, ADR-0014, implemented). The whole `meeting_minutes.sqlite` file is encrypted; the 256-bit key is a **device-scoped** entry (`db-key`) in the OS credential store (`secrets::db`, service `com.bluedev.mityu`), generated from the OS CSPRNG on first launch, never in SQLite/source/logs. The pool opens keyed via the reserved SQLCipher `key` pragma (executed first) with WAL preserved. **Fail closed:** a locked/unavailable keychain aborts opening the DB — never an unencrypted fallback.
  - **Migration path (existing users):** on the first launch after upgrade, a plaintext DB is detected by its `SQLite format 3` header and converted **once** *before* migrations run: any plaintext WAL is checkpointed into the main file, then `sqlcipher_export` writes a new cipher file (rollback-journal mode, single self-contained file), an atomic swap moves the plaintext original to a **temporary** `meeting_minutes.sqlite.pre-encryption` backup and promotes the cipher file, and the new encrypted DB is **verified to open with the key**. Only after that verification is the `.pre-encryption` backup **deleted** (scrub-then-unlink) along with any stale plaintext `-wal`/`-shm`, so **no full-database plaintext lingers on disk**. The `_sqlx_migrations` ledger and every row are preserved intact. If verification fails the backup is restored and startup aborts (fail closed, recoverable). Conversion is idempotent and safe to interrupt. The legacy `.db`→`.sqlite` import is encrypted by the same path.
  - **Key-loss = unrecoverable (by design):** because the plaintext backup is deleted after conversion and the key lives **only** in the OS keychain, removing/rotating the `db-key` entry — or moving the file to another machine without it — makes the DB **permanently unreadable**. This is the intended at-rest posture (an attacker with the file but not the keychain gets nothing). There is deliberately **no** persistent plaintext fallback. A user-facing key export/rekey affordance (so a user can back up or migrate their key) is tracked as follow-up work in ADR-0014; until it ships, protecting the OS keychain account is the recovery story.
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
