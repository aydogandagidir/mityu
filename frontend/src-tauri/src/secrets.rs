//! BYOK secret storage in the OS credential store (Windows Credential Manager /
//! macOS Keychain / Linux keyutils), per CLAUDE.md §0.7/§3 and
//! `docs/SECURITY_PRIVACY.md` "Secrets": **LLM/API keys live in the OS keychain,
//! never in SQLite plaintext, source, or logs.**
//!
//! This module is the single wrapper around [`keyring`]. The persistence layer
//! (`database/repositories/setting.rs`) stores only the non-secret
//! [`KEYCHAIN_MARKER`] in its legacy `*ApiKey` columns and defers the actual
//! secret to the functions here. `docs/DATA_MODEL.md` (`provider_credential`)
//! captures the same rule: the column is a *reference*, the key is in the keychain.
//!
//! ## Tenant-aware by design
//! Every entry name is scoped by `ctx.tenant_id` (workspace) AND by a
//! [`SecretDomain`] (summary vs transcript), so:
//!
//! - a future non-`local` workspace cannot read another workspace's key
//!   (`docs/MULTITENANCY.md` rule 2 — identity comes only from [`AuthContext`]);
//! - the summary "openai"/"groq" key and the transcript "openai"/"groq" key
//!   (the two settings tables share provider names) never clobber each other.
//!
//! The label that a future sync might replicate is only the *reference* (the
//! marker) — the secret itself never leaves the device.
//!
//! ## Fail closed, offline-safe
//! All operations run fully offline (the OS store is local). If the store is
//! unavailable we return a user-friendly `anyhow` error and **never** fall back
//! to plaintext. Key *values* are never logged — only provider/domain metadata.

use crate::context::AuthContext;
use anyhow::{Context, Result};
use keyring::Entry;

/// Reverse-domain service name under which every entry is filed in the OS store.
/// Matches the Tauri bundle identifier (`com.bluedev.mityu`).
pub const KEYCHAIN_SERVICE: &str = "com.bluedev.mityu";

/// Non-secret sentinel written into the legacy `*ApiKey` SQLite columns once a
/// key has been moved to the OS credential store. Presence of this exact value
/// means "the real secret lives in the keychain"; any other non-empty value is a
/// legacy plaintext key awaiting the one-time migration in `database::repositories::setting`.
///
/// Versioned so a future storage-scheme change can be detected without a schema
/// migration. Deliberately not a valid API-key prefix for any supported provider.
pub const KEYCHAIN_MARKER: &str = "keychain:v1";

/// Which settings table / provider namespace a credential belongs to. Prevents
/// the summary and transcript tables' overlapping provider names (`openai`,
/// `groq`) from mapping to the same keychain entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SecretDomain {
    /// `settings` table — summarization/LLM providers.
    Summary,
    /// `transcript_settings` table — speech-to-text providers.
    Transcript,
}

impl SecretDomain {
    /// Stable, lowercase segment embedded in the entry name. Changing these
    /// strings would orphan previously stored keys, so they are frozen.
    fn as_str(self) -> &'static str {
        match self {
            SecretDomain::Summary => "summary",
            SecretDomain::Transcript => "transcript",
        }
    }
}

/// Build the workspace + domain + provider scoped entry name.
/// Shape: `"{tenant_id}:{domain}:{provider}:api_key"`.
fn entry_name(ctx: &AuthContext, domain: SecretDomain, provider: &str) -> String {
    format!("{}:{}:{}:api_key", ctx.tenant_id, domain.as_str(), provider)
}

/// Open a keychain [`Entry`] for the scoped `(workspace, domain, provider)` key.
/// Failure here means the OS store itself is unreachable → fail closed.
fn open_entry(ctx: &AuthContext, domain: SecretDomain, provider: &str) -> Result<Entry> {
    let name = entry_name(ctx, domain, provider);
    Entry::new(KEYCHAIN_SERVICE, &name).with_context(|| {
        format!(
            "secrets: could not open the OS credential store for provider '{}' ({}). \
             Your system keychain may be locked or unavailable.",
            provider,
            domain.as_str(),
        )
    })
}

