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
