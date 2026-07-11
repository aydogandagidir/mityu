//! Tauri command surface for licensing (ADR-0023). Registered in `lib.rs`.
//!
//! TS bindings (frontend `licensingService` — FROZEN CONTRACT):
//! ```ts
//! const status = await invoke<LicensingStatus>('get_licensing_status');
//! const status = await invoke<LicensingStatus>('activate_license', { key });
//! const status = await invoke<LicensingStatus>('deactivate_license');
//! // activate_license rejects with a string starting "NOT_CONFIGURED:" when the
//! // build has no MITYU_POLAR_ORG_ID; gated capture commands reject with
//! // "LICENSE_REQUIRED:…" when the state is trial_expired/revoked.
//! ```
//!
//! Command-layer rules:
//! - a status read NEVER blocks on the network — validation is spawned
//!   fire-and-forget afterwards, at most once per 7 days (ADR-0023 §7);
//! - error strings are short human sentences (English), matching the
//!   neighboring command style; technical detail goes to the log.

use super::polar::{self, PolarApi};
use super::state::{self as lic_state, NOT_CONFIGURED_PREFIX};
use super::LicensingStatus;
use crate::state::AppState;
use chrono::Utc;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Manager, Runtime};

/// Collapses concurrent status reads into a single in-flight background
/// validation (belt over the 7-day throttle's braces).
static VALIDATION_IN_FLIGHT: AtomicBool = AtomicBool::new(false);

/// Current licensing status. Infallible and local-only: storage trouble
/// degrades to a keychain-only view instead of failing the read, and any due
/// Polar validation runs AFTER the answer, fire-and-forget.
#[tauri::command]
pub async fn get_licensing_status<R: Runtime>(app: AppHandle<R>) -> LicensingStatus {
    let now = Utc::now();
    let Some(state) = app.try_state::<AppState>() else {
        // First launch before DB init: serve the keychain-only view.
        tracing::debug!("licensing: AppState not managed yet; keychain-only status");
        return lic_state::evaluate_without_db(now).status;
    };
    let pool = state.db_manager.pool();
    let ctx = crate::context::current();
    let evaluation = lic_state::evaluate(pool, &ctx, now).await;

    if evaluation.validation_due {
        spawn_background_validation(pool.clone(), ctx);
    }

    evaluation.status
}

/// Activate a Polar license key on this device (consumes one activation seat).
/// The key + activation id are stored in the OS keychain only (constitution §7).
#[tauri::command]
pub async fn activate_license<R: Runtime>(
    app: AppHandle<R>,
    key: String,
) -> Result<LicensingStatus, String> {
    let Some(org_id) = polar::org_id() else {
        return Err(format!(
            "{NOT_CONFIGURED_PREFIX} This build has no licensing service configured, so keys \
             cannot be activated yet. The trial and all features keep working."
        ));
    };
    let Some(state) = app.try_state::<AppState>() else {
        return Err("The app is still initializing. Try again in a moment.".to_string());
    };
    let api = PolarApi::production().map_err(|e| {
        tracing::error!(error = %format!("{e:#}"), "licensing: could not build the Polar client");
        "Could not initialize the licensing client. Please restart the app and try again."
            .to_string()
    })?;

    lic_state::activate(
        state.db_manager.pool(),
        &crate::context::current(),
        &api,
        org_id,
        &key,
        &polar::device_label(),
        Utc::now(),
    )
    .await
}

/// Deactivate this device's seat (best-effort at Polar — offline still clears
/// the local license) and fall back to the trial state machine. Existing data
/// is untouched (ADR-0023 §5).
#[tauri::command]
pub async fn deactivate_license<R: Runtime>(app: AppHandle<R>) -> Result<LicensingStatus, String> {
    let Some(state) = app.try_state::<AppState>() else {
        return Err("The app is still initializing. Try again in a moment.".to_string());
    };
    let api = PolarApi::production().map_err(|e| {
        tracing::error!(error = %format!("{e:#}"), "licensing: could not build the Polar client");
        "Could not initialize the licensing client. Please restart the app and try again."
            .to_string()
    })?;

    lic_state::deactivate(
        state.db_manager.pool(),
        &crate::context::current(),
        &api,
        polar::org_id(),
        Utc::now(),
    )
    .await
}

/// The capture gate used by the recording-start and audio-import commands
/// (ADR-0023 §5 — the ONLY two gated entry points). NOT a Tauri command.
///
/// Fail-open: if licensing state cannot be determined (no DB yet, storage
/// trouble), capture is allowed — a licensing bug must never cost a meeting.
pub async fn ensure_capture_allowed<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let Some(state) = app.try_state::<AppState>() else {
        tracing::debug!("licensing: AppState not managed yet; allowing capture (fail-open)");
        return Ok(());
    };
    lic_state::check_capture_allowed(
        state.db_manager.pool(),
        &crate::context::current(),
        Utc::now(),
    )
    .await
}

/// Spawn the ≤1/7-day Polar validation as a detached task (never blocks the
/// status read that triggered it). No-op when the build is unconfigured or a
/// validation is already in flight.
fn spawn_background_validation(pool: sqlx::SqlitePool, ctx: crate::context::AuthContext) {
    let Some(org_id) = polar::org_id() else {
        return; // unconfigured builds never phone home
    };
    if VALIDATION_IN_FLIGHT
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return;
    }
    tauri::async_runtime::spawn(async move {
        let api = match PolarApi::production() {
            Ok(api) => api,
            Err(e) => {
                tracing::warn!(error = %format!("{e:#}"), "licensing: skipping validation (client build failed)");
                VALIDATION_IN_FLIGHT.store(false, Ordering::SeqCst);
                return;
            }
        };
        lic_state::run_validation(&pool, &ctx, &api, org_id, Utc::now()).await;
        VALIDATION_IN_FLIGHT.store(false, Ordering::SeqCst);
    });
}
