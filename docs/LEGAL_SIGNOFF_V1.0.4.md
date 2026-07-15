# Mityu v1.0.4 legal and product-owner sign-off pack

**Status (2026-07-15): engineering facts aligned; authorised legal approval still required.**

This document is a review packet, not legal advice and not an approval. An AI agent must not remove the draft warning from `TERMS_OF_SERVICE.md`, choose the distributor's legal identity, or accept commercial/legal risk on behalf of bluedev. The release remains fail-closed until the named authorised reviewers complete the decisions and signatures below.

## Frozen v1.0.4 product facts

- Production scope is Windows x64. macOS is excluded.
- Capture and transcription are local. Optional BYOK summarisation can send transcript text, never raw audio, to the provider selected by the user.
- v1.0.4 retains raw meeting audio in Mityu-managed local storage until the user deletes the meeting; it does not automatically delete audio immediately after transcription.
- A5 target-environment validation and the C8 human pilot are deferred/not passed for v1.0.4. No measured field/noise/jargon/diarization accuracy, SLA, or pilot-proven value claim is permitted.
- AI summaries and action items remain drafts until a person approves source-linked items.
- v1.0.4 embeds no PostHog project key; the analytics preference is a local no-op.
- The local database uses SQLCipher when the OS credential store supplies the key. The documented fresh/already-plaintext fallback remains; recordings and exports rely on OS/filesystem protection.
- Application-controlled deletion covers Mityu-managed database/FTS/WAL/recovery and recording artifacts, but cannot promise physical erasure from SSD wear levelling, COW, snapshots, backups, exports, swap, or residual WebView pages.
- The website download requires no account, contact details or marketing consent. When KV is enabled, the endpoint retains only an aggregate count plus a request counter under a server-secret HMAC token derived from the request IP. Its fixed window expires no later than 15 minutes after the first request and later requests do not extend it. v1.0.4 accepts no product-update signup or new lead record; contact collection remains disabled until email ownership can be verified and the legal/operational gates are approved.
- Licensing contacts Polar only for explicit activation/deactivation and at-most-weekly validation, using the license/activation identifiers and a random pseudonymous device label; meeting content is excluded.
- Built-in Qwen artifacts are Apache-2.0. Gemma artifacts remain subject to the Gemma Terms and Prohibited Use Policy. FFmpeg is distributed under the documented LGPLv3 configuration with corresponding-source/build/notices assets beside the binary dependency release.

## Engineering remediation completed

- [x] App privacy copy and website privacy copy describe the actual SQLCipher fallback, telemetry-off v1.0.4 posture, Polar fields, current raw-audio retention/deletion boundary, validation limitation, and optional cloud-provider path.
- [x] Download access requires no contact data or marketing consent; the unverified update-signup path is disabled.
- [x] The public endpoint rejects contact/marketing fields and has strict body/type/field validation, a honeypot, server-secret HMAC rate keys and a non-sliding atomic TTL. KV-enabled deployments fail closed until `DOWNLOAD_RATE_LIMIT_HMAC_SECRET` is configured.
- [x] Landing responses declare CSP, clickjacking, MIME-sniffing, referrer, browser-permission and HSTS protections.
- [x] Browser/API regression checks cover anonymous download, rejection of contact/marketing payloads, rate limiting, absence of lead writes, and request-metadata exclusion.
- [x] Model and FFmpeg notices are bundled and integrity-pinned.

## Decisions that only authorised people can close

### 1. Distributor and data-controller identity

- Legal name / entity type: ______________________________
- Registered address: ___________________________________
- Registration / tax identifier, if required: ___________
- Privacy representative or contact: _____________________
- Confirm whether `bluedev` is a registered entity, trade name, or individual undertaking: _____________________

The Turkish privacy notice must identify the controller and clearly state purpose, recipient groups, collection method/legal basis, and data-subject rights. The KVKK authority also states that a privacy notice and consent must be separate and that notice is not conditional on consent:

- [KVKK notice requirements](https://www.kvkk.gov.tr/Icerik/4132/aydinlatma-yukumlulugunun-yerine-getirilmesinde-uyulacak-usul-ve-esaslar-hakkinda-teblig)
- [KVKK 2026/347 notice/consent separation decision](https://www.kvkk.gov.tr/Icerik/8710/veri-sorumlulari-tarafindan-acik-riza-ve-aydinlatma-metinlerinin-ayri-ayri-duzenlenmesi-gerektigi-hakkinda-kisisel-verileri-koruma-kurulunun-18-02-2026-tarihli-ve-2026-347-sayili-ilke-kararina-iliskin-kamuoyu-duyurusu)

### 2. Website processing and international transfers

Counsel must confirm the exact legal basis and processor/transfer wording for each row:

| Processing | Proposed product purpose | Processor / destination | Retention | Approved basis / safeguards |
|---|---|---|---:|---|
| Vercel request/security logs | Deliver and protect the site/download | Vercel | Provider-defined short-lived logs | __________________ |
| HMAC-token rate-limit counter | Prevent automated endpoint abuse | Upstash/Vercel KV | Fixed window, at most 15 minutes from first request | __________________ |
| Aggregate download count | Capacity/product planning | Upstash/Vercel KV | Product-defined | __________________ |
| Optional update signup | Disabled in v1.0.4; no new contact collection | None | None | Product owner confirms disabled posture: __________ |
| Polar licensing | Activate and validate a paid licence | Polar | Confirm with Polar settings/terms | __________________ |
| User-selected BYOK summary | Perform the user's requested summary | Provider chosen by user | Provider-controlled | __________________ |

For an EU offering, validate the Article 13 disclosure set and any controller/processor or transfer obligations against the [official GDPR text](https://eur-lex.europa.eu/legal-content/EN/TXT/?uri=CELEX:32016R0679).

### 3. Existing lead-data migration

The v1.0.4 endpoint writes no new lead records. Before production promotion, an authorised operator must still inspect and either delete or lawfully retain/migrate legacy `downloads:lead:v2:*`, `downloads:leads` and `downloads:emails` keys created by earlier implementations. Record evidence without copying personal data into this repository:

Operational check (2026-07-15): the Mityu Vercel project has production/preview `KV_REST_API_*` variables, but they are configured as Vercel `sensitive` values and cannot be read back through the authenticated CLI/API. No PII was retrieved. The legacy-key count therefore remains unknown; an authorised operator must inspect it in the linked Upstash/Vercel storage dashboard (or provision a newly scoped audit credential) before production promotion.

- Operator: __________________
- Date/time: _________________
- Legacy keys deleted or migrated: __________________
- Evidence location / ticket (no PII): _______________

### 4. Commercial and consumer terms

The current site advertises a Pro price, device/update entitlement, Business pricing and a 14-day money-back promise. Counsel/product owner must reconcile those claims with the Polar checkout and complete Section 8 of `TERMS_OF_SERVICE.md`, including seller identity, tax/MoR allocation, delivery, licence scope, support, refunds/withdrawal, termination, updates and durable pre-contract information.

The Turkish Ministry of Trade describes mandatory pre-contract information and distance-contract withdrawal rules; software/digital-content exceptions require specific treatment rather than an assumed blanket waiver: [official distance-contract guidance](https://tuketici.ticaret.gov.tr/yayinlar/tuketici-bilgi-rehberi/mesafeli-sozlesmeler-hakkinda-bilgilendirme).

- Final Pro offer approved: ___________________________
- Final Business offer approved: ______________________
- Polar seller/MoR and checkout terms verified: _______
- Refund/withdrawal implementation verified: __________

### 5. Recording, AI, liability and governing law

Counsel must approve the participant-consent allocation, prohibited uses, warranty/liability cap, governing law/forum, enterprise use and employment/workplace implications. Product owner must confirm that the UI copy matches the approved allocation.

For EU distribution, assess the final role/classification and transparency wording against Article 50 and other applicable provisions of the [official EU AI Act](https://eur-lex.europa.eu/legal-content/EN/TXT/?uri=CELEX:32024R1689). Human-review labels and source links are engineering controls, not a legal classification decision.

## Required approvals

### Legal reviewer

- Name / firm: _______________________________________
- Jurisdictions and scope reviewed: ___________________
- Approved files and commit SHA: ______________________
- Required amendments completed: Yes / No
- v1.0.4 public distribution approved: Yes / No
- Date and signature / durable approval reference: _____

### Product owner / distributor

- Name and authority: _________________________________
- Commercial offer and refund operations confirmed: Yes / No
- Data-controller/processor facts confirmed: Yes / No
- Residual risks accepted for v1.0.4: Yes / No
- Approved commit SHA: ________________________________
- Date and signature / durable approval reference: _____

Only after both approvals are complete may the release record change the legal/commercial gate to PASS and the draft warning in `TERMS_OF_SERVICE.md` be replaced with counsel-approved text.
