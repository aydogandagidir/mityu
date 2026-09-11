//! Polar.sh customer-portal license-key client (ADR-0023 §2/§3).
//!
//! Only Polar's **public** customer-portal endpoints are used — they need no API
//! secret, so nothing sensitive is baked into the binary. Requests carry ids
//! only (license key, organization id, activation id, pseudonymous device label) — never
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
//!
//! Requests pin Polar's date-based API version (`Polar-Version: YYYY-MM`, see
//! [`DEFAULT_API_VERSION`]). The pin is **soft**: when Polar refuses the pinned
//! version — which is what happens once a version is retired, and this binary
//! cannot be recalled — the request is retried unpinned instead of surfacing as
//! a 404, because a 404 is read here as a statement about the license.

use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use std::time::Duration;

/// Production API host.
pub const PRODUCTION_BASE_URL: &str = "https://api.polar.sh";

/// Sandbox API host (Polar's test environment) — for manual/dev testing only;
/// unit tests use a fake [`LicenseApi`] and never hit the network.
pub const SANDBOX_BASE_URL: &str = "https://sandbox-api.polar.sh";

/// Header Polar reads to pin the API version a request is answered with
/// (`YYYY-MM`). Omitting it resolves to whatever is *Current* at the time of
/// the call, which rotates every quarter.
const VERSION_HEADER: &str = "Polar-Version";

/// The Polar API version this build pins.
///
/// `2026-04` is Current at the time of writing and is the version Polar's
/// rollout notice recommends pinning. The rotation on 2026-10-01 is a no-op
/// for this client either way: the transitive request/response schemas of the
/// three customer-portal endpoints below are identical in `2026-04` and
/// `2026-10` (diffed from `https://api.polar.sh/<version>/openapi.json`). Bump
/// this — and re-diff those documents — before the pinned version is retired
/// (`2026-04` is removed at the January 2027 release).
///
/// Pinning is only safe because [`PolarApi::post`] falls back to an unpinned
/// request when Polar refuses the pin; see [`is_version_rejection`].
const DEFAULT_API_VERSION: &str = "2026-04";

/// The API version requests are pinned to. A non-blank
/// `MITYU_POLAR_API_VERSION` at build time overrides [`DEFAULT_API_VERSION`]
/// (the same compile-time pattern as [`org_id`]), so a build can be aimed at a
/// future version for testing without editing the source.
pub fn api_version() -> &'static str {
    match option_env!("MITYU_POLAR_API_VERSION") {
        Some(raw) if !raw.trim().is_empty() => raw.trim(),
        _ => DEFAULT_API_VERSION,
    }
}

/// Whether a response is Polar refusing the pinned API version rather than
/// answering the request.
///
/// Polar echoes the version it served in a `Polar-Version` response header on
/// every request it routes — including application-level 404s such as "no such
/// license key". A malformed, unknown, or removed version never reaches the
/// endpoint: it comes back as a bare 404 with no echo. That pair is the only
/// runtime signal available; Polar publishes no `Sunset`/`Deprecation` headers.
/// Observed against the live API on 2026-09-11 — and if Polar ever stops
/// echoing the header, the cost is one redundant unpinned retry, never a wrong
/// licensing verdict.
fn is_version_rejection(status: reqwest::StatusCode, headers: &reqwest::header::HeaderMap) -> bool {
    status.as_u16() == 404 && !headers.contains_key(VERSION_HEADER)
}

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

/// The `label` sent with an activation. It is a random, pseudonymous identifier
/// rather than the machine hostname. We persist it in the OS credential store
/// when possible so the Polar portal still shows a stable device label without
/// disclosing a user-chosen computer name.
pub fn device_label() -> String {
    use crate::secrets::licensing::{self, DEVICE_LABEL_ENTRY};

    if let Ok(Some(existing)) = licensing::get(DEVICE_LABEL_ENTRY) {
        if is_valid_device_label(&existing) {
            return existing;
        }
    }

    let label = format!("mityu-{}", uuid::Uuid::new_v4().simple());
    if let Err(error) = licensing::set(DEVICE_LABEL_ENTRY, &label) {
        // A locked keychain must not make activation impossible. The generated
        // label is still non-identifying; it is simply not reusable next time.
        tracing::warn!(
            error = %format!("{error:#}"),
            "licensing: could not persist the pseudonymous device label"
        );
    }
    label
}