/// Store `api_key` for `(ctx.tenant_id, domain, provider)` in the OS credential
/// store, overwriting any existing value. The key value is never logged.
///
/// # Errors
/// Returns a user-facing error (never a plaintext fallback) if the store is
/// unavailable.
pub fn set_api_key(
    ctx: &AuthContext,
    domain: SecretDomain,
    provider: &str,
    api_key: &str,
) -> Result<()> {
    let entry = open_entry(ctx, domain, provider)?;
    entry.set_password(api_key).with_context(|| {
        format!(
            "secrets: failed to write the API key for provider '{}' ({}) to the OS credential store",
            provider,
            domain.as_str(),
        )
    })?;
    tracing::debug!(
        provider,
        domain = domain.as_str(),
        tenant_id = %ctx.tenant_id,
        "stored API key in OS credential store"
    );
    Ok(())
}

/// Read the API key for `(ctx.tenant_id, domain, provider)` from the OS
/// credential store. Returns `Ok(None)` when no entry exists (never set, or
/// deleted). Any other failure is surfaced (fail closed — no plaintext fallback).
pub fn get_api_key(
    ctx: &AuthContext,
    domain: SecretDomain,
    provider: &str,
) -> Result<Option<String>> {
    let entry = open_entry(ctx, domain, provider)?;
    match entry.get_password() {
        Ok(secret) => Ok(Some(secret)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(e).with_context(|| {
            format!(
                "secrets: failed to read the API key for provider '{}' ({}) from the OS credential store",
                provider,
                domain.as_str(),
            )
        }),
    }
}

/// Remove the API key for `(ctx.tenant_id, domain, provider)` from the OS
/// credential store. Deleting a non-existent entry is a no-op (`Ok(())`).
pub fn delete_api_key(ctx: &AuthContext, domain: SecretDomain, provider: &str) -> Result<()> {
    let entry = open_entry(ctx, domain, provider)?;
    match entry.delete_credential() {
        Ok(()) => {
            tracing::debug!(
                provider,
                domain = domain.as_str(),
                tenant_id = %ctx.tenant_id,
                "deleted API key from OS credential store"
            );
            Ok(())
        }
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e).with_context(|| {
            format!(
                "secrets: failed to delete the API key for provider '{}' ({}) from the OS credential store",
                provider,
                domain.as_str(),
            )
        }),
    }
}

/// At-rest encryption key for the local SQLite database (BACKLOG B3, SQLCipher).
///
/// Unlike BYOK provider keys, the DB key is **device-scoped, not workspace-scoped**:
/// there is exactly one encrypted DB file per install holding every workspace's
/// rows, so it is filed under one fixed entry ([`db::ENTRY_NAME`]) in the same OS
/// store ([`KEYCHAIN_SERVICE`]). It is generated once on first launch, stored as
/// lowercase hex, and read back on every startup — all fully offline.
///
/// Same guarantees as the rest of this module: the key is **never logged** (only a
/// "generated"/"loaded" event with no value), and every operation **fails closed** —
/// if the OS store is unavailable we return a user-facing `anyhow` error and the
/// caller MUST abort rather than fall back to an unencrypted database.
pub mod db {
    use anyhow::{anyhow, Context, Result};
    use keyring::Entry;
    use rand::RngCore;
    use zeroize::Zeroizing;

    /// Fixed OS-store entry name for the single device-wide DB key. Not
    /// workspace-scoped (one encrypted file spans all workspaces). Frozen —
    /// changing it would orphan the existing key and lock the user out of their DB.
    pub const ENTRY_NAME: &str = "db-key";

    /// Raw key length in bytes. 32 bytes = 256 bits of entropy, passed to SQLCipher
    /// as a raw key (`PRAGMA key = "x'<64 hex chars>'"`) so SQLCipher skips key
    /// derivation and uses the bytes directly.
    pub const KEY_LEN: usize = 32;

