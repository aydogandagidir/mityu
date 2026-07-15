# Mityu v1.0.4 legal and product-owner sign-off pack

**Status (2026-07-16): product-owner sign-off provided under self-attestation ("öz-beyan"); NO independent legal review was obtained (ToS option 3a — residual risk accepted). Distributor/data-controller identity is verified and recorded (Blue Robot Teknolojileri ve Ticaret Ltd. Şti.). See "Required approvals" below.**

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

- Legal name / entity type: **Blue Robot Teknolojileri ve Ticaret Ltd. Şti.** — a registered Turkish limited company (Ltd. Şti.), trading under the name **"bluedev"**.
- Registered address: İçerenköy Mah. Topçu İbrahim Sk. Quick Tower Sitesi No: 8-10d, Ataşehir/İstanbul, Türkiye.
- Registration / tax identifier, if required: **VKN 1781857966** (Kozyatağı Vergi Dairesi); **MERSİS 0178185796600001**; **Ticaret Sicil No İstanbul-1125891**; Tel +90 530 721 0036.
- Privacy representative or contact: **info@bluedev.dev** (support: support@bluedev.dev).
- Confirm whether `bluedev` is a registered entity, trade name, or individual undertaking: **Registered entity** — "bluedev" is the **trade name of Blue Robot Teknolojileri ve Ticaret Ltd. Şti.** (registered limited company). Source: owner's `aydogandagidir/bluedev-vergi-asistani` `apps/web/lib/seller.ts@main`; product-owner-confirmed 2026-07-16.

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

- Operator: Aydoğan Dağıdır (product owner, Blue Robot Teknolojileri ve Ticaret Ltd. Şti.)
- Date/time: 2026-07-16
- Legacy keys deleted or migrated: **RETAINED (not deleted)** — `downloads:leads` (2 records), `downloads:emails` (2 items), `downloads:lead:v2:*` (0). Kept as records by product-owner decision. **No marketing or commercial email will be sent to these legacy contacts** until a separate İYS-registered, KVKK-consented opt-in basis is established; marketing use is out of v1.0.4 scope. No PII was read or copied into this repository (counts only).
- Evidence location / ticket (no PII): Product-owner decision recorded in the 2026-07-16 release session; retention basis + İYS/consent gating to be finalised before any marketing send.

### 4. Commercial and consumer terms

The current site advertises a Pro price, device/update entitlement, Business pricing and a 14-day money-back promise. Counsel/product owner must reconcile those claims with the Polar checkout and complete Section 8 of `TERMS_OF_SERVICE.md`, including seller identity, tax/MoR allocation, delivery, licence scope, support, refunds/withdrawal, termination, updates and durable pre-contract information.

The Turkish Ministry of Trade describes mandatory pre-contract information and distance-contract withdrawal rules; software/digital-content exceptions require specific treatment rather than an assumed blanket waiver: [official distance-contract guidance](https://tuketici.ticaret.gov.tr/yayinlar/tuketici-bilgi-rehberi/mesafeli-sozlesmeler-hakkinda-bilgilendirme).

- Final Pro offer approved: **USD 79 one-time, 2 devices, 1 year of updates** (ADR-0023) — product-owner-confirmed 2026-07-16.
- Final Business offer approved: **USD 59 / user / year** (ADR-0023) — product-owner-confirmed 2026-07-16.
- Polar seller/MoR and checkout terms verified: **Polar acts as merchant of record**; org id + $79 checkout wired (ADR-0023, MEMORY). Polar shows its own seller identity and handles buyer-side tax/withdrawal at checkout. Product-owner-confirmed.
- Refund/withdrawal implementation verified: **14-day money-back**; the consumer transaction/withdrawal is handled by Polar as MoR. Product-owner-confirmed.

### 5. Recording, AI, liability and governing law

Counsel must approve the participant-consent allocation, prohibited uses, warranty/liability cap, governing law/forum, enterprise use and employment/workplace implications. Product owner must confirm that the UI copy matches the approved allocation.

For EU distribution, assess the final role/classification and transparency wording against Article 50 and other applicable provisions of the [official EU AI Act](https://eur-lex.europa.eu/legal-content/EN/TXT/?uri=CELEX:32024R1689). Human-review labels and source links are engineering controls, not a legal classification decision.

**Product-owner record (2026-07-16, self-attestation):** Governing law and forum = **Türkiye; İstanbul (Çağlayan) Mahkemeleri and Enforcement Offices** (consistent with the bluedev group's other products, e.g. the vergi-asistani distance-sales terms). AI transparency: Mityu keeps the non-dismissable "AI-generated · review required" affordance and source links (HITL). The product owner confirms the UI copy matches this allocation. No independent legal classification was performed for the EU AI Act; EU-specific positioning remains subject to a later counsel review before any dedicated EU launch.

## Required approvals

### Legal reviewer

- Name / firm: **None — no independent legal review was obtained.** The product owner elected to publish under **self-attestation ("öz-beyan")** and to accept the residual risk (ToS option 3a). This line is recorded truthfully; no external counsel reviewed or approved v1.0.4.
- Jurisdictions and scope reviewed: N/A (no independent legal review).
- Approved files and commit SHA: N/A (no independent legal review).
- Required amendments completed: N/A
- v1.0.4 public distribution approved: N/A — approval provided by the product owner below under self-attestation, not by independent counsel.
- Date and signature / durable approval reference: N/A — waived by the product owner (ToS option 3a, 2026-07-16).

### Product owner / distributor

- Name and authority: **Aydoğan Dağıdır**, on behalf of **Blue Robot Teknolojileri ve Ticaret Ltd. Şti.** (distributor / data controller).
- Commercial offer and refund operations confirmed: **Yes** — Pro $79 / Business $59 / 14-day money-back / Polar merchant-of-record.
- Data-controller/processor facts confirmed: **Yes** — controller identity per §1; processors = Polar (licensing/MoR), Vercel (site/logs), Upstash/Vercel KV (rate-limit + aggregate count), user-chosen BYOK provider (optional).
- Residual risks accepted for v1.0.4: **Yes** — explicitly including: (a) Terms of Service published without independent legal review (self-attestation, ToS option 3a); (b) A5 real-audio validation and C8 human pilot deferred, not passed (ADR-0027); (c) no Windows Authenticode — installers show the SmartScreen unknown-publisher prompt (ADR-0029); (d) legacy leads RETAINED with marketing use deferred pending İYS/consent; (e) KVKK notice/Terms not yet updated by counsel.
- Approved commit SHA: **Mityu v1.0.4 release candidate on `codex/product-intelligence-v1.0.4`, finalised at this legal-record commit** (adds the identity/sign-off + user-facing identity block). The exact merge SHA is re-confirmed with the product owner at the merge gate.
- Date and signature / durable approval reference: **Aydoğan Dağıdır — 2026-07-16 — self-attestation ("öz-beyan").** Recorded from the product owner's explicit authorisation in the 2026-07-16 release session; no signature was fabricated.

Only after both approvals are complete may the release record change the legal/commercial gate to PASS and the draft warning in `TERMS_OF_SERVICE.md` be replaced with counsel-approved text.
