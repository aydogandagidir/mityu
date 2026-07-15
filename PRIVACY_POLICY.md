# Mityu Privacy Policy

*Last updated: 2026-07-15*

## Our Privacy-First Commitment

Mityu is built on the principle that your meeting data should remain private and under your control. This privacy policy explains how we handle data in our local-first meeting assistant.

## Data Processing Philosophy

### Local-First Processing
- **Meeting transcription**: Processed entirely on your device using local Whisper or Parakeet models
- **Audio recordings**: Never transmitted to external servers
- **v1.0.4 raw-audio retention**: The raw recording remains in Mityu-managed local storage until you delete the meeting. Automatic deletion immediately after transcription is not implemented in this version.
- **Meeting content**: Remains on your device unless you explicitly choose a third-party LLM provider or export/share it
- **AI summaries**: Generated locally or through the LLM provider you choose

### Your Data Ownership
- You own all meeting data, transcripts, and recordings
- Data is stored locally on your device
- No vendor lock-in - export your data anytime
- Control over application-managed retention, export, and deletion, subject to the storage limitations described below

## Website Download

Downloading the installer requires no account, name, email address, or marketing consent. The Mityu website is hosted by Vercel, which may process ordinary request/security logs under its service terms. The download endpoint keeps an aggregate counter. For abuse prevention it also derives a keyed HMAC token from the request IP address using a server-only secret and retains only a request counter under that token. The token expires no later than 15 minutes after the first request; later requests do not extend the window. The KV rate-limit record contains neither the IP address nor the HMAC secret, and the endpoint does not store the user agent, referrer, meeting content, audio, or contact details.

v1.0.4 does not offer or store a product-update signup. Contact collection remains disabled until an email-ownership verification flow, retention/erasure operations, processor terms, transfer safeguards, and the required legal/product approvals are in place.

## Usage Analytics

### What We Collect
Usage analytics is optional and off by default. When you choose to enable it, Mityu collects minimal, pseudonymous usage data:

**Application Usage:**
- Feature usage patterns (which tools you use most)
- Session duration and frequency
- Performance metrics (transcription success rates, error frequencies)
- UI interaction patterns (button clicks, navigation flows)

**Technical Metrics (only if a future production build enables the optional processor):**
- Application version and platform information
- Content-free error occurrence categories; Mityu does not upload diagnostic logs or crash dumps
- Performance benchmarks (processing times, resource usage)

### What We DON'T Collect
We never collect:
- ❌ Meeting content, transcripts, or recordings
- ❌ Names, email addresses, or contact information entered as analytics identity
- ❌ File names, meeting titles, or metadata
- ❌ Audio data or voice patterns
- ❌ Participant names or contact information
- ❌ LLM conversations or AI-generated content

### Why We Collect This Data
When enabled, analytics helps us with:
- **Product Quality**: Identifying and fixing bugs that impact user experience
- **Performance Optimization**: Understanding resource usage and system bottlenecks
- **Security**: Detecting potential security issues and vulnerabilities
- **Feature Development**: Making data-driven decisions about new features
- **Open Source Sustainability**: Ensuring the project meets user needs effectively

### Analytics Implementation
- **Provider**: PostHog (privacy-focused analytics platform)
- **Default**: Off by default; analytics starts only after you enable it in settings
- **v1.0.4 production posture**: The release workflow intentionally embeds no PostHog project key, so analytics remains a local no-op even after opt-in. A later release may enable the processor only after the region, retention, deletion/erasure process, access controls, DPA/subprocessor terms, and this notice are approved.
- **Pseudonymous identifier**: Events are linked to a generated installation/user ID rather than a name or email address
- **Data retention**: The desktop application does not enforce PostHog-side retention. The active PostHog project configuration and provider policy control retention and must be verified by the distributor.
- **Encryption**: All data encrypted in transit using industry-standard protocols
- **Location**: No PostHog processing occurs in v1.0.4. Before a future build enables it, the selected processing region and cross-border transfer basis must be disclosed here.
- **Access Control**: Strictly limited to core development team members

## Third-Party Services