fn is_valid_device_label(label: &str) -> bool {
    let Some(id) = label.strip_prefix("mityu-") else {
        return false;
    };
    id.len() == 32 && id.bytes().all(|byte| byte.is_ascii_hexdigit())
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
            // License keys live in POST bodies. Never replay them to a redirect
            // target, even if Polar or an intermediary returns 307/308.
            .redirect(reqwest::redirect::Policy::none())
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
        let response = self
            .client
            .post(self.url(action))
            .header(VERSION_HEADER, api_version())
            .json(body)
            .send()
            .await
            .with_context(|| {
                format!("licensing: request to the Polar '{action}' endpoint failed (offline?)")
            })?;

        // The pin ships inside a signed, installed binary we cannot recall, and
        // every Polar version is eventually removed. Polar answers an unknown or
        // removed version with a bare 404 — the same status this client reads as a
        // verdict about the license, and on `validate` a sustained 404 deletes the
        // stored key (see `super::state::run_validation`). Left unhandled, a stale
        // pin would de-license the installed base. Retry once unpinned instead: an
        // omitted header always resolves to Current, so the worst case is the
        // unpinned behaviour this client shipped with before.
        if is_version_rejection(response.status(), response.headers()) {
            tracing::warn!(
                pinned_version = api_version(),
                action,
                "licensing: Polar did not accept the pinned API version; retrying \
                 unpinned (bump DEFAULT_API_VERSION)"
            );
            return self
                .client
                .post(self.url(action))
                .json(body)
                .send()
                .await
                .with_context(|| {
                    format!("licensing: unpinned retry of the Polar '{action}' endpoint failed")
                });
        }

        Ok(response)
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
    fn device_label_format_is_pseudonymous_and_bounded() {
        crate::secrets::test_store::install();
        let label = device_label();
        assert!(is_valid_device_label(&label));
        assert_eq!(device_label(), label, "the stored label must be stable");
        assert!(!label.contains(' '));
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
    fn pinned_api_version_is_a_polar_release_month() {
        let version = api_version();
        let (year, month) = version
            .split_once('-')
            .unwrap_or_else(|| panic!("Polar-Version must be YYYY-MM, got {version:?}"));
        assert_eq!(year.len(), 4, "bad year in {version:?}");
        assert!(
            year.bytes().all(|byte| byte.is_ascii_digit()),
            "bad year in {version:?}"
        );
        let month: u8 = month
            .parse()
            .unwrap_or_else(|_| panic!("bad month in {version:?}"));
        // Polar cuts a version in the first week of January, April, July, October.
        assert!(
            matches!(month, 1 | 4 | 7 | 10),
            "not a Polar release month: {version:?}"
        );
    }

    #[test]
    fn version_rejection_is_a_404_without_the_echoed_version_header() {
        use reqwest::header::{HeaderMap, HeaderValue};
        use reqwest::StatusCode;

        let mut echoed = HeaderMap::new();
        // Polar echoes the header lowercased; the lookup must be case-insensitive.
        echoed.insert("polar-version", HeaderValue::from_static("2026-04"));

        // A routed 404 ("no such license key") is a real answer about the license.
        assert!(!is_version_rejection(StatusCode::NOT_FOUND, &echoed));
        // A 404 with no echo is the version being refused, not an answer.
        assert!(is_version_rejection(
            StatusCode::NOT_FOUND,
            &HeaderMap::new()
        ));
        // Nothing else is ever read as a rejection.
        for status in [
            StatusCode::OK,
            StatusCode::NO_CONTENT,
            StatusCode::FORBIDDEN,
            StatusCode::UNPROCESSABLE_ENTITY,
            StatusCode::INTERNAL_SERVER_ERROR,
        ] {
            assert!(
                !is_version_rejection(status, &HeaderMap::new()),
                "{status} must not be treated as a version rejection"
            );
        }
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
