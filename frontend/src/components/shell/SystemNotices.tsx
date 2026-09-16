'use client';

/**
 * The one slot for app-wide system notices — DESIGN_SYSTEM.md §5.3 "with-banner".
 *
 * Before this, the trial chrome and the encryption warning each supplied their own
 * `px-4 pt-4` wrapper and stacked above whatever top padding the page already had, so
 * three gutters competed for the same 16px and the gap changed depending on which
 * banners happened to be live. The banners now render bare and this slot owns the
 * spacing — and renders nothing at all, not even a gap, when both are silent.
 *
 * 🔒 Both children keep their own visibility rules, live-region semantics and copy:
 * `TrialBanner` renders nothing while licensed or early in the trial;
 * `EncryptionStatusBanner` renders only when the database is CONFIRMED unencrypted,
 * is never dismissable, and is never shown during onboarding (it is not mounted there).
 */

import React from 'react';
import { TrialBanner } from '@/components/licensing/TrialBanner';
import { EncryptionStatusBanner } from '@/components/consent/EncryptionStatusBanner';

export function SystemNotices() {
  return (
    <div className="empty:hidden [&>*:not(:first-child)]:mt-2 [&>*]:mx-gutter [&>*]:mt-2">
      <TrialBanner />
      <EncryptionStatusBanner />
    </div>
  );
}

export default SystemNotices;
