/**
 * Unit tests for the licensing service wrapper (ADR-0023).
 *
 * Run with `pnpm test` (Vitest, `environment: 'node'` — see `vitest.config.ts`).
 * `@tauri-apps/api/core` is mocked so no real IPC exists; the node environment
 * has no `window`, which IS the outside-Tauri case (`isTauri()` false). The
 * Tauri path is exercised by stubbing a `window` carrying `__TAURI_INTERNALS__`.
 *
 * Pins the two contract-critical behaviors:
 * 1. Outside Tauri every method resolves the safe stub
 *    ({ state: 'licensed', configured: false }) WITHOUT touching invoke(),
 *    so browser/design routes render with no licensing chrome.
 * 2. Inside Tauri each method is a pure 1-to-1 invoke wrapper (command name,
 *    args, result and rejection pass through unchanged).
 */

import { afterEach, describe, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { licensingService } from './licensingService';
import type { LicensingStatus } from '@/types/licensing';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

const mockedInvoke = vi.mocked(invoke);

/** Make `isTauri()` return true by stubbing the marker the helper checks for. */
function enterTauriShell(): void {
  vi.stubGlobal('window', { __TAURI_INTERNALS__: {} });
}

const BACKEND_STATUS: LicensingStatus = {
  state: 'trial',
  daysLeft: 5,
  plan: null,
  expiresAt: null,
  displayKey: null,
  reason: null,
  configured: true,
};

afterEach(() => {
  vi.unstubAllGlobals();
  mockedInvoke.mockReset();
});

describe('licensingService outside the Tauri shell (browser/design routes)', () => {
  it('getLicensingStatus resolves the safe stub without invoking', async () => {
    const status = await licensingService.getLicensingStatus();

    expect(status).toEqual({
      state: 'licensed',
      daysLeft: null,
      plan: null,
      expiresAt: null,
      displayKey: null,
      reason: null,
      configured: false,
    });
    expect(mockedInvoke).not.toHaveBeenCalled();
  });

  it('activateLicense resolves the safe stub without invoking', async () => {
    const status = await licensingService.activateLicense('ANY-KEY');

    expect(status.state).toBe('licensed');
    expect(status.configured).toBe(false);
    expect(mockedInvoke).not.toHaveBeenCalled();
  });

  it('deactivateLicense resolves the safe stub without invoking', async () => {
    const status = await licensingService.deactivateLicense();

    expect(status.state).toBe('licensed');
    expect(status.configured).toBe(false);
    expect(mockedInvoke).not.toHaveBeenCalled();
  });
});

describe('licensingService inside the Tauri shell', () => {
  it('getLicensingStatus passes the backend status through 1-to-1', async () => {
    enterTauriShell();
    mockedInvoke.mockResolvedValueOnce(BACKEND_STATUS);

    const status = await licensingService.getLicensingStatus();

    expect(mockedInvoke).toHaveBeenCalledExactlyOnceWith('get_licensing_status');
    expect(status).toEqual(BACKEND_STATUS);
  });

  it('activateLicense sends the key and returns the fresh status', async () => {
    enterTauriShell();
    const licensed: LicensingStatus = {
      ...BACKEND_STATUS,
      state: 'licensed',
      daysLeft: null,
      plan: 'pro',
      displayKey: 'MITYU-****-****-AB12',
    };
    mockedInvoke.mockResolvedValueOnce(licensed);

    const status = await licensingService.activateLicense('the-key');

    expect(mockedInvoke).toHaveBeenCalledExactlyOnceWith('activate_license', { key: 'the-key' });
    expect(status).toEqual(licensed);
  });

  it('activateLicense propagates the backend string rejection unchanged', async () => {
    enterTauriShell();
    mockedInvoke.mockRejectedValueOnce('NOT_CONFIGURED: no Polar organization id');

    await expect(licensingService.activateLicense('the-key')).rejects.toBe(
      'NOT_CONFIGURED: no Polar organization id',
    );
  });

  it('deactivateLicense invokes the command and returns the fresh status', async () => {
    enterTauriShell();
    const backToTrial: LicensingStatus = { ...BACKEND_STATUS, state: 'trial_expired', daysLeft: null };
    mockedInvoke.mockResolvedValueOnce(backToTrial);

    const status = await licensingService.deactivateLicense();

    expect(mockedInvoke).toHaveBeenCalledExactlyOnceWith('deactivate_license');
    expect(status).toEqual(backToTrial);
  });
});