    /// Open the [`Entry`] for the device DB key. A failure here means the OS store
    /// itself is unreachable → fail closed.
    fn open_entry() -> Result<Entry> {
        Entry::new(super::KEYCHAIN_SERVICE, ENTRY_NAME).with_context(|| {
            format!(
                "secrets: could not open the OS credential store for the database \
                 encryption key (entry '{ENTRY_NAME}'). Your system keychain may be \
                 locked or unavailable."
            )
        })
    }

    /// Encode raw key bytes as lowercase hex (the on-store and SQLCipher wire form).
    /// Wrapped in [`Zeroizing`] so the derived hex is scrubbed on drop, same as the
    /// raw key bytes.
    fn to_hex(bytes: &[u8]) -> Zeroizing<String> {
        let mut s = String::with_capacity(bytes.len() * 2);
        for b in bytes {
            use std::fmt::Write as _;
            let _ = write!(s, "{b:02x}");
        }
        Zeroizing::new(s)
    }

    /// Validate that a stored string is exactly `KEY_LEN` bytes of lowercase hex.
    /// A malformed value must fail closed (never silently regenerate — that would
    /// throw away the key that can still decrypt the user's data).
    fn validate_hex(hex: &str) -> Result<()> {
        if hex.len() != KEY_LEN * 2 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(anyhow!(
                "secrets: the stored database key is malformed ({} chars); refusing to \
                 proceed so existing encrypted data is not lost. Restore the OS keychain \
                 entry or the pre-encryption backup.",
                hex.len()
            ));
        }
        Ok(())
    }

    /// Generate a fresh 32-byte key from the OS CSPRNG and return it as lowercase hex.
    /// The transient raw-byte buffer and the returned hex are both zeroized on drop.
    fn generate_hex() -> Zeroizing<String> {
        let mut key = Zeroizing::new([0u8; KEY_LEN]);
        rand::thread_rng().fill_bytes(key.as_mut());
        to_hex(key.as_ref())
    }

    /// Return the database encryption key as lowercase hex, **generating and
    /// persisting** it on first use. Subsequent calls read the same key back.
    ///
    /// Wrapped in [`Zeroizing`] so the key material is scrubbed from memory on drop
    /// (the caller should not keep it alive past the pool-open call). Runs fully
    /// offline. The key value is never logged. Fails closed: if the OS store cannot
    /// be read or written, an error is returned and the caller MUST abort (no
    /// unencrypted fallback).
    pub fn get_or_create_hex() -> Result<Zeroizing<String>> {
        let entry = open_entry()?;
        match entry.get_password() {
            Ok(existing) => {
                let existing = Zeroizing::new(existing);
                validate_hex(&existing)?;
                tracing::debug!("loaded database encryption key from OS credential store");
                Ok(existing)
            }
            Err(keyring::Error::NoEntry) => {
                let hex = generate_hex();
                entry.set_password(&hex).with_context(|| {
                    "secrets: failed to store a newly generated database encryption key in \
                     the OS credential store"
                        .to_string()
                })?;
                tracing::info!(
                    "generated a new database encryption key and stored it in the OS \
                     credential store"
                );
                Ok(hex)
            }
            Err(e) => Err(e).with_context(|| {
                "secrets: failed to read the database encryption key from the OS credential \
                 store (it may be locked or unavailable); refusing to open the database \
                 unencrypted"
                    .to_string()
            }),
        }
    }

    /// Read the database key hex **without** creating it. Returns `Ok(None)` when no
    /// key has ever been stored (fresh install before first open). Any other store
    /// failure is surfaced (fail closed). The key is [`Zeroizing`]-wrapped.
    pub fn get_hex() -> Result<Option<Zeroizing<String>>> {
        let entry = open_entry()?;
        match entry.get_password() {
            Ok(existing) => {
                let existing = Zeroizing::new(existing);
                validate_hex(&existing)?;
                Ok(Some(existing))
            }
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(e).with_context(|| {
                "secrets: failed to read the database encryption key from the OS credential store"
                    .to_string()
            }),
        }
    }

    /// The SQLCipher `PRAGMA key` value for a raw (non-derived) key.
    ///
    /// sqlx emits `PRAGMA key = {value};`, so the value must be a **double-quoted**
    /// string whose content is the raw-key literal `x'<hex>'` (empirically the only
    /// form this libsqlite3-sys/SQLCipher build accepts — an unquoted `x'…'` is
    /// parsed as a blob literal and rejected with a syntax error). SQLCipher then
    /// reads the 32 raw key bytes directly (no KDF). The same value is used for the
    /// live pool and for opening the encrypted side during conversion.
    pub fn pragma_key_value(hex: &str) -> String {
        format!("\"x'{hex}'\"")
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn generated_key_is_64_hex_chars() {
            let hex = generate_hex();
            assert_eq!(hex.len(), KEY_LEN * 2);
            assert!(hex.bytes().all(|b| b.is_ascii_hexdigit()));
            validate_hex(&hex).expect("generated key must validate");
            // Two generations must differ (CSPRNG, not a constant).
            assert_ne!(hex, generate_hex());
        }

        #[test]
        fn pragma_value_is_raw_key_form() {
            // Double-quoted so sqlx's `PRAGMA key = {value}` yields a string whose
            // content is the raw-key literal x'…' (the accepted SQLCipher form).
            assert_eq!(pragma_key_value("00ff"), "\"x'00ff'\"");
        }

        #[test]
        fn malformed_hex_is_rejected() {
            assert!(validate_hex("").is_err());
            assert!(validate_hex("zz").is_err());
            assert!(validate_hex(&"a".repeat(KEY_LEN * 2 - 1)).is_err());
            validate_hex(&"a".repeat(KEY_LEN * 2)).expect("well-formed hex accepted");
        }

        #[test]
        fn get_or_create_round_trips_through_store() {
            super::super::test_store::install();
            let first = get_or_create_hex().expect("first get_or_create");
            let second = get_or_create_hex().expect("second get_or_create");
            assert_eq!(first, second, "key must be stable across calls");
            let stored = get_hex().expect("get_hex");
            assert_eq!(stored.as_deref().map(String::as_str), Some(first.as_str()));
        }
    }
}