### LLM Providers (Optional, BYOK)
If you choose to use external LLM providers with your own API key:
- **Anthropic Claude**: Subject to Anthropic's privacy policy
- **OpenAI** (and OpenAI-compatible endpoints): Subject to the provider's privacy policy
- **Groq**: Subject to Groq's privacy policy
- **OpenRouter**: Subject to OpenRouter's privacy policy
- **Ollama**: Processed entirely on your device only when configured to an exact loopback endpoint. If you deliberately configure a remote HTTPS Ollama-compatible endpoint, meeting content is sent to that endpoint under its operator's terms.

Newly configured provider API keys are stored in your operating system's secure credential store (keychain) and are transmitted only to the provider you choose. Legacy plaintext database fields are scrubbed during startup migration. When the credential store accepts the key, the database keeps only a non-secret marker; if the store is unavailable, the plaintext is removed and you must enter the key again.

### Licensing (Polar)
If you activate, deactivate, or periodically validate a paid license, Mityu contacts `api.polar.sh`. The request contains the license key, public organization ID, Polar activation ID, and a random pseudonymous device label. Mityu does not send your hostname or meeting content. Licensed installations validate in the background at most once every seven days; network failure does not disable an existing license.

### Models and Updates
Model downloads are started by you and fetch only pinned model artifacts from Hugging Face; no meeting content is uploaded. The app may check the Mityu GitHub release endpoint for signed update metadata. Downloaded updater artifacts are accepted only after signature verification.

### Analytics Service (Optional)
- **PostHog**: Used for usage analytics when enabled
- **Data**: Only pseudonymous usage patterns, no meeting content
- **Control**: Completely optional, off by default, and user-controlled

## Your Privacy Rights

### Data Control
- **Access**: View all data stored locally on your device
- **Export**: Export your data in standard formats
- **Delete**: Remove Mityu-managed meeting rows, search-index data, recording artifacts, and recovery-cache entries through the App

Application-controlled deletion enables SQLite and FTS secure deletion, compacts free pages, truncates the SQLite write-ahead log, and overwrites/unlinks Mityu-managed recording artifacts before reporting success. It cannot guarantee forensic erasure from SSD wear-leveling, copy-on-write storage, filesystem or cloud snapshots, backups, swap, exported/shared copies, or residual WebView/browser-storage pages. Unknown files you add to a meeting folder are retained.

Deleting only transcript text or closing the App does not remove the raw recording. To remove Mityu's managed raw audio in v1.0.4, delete the meeting through the App; independently exported or copied files remain under your control.


### Analytics Transparency
- **Open source**: Full analytics implementation available for review in our source code
- **Opt-in**: New and existing installs have analytics disabled until you turn it on
- **Questions**: Contact us for any analytics-related concerns

## Data Security

### Local Security
- The local SQLite database uses SQLCipher when the operating-system credential store is available. On a fresh or already-plaintext database, Mityu has a documented local-first plaintext fallback if key acquisition or conversion fails; an existing encrypted database is never opened without its key.
- Audio recordings, transcript/metadata JSON files, and exports are ordinary files protected by your operating system and filesystem permissions; Mityu does not currently apply application-level encryption to those files.
- The offline capture/transcription path transmits no meeting data; content is sent only when you explicitly choose a remote provider or export/share it
- Standard file system permissions protect your data

### Open Source Transparency
- Full source code available for security review
- Community-audited privacy implementations
- No hidden data collection or tracking

## Changes to This Policy

We will notify users of any material changes to this privacy policy through:
- Updates to this document in our GitHub repository
- Release notes for application updates
- In-app notifications for significant privacy changes

## Contact Us

For privacy-related questions or concerns:
- **GitHub Issues**: [Create an issue](https://github.com/aydogandagidir/mityu/issues)
- **Email**: info@bluedev.dev
- **Website**: [bluedev.dev](https://bluedev.dev)

## Open Source Commitment

As an open-source project under MIT license, you can:
- Review our complete privacy implementation
- Modify data handling to meet your requirements
- Deploy entirely on your own infrastructure
- Contribute to privacy improvements

---

*This privacy policy describes Mityu v1.0.4. For enterprise deployments, additional privacy controls may be available.*
