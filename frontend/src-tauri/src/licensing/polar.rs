//! Polar.sh customer-portal license-key client (ADR-0023 §2/§3).
//!
//! Only Polar's **public** customer-portal endpoints are used — they need no API
//! secret, so nothing sensitive is baked into the binary. Requests carry ids
//! only (license key, organization id, activation id, hostname label) — never
//! meeting content, never telemetry (constitution §10).
//!
//! The HTTP surface sits behind the small [`LicenseApi`] trait so the state
//! machine ([`super::state`]) is unit-testable with a fake — tests never touch
//! the network. Transport-class failures (timeouts, DNS, TLS, unexpected 5xx)
//! surface as `Err(_)`; HTTP-semantic outcomes (403 limit, 404 unknown, 422
//! invalid) are values, because the caller reacts to them differently
//! (fail-open vs. act).
//!
//! The org id is compile-time injected via `option_env!("MITYU_POLAR_ORG_ID")`
//! (the ADR-0016 PostHog pattern): unset ⇒ activation reports "not configured"
//! while trial mechanics keep working; release builds inject the real id. The
//! id is public by design (it is in every checkout URL anyway).

use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use std::time::Duration;

/// Production API host.
pub const PRODUCTION_BASE_URL: &str = "https://api.polar.sh";

/// Sandbox API host (Polar's test environment) — for manual/dev testing only;
/// unit tests use a fake [`LicenseApi`] and never hit the network.
pub const SANDBOX_BASE_URL: &str = "https://sandbox-api.polar.sh";

/// The baked-in Polar organization id that owns the "Mityu Pro" product. Public
/// by design — it is in every checkout URL and returned by Polar's public
/// checkout API — so it ships in the binary rather than as a secret. Verified
/// 2026-07-12 against the live Mityu Pro checkout session's `organization_id`.
const DEFAULT_ORG_ID: &str = "2afb00f6-61f3-47f9-be97-be37d83bfd64";

/// The Polar organization id licensing calls use. Defaults to [`DEFAULT_ORG_ID`];
/// a non-blank `MITYU_POLAR_ORG_ID` at build time overrides it (e.g. to point at
/// a sandbox org for testing). Only `None` if both are blank, which disables
/// licensing config while trial mechanics keep working.
pub fn org_id() -> Option<&'static str> {
    let candidate = match option_env!("MITYU_POLAR_ORG_ID") {
        Some(raw) if !raw.trim().is_empty() => raw.trim(),
        _ => DEFAULT_ORG_ID,
    };
    if candidate.is_empty() {
        None
    } else {
        Some(candidate)
    }
}

/// Whether this build can talk to Polar at all (`configured` in the status).
pub fn is_configured() -> bool {
    org_id().is_some()
}

/// The `label` sent with an activation: the machine's hostname (no PII beyond
/// the machine name — it is what Polar's self-service portal shows the customer
/// so they can tell their devices apart).
pub fn device_label() -> String {
    sysinfo::System::host_name().unwrap_or_else(|| "mityu-device".to_string())
}

/// Outcome of `POST /v1/customer-portal/license-keys/activate`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ActivateOutcome {
    /// 200 — a seat was consumed; keep `activation_id` for validate/deactivate.
    Activated {
        activation_id: String,
        /// `expires_at` from the returned `license_key` object, when present.
        expires_at: Option<String>,
    },
    /// 403 — the key's activation limit (e.g. 2 devices) is exhausted.
    LimitReached,
    /// 404 — no such key in this organization.
    KeyNotFound,
    /// 422 — the request was structurally rejected (malformed key/body).
    Invalid,
}

/// Outcome of `POST /v1/customer-portal/license-keys/validate`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ValidateOutcome {
    /// 200 with `status: "granted"`.
    Granted { expires_at: Option<String> },
    /// 200 with `status: "revoked"`.
    Revoked,
    /// 200 with `status: "disabled"`.
    Disabled,
    /// 200 with a status token this build does not know — treated fail-open.
    Unknown(String),
    /// 404 — the key or our activation no longer exists (seat freed via
    /// Polar's portal) ⇒ clear the local license, fall back to the trial.
    NotFound,
    /// 422 — structurally rejected.
    Invalid,
}

