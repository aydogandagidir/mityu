# Mityu Terms of Service

*Last updated: 2026-07-14*

> **⚠ NOT LEGAL ADVICE · NOT REVIEWED BY COUNSEL · PUBLISHED UNDER SELF-ATTESTATION.** These Terms were drafted to reflect this codebase's actual behavior (local-first processing, recording-consent flow, HITL review, redaction) and follow common structure for BYOK desktop software. They have **not been reviewed by a lawyer**; the product owner has elected to publish them under self-attestation ("öz-beyan", 2026-07-16) and has accepted the residual risk — particularly the KVKK (Turkey) and potential GDPR (EU) considerations around recording third parties' voices. These Terms may be revised after a future legal review.

## 1. Acceptance of terms

By downloading, installing, or using Mityu ("the App", "the Service"), you agree to these Terms of Service ("Terms"). If you do not agree, do not use the App. These Terms apply alongside the [Privacy Policy](PRIVACY_POLICY.md) and the [MIT License](LICENSE.md) governing the underlying source code.

## 2. What Mityu is

Mityu is a local-first desktop application that records audio, transcribes it on-device (Whisper / NVIDIA Parakeet), and produces AI-assisted, human-reviewed summaries and action items. Capture and transcription run entirely on your device by default. Summarization may run locally or, at your choice, through a third-party LLM provider you configure with your own API key ("BYOK").

Mityu is an independent product of **bluedev** — the trade name of **Blue Robot Teknolojileri ve Ticaret Ltd. Şti.** (`bluedev.dev`). It is built on the open-source **Meetily** project by Zackriya Solutions (MIT license) but is **not affiliated with, endorsed by, or supported by Meetily or Zackriya Solutions**.

## 3. Your responsibility when recording others

**You are solely responsible for obtaining any consent required by law before recording a conversation.** Recording laws vary by jurisdiction (in Turkey, KVKK and Penal Code provisions on recording communications; other jurisdictions may require all-party consent). Mityu provides an in-app pre-recording consent prompt as a tool to help you meet this obligation, but it does not verify that you have actually obtained consent from every participant, and it cannot make that determination for you. You agree not to use Mityu to record any person without the legally required consent, and you accept full responsibility for how you use recordings and transcripts of other people.

## 4. AI-generated content is a draft, not a fact

Transcripts, summaries, and action items produced by Mityu — whether from local models or a BYOK provider — **may be incomplete, inaccurate, or misattributed**. Every summary and action item is marked as a draft requiring your review and approval before it is treated as final; Mityu does not publish or act on AI output without that human step. You are responsible for reviewing AI-generated content against the source recording/transcript before relying on it for any business, legal, or compliance purpose.

## 5. Third-party LLM providers (BYOK)

If you configure a third-party LLM provider (OpenAI, Anthropic, Groq, OpenRouter, or an OpenAI-compatible endpoint), you are contracting directly with that provider under its own terms of service and privacy policy, using your own API key. Mityu sends the minimum content needed for the requested operation directly from your device to the provider you selected; bluedev is not a party to that exchange and is not responsible for the provider's handling, pricing, availability, or output. Using a local model (Ollama, built-in on-device summarization) avoids sending content to any third party.

Built-in model files are downloaded separately at your request and remain subject to their own terms. Qwen 3.5 is licensed under Apache 2.0. Gemma 3 is subject to the [Gemma Terms of Use](https://ai.google.dev/gemma/terms), including the incorporated [Gemma Prohibited Use Policy](https://ai.google.dev/gemma/prohibited_use_policy); you must not use or redistribute Gemma contrary to those restrictions. Whisper and Parakeet notices are included in `MODEL-NOTICES.txt` with the distributed App. These model terms apply independently of Mityu's MIT-licensed source code.

## 6. Your data, your control

You own your recordings, transcripts, summaries, and action items. They are stored locally on your device. In v1.0.4 the raw recording is retained in Mityu-managed local storage until you delete the meeting; automatic deletion immediately after transcription is not implemented. The SQLite database uses SQLCipher when its operating-system credential-store key is available, with the limited fresh/plaintext local-first fallback described in the [Privacy Policy](PRIVACY_POLICY.md); recording and export files rely on operating-system and filesystem protection rather than application-level encryption. You may export, redact, or request deletion of Mityu-managed copies through the App. Application-controlled deletion does not guarantee forensic erasure from SSD wear-leveling, copy-on-write storage, snapshots, backups, exports, or other external copies. bluedev does not have access to your meeting content unless you explicitly send it to a provider, export it, or share it yourself. See the Privacy Policy for details on optional, off-by-default analytics.

## 7. Acceptable use

You agree not to: use Mityu to violate applicable law (including recording-consent and data-protection law); reverse-engineer or circumvent any license-activation, update-signing, or anti-tampering mechanism the App may use for paid distributions; redistribute a paid build of the App without authorization; or use the App to process content you do not have the right to record or process.

## 8. Licensing and payment (if applicable to your distribution)

The App's source code is licensed under the MIT License (see `LICENSE.md`) — you may build it yourself under those terms. If you obtained a **compiled, branded distribution** of Mityu through a paid channel, that purchase is governed by the specific license terms presented at time of purchase (e.g. per-device or per-seat activation); this section will be completed once bluedev finalizes its pricing/licensing mechanism (tracked in `docs/GA_READINESS.md`, Tier 3, item 18).

## 9. No warranty

THE APP IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED. bluedev does not warrant that transcription or summarization will be accurate, uninterrupted, or error-free, or that the App will be compatible with your hardware or use case.

## 10. Limitation of liability

To the maximum extent permitted by law, bluedev's total liability arising from your use of the App is limited to the amount you paid for it (or zero, if used free of charge). bluedev is not liable for indirect, incidental, or consequential damages, including any liability arising from your failure to obtain recording consent under Section 3.

## 11. Changes to these terms

We may update these Terms as the product evolves. Material changes will be noted via the same channels described in the Privacy Policy (repository updates, release notes, in-app notice for significant changes).

## 12. Governing law

These Terms are governed by the laws of the Republic of Türkiye, without regard to conflict-of-law principles, unless otherwise required by mandatory local consumer-protection law in your jurisdiction. *(To be confirmed by counsel — this may need to change depending on target markets and entity structure.)*

## 13. Provider / seller identity and contact

**Provider / seller (data controller):** **Blue Robot Teknolojileri ve Ticaret Ltd. Şti.** (trading as "bluedev")

- **Registered address**: İçerenköy Mah. Topçu İbrahim Sk. Quick Tower Sitesi No: 8-10d, Ataşehir/İstanbul, Türkiye
- **MERSİS No**: 0178185796600001 · **VKN**: 1781857966 (Kozyatağı Vergi Dairesi) · **Ticaret Sicil No**: İstanbul-1125891
- **Phone**: +90 530 721 0036
- **Email**: info@bluedev.dev · **Support**: support@bluedev.dev
- **Website**: [bluedev.dev](https://bluedev.dev)
- **GitHub Issues**: [Create an issue](https://github.com/aydogandagidir/mityu/issues)

Paid distributions are sold through **Polar** as merchant of record; Polar's own seller identity and terms also apply to the payment transaction shown at checkout.
