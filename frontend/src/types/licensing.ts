/**
 * Licensing & Trial Type System (ADR-0023)
 *
 * Frozen contract with the Rust `licensing/` module:
 *   invoke('get_licensing_status')            -> LicensingStatus
 *   invoke('activate_license', { key })       -> LicensingStatus (rejects with a string)
 *   invoke('deactivate_license')              -> LicensingStatus (rejects with a string)
 *
 * Gated backend commands (recording start, audio import) reject with a string
 * error starting with {@link LICENSE_REQUIRED_PREFIX}; the UI maps that to the
 * paywall dialog instead of a raw error surface. `activate_license` may reject
 * with a string starting with {@link NOT_CONFIGURED_PREFIX} when the Polar
 * organization id was not baked into this build.
 *
 * ADR-0023 §5: expiry gates CAPTURE only — never data. Existing meetings,
 * search, playback and exports stay available forever regardless of state.
 */

/** Licensing state machine (single source of truth lives in Rust). */
export type LicensingState = 'trial' | 'trial_expired' | 'licensed' | 'revoked';

/** Snapshot returned by `get_licensing_status` / `activate_license` / `deactivate_license`. */
export interface LicensingStatus {
  state: LicensingState;
  /** Days remaining in the trial; only meaningful while `state === 'trial'`. */
  daysLeft: number | null;
  /** Plan identifier, e.g. "pro"; only meaningful when licensed. */
  plan: string | null;
  /** ISO8601 expiry of the license, when the plan has one (e.g. Business yearly). */
  expiresAt: string | null;
  /** Masked license key safe for display (never the full secret key). */
  displayKey: string | null;
  /** Human sentence explaining a `revoked` state. */
  reason: string | null;
  /** `false` = the Polar organization id is not baked into this build (activation unavailable). */
  configured: boolean;
}

/** Prefix of the string error thrown by license-gated backend commands. */
export const LICENSE_REQUIRED_PREFIX = 'LICENSE_REQUIRED:';

/** Prefix of the string error thrown by `activate_license` in unconfigured builds. */
export const NOT_CONFIGURED_PREFIX = 'NOT_CONFIGURED:';

/**
 * Best-effort extraction of the error text from an unknown rejection value.
 * Tauri rejects command errors as plain strings; be tolerant of `Error`
 * instances and `{ message }` shapes produced by re-throws/wrappers.
 */
export function licensingErrorText(e: unknown): string {
  if (typeof e === 'string') return e;
  if (e instanceof Error) return e.message;
  if (
    typeof e === 'object' &&
    e !== null &&
    'message' in e &&
    typeof (e as { message: unknown }).message === 'string'
  ) {
    return (e as { message: string }).message;
  }
  return '';
}

/**
 * True when a rejection came from the ADR-0023 license gate
 * (recording start / audio import blocked in `trial_expired` or `revoked`).
 */
export function isLicenseRequiredError(e: unknown): boolean {
  return licensingErrorText(e).startsWith(LICENSE_REQUIRED_PREFIX);
}

/** True when `activate_license` rejected because this build has no store connection. */
export function isNotConfiguredError(e: unknown): boolean {
  return licensingErrorText(e).startsWith(NOT_CONFIGURED_PREFIX);
}
