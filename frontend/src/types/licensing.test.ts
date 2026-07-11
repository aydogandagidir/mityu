/**
 * Unit tests for the licensing error helpers (ADR-0023).
 *
 * Run with `pnpm test` (Vitest, `environment: 'node'` — see `vitest.config.ts`).
 * Pure-logic assertions: no DOM, no mocks, no I/O.
 *
 * The frozen backend contract rejects gated commands (recording start, audio
 * import) with a STRING starting with "LICENSE_REQUIRED:"; `activate_license`
 * may reject with "NOT_CONFIGURED:...". These cases pin the detection logic the
 * paywall interception depends on.
 */

import { describe, it, expect } from 'vitest';
import {
  LICENSE_REQUIRED_PREFIX,
  NOT_CONFIGURED_PREFIX,
  isLicenseRequiredError,
  isNotConfiguredError,
  licensingErrorText,
} from './licensing';

describe('isLicenseRequiredError', () => {
  it('detects the raw string rejection from a gated Tauri command', () => {
    expect(
      isLicenseRequiredError('LICENSE_REQUIRED: trial expired — activate a license to record'),
    ).toBe(true);
  });

  it('detects the prefix with no trailing message', () => {
    expect(isLicenseRequiredError(LICENSE_REQUIRED_PREFIX)).toBe(true);
  });

  it('detects an Error wrapper around the string (re-thrown upstream)', () => {
    expect(isLicenseRequiredError(new Error('LICENSE_REQUIRED: trial expired'))).toBe(true);
  });

  it('detects a { message } shaped rejection', () => {
    expect(isLicenseRequiredError({ message: 'LICENSE_REQUIRED: revoked' })).toBe(true);
  });

  it('requires the prefix at the START of the message', () => {
    expect(isLicenseRequiredError('error: LICENSE_REQUIRED: nope')).toBe(false);
  });

  it('rejects ordinary error strings', () => {
    expect(isLicenseRequiredError('Failed to open microphone')).toBe(false);
  });

  it('rejects non-error values (null, undefined, numbers, plain objects)', () => {
    expect(isLicenseRequiredError(null)).toBe(false);
    expect(isLicenseRequiredError(undefined)).toBe(false);
    expect(isLicenseRequiredError(42)).toBe(false);
    expect(isLicenseRequiredError({})).toBe(false);
    expect(isLicenseRequiredError({ message: 123 })).toBe(false);
  });
});

describe('isNotConfiguredError', () => {
  it('detects the NOT_CONFIGURED activation rejection', () => {
    expect(isNotConfiguredError('NOT_CONFIGURED: no Polar organization id in this build')).toBe(true);
    expect(isNotConfiguredError(NOT_CONFIGURED_PREFIX)).toBe(true);
  });

  it('rejects other activation errors', () => {
    expect(isNotConfiguredError('Activation limit reached')).toBe(false);
    expect(isNotConfiguredError('LICENSE_REQUIRED: trial expired')).toBe(false);
  });
});

describe('licensingErrorText', () => {
  it('passes strings through', () => {
    expect(licensingErrorText('boom')).toBe('boom');
  });

  it('extracts Error messages', () => {
    expect(licensingErrorText(new Error('boom'))).toBe('boom');
  });

  it('extracts { message } string props', () => {
    expect(licensingErrorText({ message: 'boom' })).toBe('boom');
  });

  it('returns an empty string for unusable values', () => {
    expect(licensingErrorText(null)).toBe('');
    expect(licensingErrorText(42)).toBe('');
    expect(licensingErrorText({ message: 42 })).toBe('');
  });
});
