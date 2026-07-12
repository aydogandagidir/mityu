/**
 * Unit tests for the TrialBanner render decision (ADR-0023).
 *
 * Run with `pnpm test` (Vitest, `environment: 'node'` — see `vitest.config.ts`).
 * The component delegates its entire "what do I render?" decision to
 * `getTrialBannerVariant`, so these pure-logic cases pin the banner behavior:
 * quiet trial (days 8–14) renders nothing, the last 7 days render the chip,
 * expired/revoked render the persistent banner, licensed renders nothing.
 */

import { describe, it, expect } from 'vitest';
import type { LicensingStatus } from '@/types/licensing';
import {
  TRIAL_CHIP_THRESHOLD_DAYS,
  getTrialBannerVariant,
  trialChipLabel,
} from './trialBannerLogic';

/** Build a LicensingStatus with sane defaults for the fields under test. */
function status(overrides: Partial<LicensingStatus>): LicensingStatus {
  return {
    state: 'trial',
    daysLeft: null,
    plan: null,
    expiresAt: null,
    displayKey: null,
    reason: null,
    configured: true,
    ...overrides,
  };
}

describe('getTrialBannerVariant', () => {
  it('renders nothing before the first status fetch resolves', () => {
    expect(getTrialBannerVariant(null)).toBe('none');
  });

  it('stays quiet early in the trial (days 8-14)', () => {
    expect(getTrialBannerVariant(status({ state: 'trial', daysLeft: 14 }))).toBe('none');
    expect(getTrialBannerVariant(status({ state: 'trial', daysLeft: 10 }))).toBe('none');
    expect(getTrialBannerVariant(status({ state: 'trial', daysLeft: 8 }))).toBe('none');
  });

  it('shows the chip through the last 7 trial days', () => {
    expect(getTrialBannerVariant(status({ state: 'trial', daysLeft: 7 }))).toBe('chip');
    expect(getTrialBannerVariant(status({ state: 'trial', daysLeft: 3 }))).toBe('chip');
    expect(getTrialBannerVariant(status({ state: 'trial', daysLeft: 1 }))).toBe('chip');
    expect(getTrialBannerVariant(status({ state: 'trial', daysLeft: 0 }))).toBe('chip');
  });

  it('pins the chip threshold to exactly 7 days', () => {
    expect(TRIAL_CHIP_THRESHOLD_DAYS).toBe(7);
    expect(
      getTrialBannerVariant(status({ state: 'trial', daysLeft: TRIAL_CHIP_THRESHOLD_DAYS + 1 })),
    ).toBe('none');
  });

  it('stays quiet for a trial with unknown daysLeft (fail-quiet, never a false alarm)', () => {
    expect(getTrialBannerVariant(status({ state: 'trial', daysLeft: null }))).toBe('none');
  });

  it('shows the persistent banner once the trial expired', () => {
    expect(getTrialBannerVariant(status({ state: 'trial_expired', daysLeft: null }))).toBe('banner');
  });

  it('shows the persistent banner when the license is revoked', () => {
    expect(
      getTrialBannerVariant(status({ state: 'revoked', reason: 'The seat was released.' })),
    ).toBe('banner');
  });

  it('renders nothing while licensed (including the outside-Tauri stub)', () => {
    expect(getTrialBannerVariant(status({ state: 'licensed', plan: 'pro' }))).toBe('none');
    // Browser/design-route stub: licensed + not configured -> no licensing chrome.
    expect(getTrialBannerVariant(status({ state: 'licensed', configured: false }))).toBe('none');
  });
});

describe('trialChipLabel', () => {
  it('pluralizes day counts', () => {
    expect(trialChipLabel(7)).toBe('7 days left in trial');
    expect(trialChipLabel(2)).toBe('2 days left in trial');
  });

  it('uses the singular form for one day', () => {
    expect(trialChipLabel(1)).toBe('1 day left in trial');
  });

  it('says "Trial ends today" at zero (and never negative phrasing)', () => {
    expect(trialChipLabel(0)).toBe('Trial ends today');
    expect(trialChipLabel(-1)).toBe('Trial ends today');
  });
});