/// Outcome of `POST /v1/customer-portal/license-keys/deactivate`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeactivateOutcome {
    /// 204 — the seat was freed.
    Done,
    /// 404 — already gone; treat as freed.
    NotFound,
    /// 422 — structurally rejected.
    Invalid,
}

/// The seam the state machine talks through. Implemented by [`PolarApi`]
/// (reqwest) and by test fakes. `Err(_)` is transport-class only.
#[async_trait::async_trait]
pub trait LicenseApi: Send + Sync {
    async fn activate(
        &self,
        key: &str,
        organization_id: &str,
        label: &str,
    ) -> Result<ActivateOutcome>;

    async fn validate(
        &self,
        key: &str,
        organization_id: &str,
        activation_id: &str,
    ) -> Result<ValidateOutcome>;

    async fn deactivate(
        &self,
        key: &str,
        organization_id: &str,
        activation_id: &str,
    ) -> Result<DeactivateOutcome>;
}

/// 200 body of `activate`: `{ id, license_key { … } }`.
#[derive(Debug, Deserialize)]
struct ActivateResponse {
    /// The activation id (our seat).
    id: String,
    #[serde(default)]
    license_key: Option<LicenseKeyMeta>,
}

/// The subset of Polar's `license_key` object we care about; every other field
/// is ignored (lenient by design — upstream may add fields).
#[derive(Debug, Default, Deserialize)]
struct LicenseKeyMeta {
    #[serde(default)]
    expires_at: Option<String>,
}

/// 200 body of `validate`: `{ status, expires_at, limit_activations, customer…, … }`.
#[derive(Debug, Deserialize)]
struct ValidateResponse {
    status: String,
    #[serde(default)]
    expires_at: Option<String>,
}

/// The reqwest-backed [`LicenseApi`]. Short budgets (connect ≤ 5 s, total
/// ≤ 10 s), **no retries** — a licensing check must never make the app feel
/// hung, and every caller is fail-open on transport errors anyway.
pub struct PolarApi {
    client: reqwest::Client,
    base_url: String,
}

impl PolarApi {
    /// Build against an explicit base host (production, sandbox, or a test
    /// server).
    pub fn new(base_url: impl Into<String>) -> Result<Self> {
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(10))
            .build()
            .context("licensing: failed to build the HTTP client for the Polar API")?;
        Ok(Self {
            client,
            base_url: base_url.into().trim_end_matches('/').to_string(),
        })
    }

    /// The client every production caller uses.
    pub fn production() -> Result<Self> {
        Self::new(PRODUCTION_BASE_URL)
    }

    fn url(&self, action: &str) -> String {
        format!(
            "{}/v1/customer-portal/license-keys/{}",
            self.base_url, action
        )
    }

    /// POST a JSON body; map transport failures to `Err`. Never logs the body
    /// (it contains the license key).
    async fn post(&self, action: &str, body: &serde_json::Value) -> Result<reqwest::Response> {
        self.client
            .post(self.url(action))
            .json(body)
            .send()
            .await
            .with_context(|| {
                format!("licensing: request to the Polar '{action}' endpoint failed (offline?)")
            })
    }
}

#[async_trait::async_trait]
impl LicenseApi for PolarApi {
    async fn activate(
        &self,
        key: &str,
        organization_id: &str,
        label: &str,
    ) -> Result<ActivateOutcome> {
        let body = serde_json::json!({
            "key": key,
            "organization_id": organization_id,
            "label": label,
        });
        let response = self.post("activate", &body).await?;
        let status = response.status();
        match status.as_u16() {
            200 => {
                let parsed: ActivateResponse = response
                    .json()
                    .await
                    .context("licensing: Polar activate returned 200 with an unreadable body")?;
                Ok(ActivateOutcome::Activated {
                    activation_id: parsed.id,
                    expires_at: parsed.license_key.unwrap_or_default().expires_at,
                })
            }
            403 => Ok(ActivateOutcome::LimitReached),
            404 => Ok(ActivateOutcome::KeyNotFound),
            422 => Ok(ActivateOutcome::Invalid),
            code => Err(anyhow!(
                "licensing: Polar activate returned unexpected HTTP {code}"
            )),
        }
    }