/// Licensing entries (ADR-0023): the trial anchor, the Polar license key and its
/// activation id.
///
/// Like [`db`], these are **device-scoped, not workspace-scoped**: a license seat
/// is consumed per machine (Polar's activation limit counts devices), and the
/// trial clock is a property of the install, so all three are filed under fixed
/// entry names in the same OS store ([`KEYCHAIN_SERVICE`]).
///
/// Storage rules (CLAUDE.md §0.7, constitution §7): the license key and
/// activation id are **keychain-only** — never SQLite, never source, never logs.
/// The trial anchor is not a secret, but it lives here too (second copy of the
/// anchor; the first is the `licensingState` JSON blob) so wiping the app's data
/// directory alone does not reset the trial. Same discipline as the rest of this
/// module: **values are never logged**, and every operation runs fully offline.
pub mod licensing {
    use anyhow::{Context, Result};
    use keyring::Entry;

    /// First-launch trial anchor (RFC 3339 timestamp). Frozen — renaming would
    /// restart every in-flight trial.
    pub const TRIAL_ANCHOR_ENTRY: &str = "licensing:trial-anchor";

    /// The Polar license key exactly as the user entered it. SECRET.
    pub const KEY_ENTRY: &str = "licensing:key";

    /// The Polar activation id returned by a successful activate call. Needed to
    /// validate/deactivate this device's seat.
    pub const ACTIVATION_ID_ENTRY: &str = "licensing:activation-id";

    /// Stable, pseudonymous label used for a Polar activation. This is not a
    /// secret, but keeping it in the credential store avoids exposing the host
    /// name and keeps the label stable across app restarts.
    pub const DEVICE_LABEL_ENTRY: &str = "licensing:device-label";

