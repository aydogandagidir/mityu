'use client';

/**
 * TrialBanner (ADR-0023)
 *
 * Trial/licensing chrome at the top of MainContent, mirroring the placement of
 * `EncryptionStatusBanner` (rendered right below it in `app/layout.tsx`):
 *
 * - Quiet trial (days 8–14) and licensed: renders NOTHING.
 * - Trial, <= 7 days left: a slim, unobtrusive inline chip —
 *   "N days left in trial · Activate license".
 * - Trial expired / license revoked: a persistent slim banner with an
 *   Activate button and a Buy link. Per ADR-0023 §5 only NEW capture is
 *   gated — existing meetings stay fully accessible, and the copy says so.
 *
 * Semantic theme tokens only, so dark mode is automatic.
 */

import React from 'react';
import { KeyRound } from 'lucide-react';
import { Button } from '@/components/ui/button';
import { useLicensing } from '@/contexts/LicensingContext';
import { openCheckout } from '@/lib/checkout';
import { getTrialBannerVariant, trialChipLabel } from './trialBannerLogic';

export function TrialBanner() {
  const { status, openActivateDialog } = useLicensing();
  const variant = getTrialBannerVariant(status);

  if (variant === 'none' || status === null) {
    return null;
  }

  // The outer gutters are gone: `shell/SystemNotices` owns the spacing now, so the two
  // system banners cannot each contribute a top margin and leave a gap whose size
  // depends on which of them happens to be live.
  if (variant === 'chip') {
    return (
      <div className="flex justify-end">
        <div
          role="status"
          className="inline-flex items-center gap-1.5 rounded-full border border-border bg-accent px-3 py-1 text-caption text-accent-foreground"
        >
          <span>{trialChipLabel(status.daysLeft ?? 0)}</span>
          <span aria-hidden="true">·</span>
          <button
            type="button"
            onClick={() => openActivateDialog()}
            className="font-medium text-primary hover:underline"
          >
            Activate license
          </button>
        </div>
      </div>
    );
  }

  // Persistent slim banner: trial_expired | revoked.
  const message =
    status.state === 'revoked'
      ? `${status.reason?.trim() || 'Your license is no longer active.'} Your existing meetings stay fully accessible.`
      : 'Your free trial has ended. Your existing meetings stay fully accessible — a license is needed to record or import new audio.';

  return (
    <div
      role="status"
      aria-live="polite"
      className="flex flex-wrap items-center gap-x-4 gap-y-2 rounded-md border border-border bg-accent px-4 py-2.5"
    >
      <KeyRound className="size-4 shrink-0 text-accent-foreground" aria-hidden="true" />
      <p className="min-w-0 flex-1 text-body text-accent-foreground">{message}</p>
      <div className="flex shrink-0 items-center gap-3">
        <Button size="sm" onClick={() => openActivateDialog()}>
          Activate license
        </Button>
        <Button variant="link" size="sm" onClick={() => openCheckout()}>
          Buy Mityu Pro
        </Button>
      </div>
    </div>
  );
}

export default TrialBanner;
