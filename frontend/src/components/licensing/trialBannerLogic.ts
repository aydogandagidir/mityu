/**
 * Pure render-decision logic for the trial banner/chip (ADR-0023).
 *
 * Kept free of React so it can be unit-tested in the repo's node-environment
 * Vitest setup (see `vitest.config.ts` — pure logic, no DOM).
 */

import type { LicensingStatus } from '@/types/licensing';

/** What the TrialBanner component should render for a given status. */
export type TrialBannerVariant = 'none' | 'chip' | 'banner';

/**
 * Last N trial days that show the reminder chip. Days above this threshold
 * (8–14 of the 14-day trial) stay quiet — no licensing chrome at all.
 */
export const TRIAL_CHIP_THRESHOLD_DAYS = 7;

/**
 * Decide the banner variant:
 * - `banner`  — persistent slim banner: trial expired or license revoked.
 * - `chip`    — unobtrusive inline chip: trial with <= 7 days left.
 * - `none`    — licensed, early ("quiet") trial, unknown days, or no status yet.
 */
export function getTrialBannerVariant(status: LicensingStatus | null): TrialBannerVariant {
  if (!status) return 'none';
  switch (status.state) {
    case 'trial_expired':
    case 'revoked':
      return 'banner';
    case 'trial':
      return status.daysLeft !== null && status.daysLeft <= TRIAL_CHIP_THRESHOLD_DAYS
        ? 'chip'
        : 'none';
    case 'licensed':
    default:
      return 'none';
  }
}

/** Chip label, e.g. "5 days left in trial" / "1 day left in trial" / "Trial ends today". */
export function trialChipLabel(daysLeft: number): string {
  if (daysLeft <= 0) return 'Trial ends today';
  if (daysLeft === 1) return '1 day left in trial';
  return `${daysLeft} days left in trial`;
}