    /// Open the [`Entry`] for one of the fixed licensing entry names. Failure
    /// means the OS store itself is unreachable.
    fn open_entry(name: &str) -> Result<Entry> {
        Entry::new(super::KEYCHAIN_SERVICE, name).with_context(|| {
            format!(
                "secrets: could not open the OS credential store for licensing \
                 entry '{name}'. Your system keychain may be locked or unavailable."
            )
        })
    }

    /// Read a licensing entry. `Ok(None)` when it was never set (or was deleted);
    /// any other store failure is surfaced. The value is never logged.
    pub fn get(name: &str) -> Result<Option<String>> {
        let entry = open_entry(name)?;
        match entry.get_password() {
            Ok(value) => Ok(Some(value)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(e).with_context(|| {
                format!(
                    "secrets: failed to read licensing entry '{name}' from the OS credential store"
                )
            }),
        }
    }

    /// Store a licensing entry, overwriting any existing value. The value is
    /// never logged.
    pub fn set(name: &str, value: &str) -> Result<()> {
        let entry = open_entry(name)?;
        entry.set_password(value).with_context(|| {
            format!("secrets: failed to write licensing entry '{name}' to the OS credential store")
        })?;
        tracing::debug!(
            entry = name,
            "stored licensing entry in OS credential store"
        );
        Ok(())
    }

    /// Remove a licensing entry. Deleting a non-existent entry is a no-op.
    pub fn delete(name: &str) -> Result<()> {
        let entry = open_entry(name)?;
        match entry.delete_credential() {
            Ok(()) => {
                tracing::debug!(entry = name, "deleted licensing entry from OS credential store");
                Ok(())
            }
            Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(e).with_context(|| {
                format!(
                    "secrets: failed to delete licensing entry '{name}' from the OS credential store"
                )
            }),
        }
    }
}

/// Process-global, persistent, in-memory credential store for tests.
///
/// keyring's built-in `mock` backend is `EntryOnly` (each `Entry::new` gets a
/// fresh empty credential — a set on one `Entry` is invisible to a later `Entry`
/// for the same name), so it cannot verify a set→get round-trip across the
/// separate `Entry::new` calls this module makes. This store keys secrets by
/// `(service, entry_name)` in a shared map, faithfully modelling a real OS store
/// (persist across opens, workspace-scoped names) while NEVER touching the
/// machine credential manager. Test-only.
#[cfg(test)]
pub(crate) mod test_store {
    use keyring::credential::{
        Credential, CredentialApi, CredentialBuilderApi, CredentialPersistence,
    };
    use keyring::Error;
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};

    type Key = (String, String); // (service, entry_name)

