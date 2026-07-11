/**
 * Licensing Service (ADR-0023)
 *
 * Typed wrapper over the licensing Tauri backend calls — the ONLY place the
 * licensing commands are invoked (no raw `invoke` in components/contexts).
 *
 * Outside the Tauri shell (browser dev-preview / design routes, where
 * `isTauri()` is false and `invoke` would throw) every method resolves to a
 * safe stub — `state: 'licensed'`, `configured: false` — so those routes
 * render without any licensing chrome (no banner, no paywall).
 */

import { invoke } from '@tauri-apps/api/core';
import { isTauri } from '@/lib/isTauri';
import type { LicensingStatus } from '@/types/licensing';

/**
 * Stub returned outside the Tauri shell. `licensed` suppresses the trial
 * banner/paywall; `configured: false` marks it as a non-store build.
 */
const BROWSER_STUB: LicensingStatus = {
  state: 'licensed',
  daysLeft: null,
  plan: null,
  expiresAt: null,
  displayKey: null,
  reason: null,
  configured: false,
};

/**
 * Licensing Service
 * Singleton service for trial/license status and activation lifecycle.
 */
export class LicensingService {
  /**
   * Current licensing snapshot (trial days left, plan, masked key, ...).
   * Never touches the network on the backend's read path (local-first).
   */
  async getLicensingStatus(): Promise<LicensingStatus> {
    if (!isTauri()) return { ...BROWSER_STUB };
    return invoke<LicensingStatus>('get_licensing_status');
  }

  /**
   * Activate a license key on this device.
   * @param key - The full license key pasted by the user.
   * @returns The fresh licensing status after activation.
   * @throws A string error, e.g. "NOT_CONFIGURED:..." or a human sentence.
   */
  async activateLicense(key: string): Promise<LicensingStatus> {
    if (!isTauri()) return { ...BROWSER_STUB };
    return invoke<LicensingStatus>('activate_license', { key });
  }

  /**
   * Deactivate the license on this device (frees the seat under the
   * activation limit).
   * @returns The fresh licensing status after deactivation.
   * @throws A string error with a human message.
   */
  async deactivateLicense(): Promise<LicensingStatus> {
    if (!isTauri()) return { ...BROWSER_STUB };
    return invoke<LicensingStatus>('deactivate_license');
  }
}

// Export singleton instance
export const licensingService = new LicensingService();

/** Convenience wrapper: current licensing snapshot. */
export function getLicensingStatus(): Promise<LicensingStatus> {
  return licensingService.getLicensingStatus();
}

/** Convenience wrapper: activate a license key on this device. */
export function activateLicense(key: string): Promise<LicensingStatus> {
  return licensingService.activateLicense(key);
}

/** Convenience wrapper: deactivate the license on this device. */
export function deactivateLicense(): Promise<LicensingStatus> {
  return licensingService.deactivateLicense();
}