    async fn validate(
        &self,
        key: &str,
        organization_id: &str,
        activation_id: &str,
    ) -> Result<ValidateOutcome> {
        let body = serde_json::json!({
            "key": key,
            "organization_id": organization_id,
            "activation_id": activation_id,
        });
        let response = self.post("validate", &body).await?;
        let status = response.status();
        match status.as_u16() {
            200 => {
                let parsed: ValidateResponse = response
                    .json()
                    .await
                    .context("licensing: Polar validate returned 200 with an unreadable body")?;
                Ok(match parsed.status.as_str() {
                    "granted" => ValidateOutcome::Granted {
                        expires_at: parsed.expires_at,
                    },
                    "revoked" => ValidateOutcome::Revoked,
                    "disabled" => ValidateOutcome::Disabled,
                    other => ValidateOutcome::Unknown(other.to_string()),
                })
            }
            404 => Ok(ValidateOutcome::NotFound),
            422 => Ok(ValidateOutcome::Invalid),
            code => Err(anyhow!(
                "licensing: Polar validate returned unexpected HTTP {code}"
            )),
        }
    }

    async fn deactivate(
        &self,
        key: &str,
        organization_id: &str,
        activation_id: &str,
    ) -> Result<DeactivateOutcome> {
        let body = serde_json::json!({
            "key": key,
            "organization_id": organization_id,
            "activation_id": activation_id,
        });
        let response = self.post("deactivate", &body).await?;
        let status = response.status();
        match status.as_u16() {
            204 => Ok(DeactivateOutcome::Done),
            404 => Ok(DeactivateOutcome::NotFound),
            422 => Ok(DeactivateOutcome::Invalid),
            code => Err(anyhow!(
                "licensing: Polar deactivate returned unexpected HTTP {code}"
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn org_id_is_none_or_nonblank() {
        // The build env may or may not carry MITYU_POLAR_ORG_ID; either way the
        // accessor must never yield a blank id (an unset CI secret renders "").
        match org_id() {
            None => assert!(!is_configured()),
            Some(id) => {
                assert!(!id.trim().is_empty());
                assert!(is_configured());
            }
        }
    }

    #[test]
    fn device_label_is_never_empty() {
        assert!(!device_label().is_empty());
    }

    #[test]
    fn activate_response_parses_documented_schema() {
        // Shape from ADR-0023 §2 / the Polar customer-portal docs; extra fields
        // must be ignored.
        let raw = r#"{
            "id": "act-1234",
            "license_key": {
                "id": "lk-1",
                "key": "MITYU-AAAA-BBBB-CCCC",
                "status": "granted",
                "expires_at": "2027-07-11T00:00:00Z",
                "limit_activations": 2
            },
            "label": "my-laptop",
            "meta": {}
        }"#;
        let parsed: ActivateResponse = serde_json::from_str(raw).expect("parse");
        assert_eq!(parsed.id, "act-1234");
        assert_eq!(
            parsed.license_key.unwrap_or_default().expires_at.as_deref(),
            Some("2027-07-11T00:00:00Z")
        );

        // Minimal body (no license_key object) still parses.
        let parsed: ActivateResponse =
            serde_json::from_str(r#"{"id": "act-min"}"#).expect("parse minimal");
        assert_eq!(parsed.id, "act-min");
        assert!(parsed.license_key.is_none());
    }

    #[test]
    fn validate_response_parses_documented_schema() {
        let raw = r#"{
            "status": "granted",
            "expires_at": null,
            "limit_activations": 2,
            "customer": {"id": "cus-1", "email": "x@example.com"},
            "usage": 0
        }"#;
        let parsed: ValidateResponse = serde_json::from_str(raw).expect("parse");
        assert_eq!(parsed.status, "granted");
        assert_eq!(parsed.expires_at, None);

        let parsed: ValidateResponse =
            serde_json::from_str(r#"{"status": "revoked", "expires_at": "2026-01-01T00:00:00Z"}"#)
                .expect("parse revoked");
        assert_eq!(parsed.status, "revoked");
        assert_eq!(parsed.expires_at.as_deref(), Some("2026-01-01T00:00:00Z"));
    }

    #[test]
    fn base_url_trailing_slash_is_normalized() {
        let api = PolarApi::new("https://sandbox-api.polar.sh/").expect("client");
        assert_eq!(
            api.url("validate"),
            "https://sandbox-api.polar.sh/v1/customer-portal/license-keys/validate"
        );
    }
}