    fn store() -> &'static Mutex<HashMap<Key, Vec<u8>>> {
        static STORE: OnceLock<Mutex<HashMap<Key, Vec<u8>>>> = OnceLock::new();
        STORE.get_or_init(|| Mutex::new(HashMap::new()))
    }

    #[derive(Debug)]
    struct MemCredential {
        key: Key,
    }

    impl CredentialApi for MemCredential {
        fn set_secret(&self, secret: &[u8]) -> Result<(), Error> {
            store()
                .lock()
                .unwrap()
                .insert(self.key.clone(), secret.to_vec());
            Ok(())
        }
        fn get_secret(&self) -> Result<Vec<u8>, Error> {
            match store().lock().unwrap().get(&self.key) {
                Some(v) => Ok(v.clone()),
                None => Err(Error::NoEntry),
            }
        }
        fn delete_credential(&self) -> Result<(), Error> {
            match store().lock().unwrap().remove(&self.key) {
                Some(_) => Ok(()),
                None => Err(Error::NoEntry),
            }
        }
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    }

    struct MemBuilder;

    impl CredentialBuilderApi for MemBuilder {
        fn build(
            &self,
            _target: Option<&str>,
            service: &str,
            user: &str,
        ) -> Result<Box<Credential>, Error> {
            Ok(Box::new(MemCredential {
                key: (service.to_string(), user.to_string()),
            }))
        }
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
        fn persistence(&self) -> CredentialPersistence {
            CredentialPersistence::UntilDelete
        }
    }

    /// Install this store as the process-wide default. Idempotent per process.
    pub fn install() {
        use std::sync::Once;
        static ONCE: Once = Once::new();
        ONCE.call_once(|| {
            keyring::set_default_credential_builder(Box::new(MemBuilder));
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::{AuthContext, RequestId, Role, TenantId, UserId};

    fn ensure_mock() {
        super::test_store::install();
    }

    fn ctx_for(tenant: &str) -> AuthContext {
        AuthContext {
            tenant_id: TenantId::new(tenant),
            user_id: UserId::new("unit-user"),
            roles: vec![Role::Owner],
            request_id: RequestId::generate(),
        }
    }

    #[test]
    fn set_get_delete_round_trip() {
        ensure_mock();
        let ctx = ctx_for("rt-ws");
        // Unique provider per test avoids cross-test collisions in the shared mock.
        set_api_key(&ctx, SecretDomain::Summary, "rt-openai", "sk-round-trip").unwrap();
        assert_eq!(
            get_api_key(&ctx, SecretDomain::Summary, "rt-openai").unwrap(),
            Some("sk-round-trip".to_string())
        );
        delete_api_key(&ctx, SecretDomain::Summary, "rt-openai").unwrap();
        assert_eq!(
            get_api_key(&ctx, SecretDomain::Summary, "rt-openai").unwrap(),
            None
        );
    }

    #[test]
    fn missing_entry_reads_none_and_delete_is_noop() {
        ensure_mock();
        let ctx = ctx_for("missing-ws");
        assert_eq!(
            get_api_key(&ctx, SecretDomain::Summary, "never-set").unwrap(),
            None
        );
        // Deleting a key that was never stored must not error.
        delete_api_key(&ctx, SecretDomain::Summary, "never-set").unwrap();
    }

    #[test]
    fn summary_and_transcript_domains_do_not_collide() {
        ensure_mock();
        let ctx = ctx_for("dom-ws");
        set_api_key(&ctx, SecretDomain::Summary, "openai", "sk-summary").unwrap();
        set_api_key(&ctx, SecretDomain::Transcript, "openai", "sk-transcript").unwrap();
        assert_eq!(
            get_api_key(&ctx, SecretDomain::Summary, "openai").unwrap(),
            Some("sk-summary".to_string())
        );
        assert_eq!(
            get_api_key(&ctx, SecretDomain::Transcript, "openai").unwrap(),
            Some("sk-transcript".to_string())
        );
    }

    #[test]
    fn entry_names_are_scoped_by_workspace_domain_and_provider() {
        let ctx = ctx_for("local");
        assert_eq!(
            entry_name(&ctx, SecretDomain::Summary, "openai"),
            "local:summary:openai:api_key"
        );
        assert_eq!(
            entry_name(&ctx, SecretDomain::Transcript, "openai"),
            "local:transcript:openai:api_key"
        );
    }

    #[test]
    fn different_workspaces_get_isolated_entries() {
        ensure_mock();
        let local = ctx_for("iso-local");
        let other = ctx_for("iso-other");
        set_api_key(&local, SecretDomain::Summary, "claude", "sk-local").unwrap();
        // A different workspace must not observe another workspace's key.
        assert_eq!(
            get_api_key(&other, SecretDomain::Summary, "claude").unwrap(),
            None
        );
        set_api_key(&other, SecretDomain::Summary, "claude", "sk-other").unwrap();
        assert_eq!(
            get_api_key(&local, SecretDomain::Summary, "claude").unwrap(),
            Some("sk-local".to_string())
        );
        assert_eq!(
            get_api_key(&other, SecretDomain::Summary, "claude").unwrap(),
            Some("sk-other".to_string())
        );
    }
}
