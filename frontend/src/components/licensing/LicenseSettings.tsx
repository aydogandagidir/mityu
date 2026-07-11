'use client';

/**
 * Settings -> License section (ADR-0023)
 *
 * Shows the current licensing state (trial days / masked key + plan / expired /
 * revoked + reason) with the matching actions:
 * - Activate license  -> opens the shared ActivateLicenseDialog.
 * - Deactivate        -> only when licensed, behind an inline confirm step
 *                        (frees the seat under the 2-device activation limit).
 * - Buy Mityu Pro     -> checkout placeholder (data-todo="checkout", no href yet).
 *
 * Visual parity with sibling sections (BetaSettings card layout), but with
 * semantic theme tokens only.
 */

import React, { useState } from 'react';
import { KeyRound, Loader2 } from 'lucide-react';
import { toast } from 'sonner';
import { Button } from '@/components/ui/button';
import { useLicensing } from '@/contexts/LicensingContext';
import { licensingErrorText, type LicensingState, type LicensingStatus } from '@/types/licensing';

const STATE_PILLS: Record<LicensingState, { label: string; className: string }> = {
  trial: { label: 'Trial', className: 'bg-accent text-accent-foreground' },
  trial_expired: { label: 'Trial ended', className: 'bg-destructive/10 text-destructive' },
  licensed: { label: 'Licensed', className: 'bg-primary/10 text-primary' },
  revoked: { label: 'Revoked', className: 'bg-destructive/10 text-destructive' },
};

/** One-sentence state line under the section title. */
function statusLine(status: LicensingStatus): string {
  switch (status.state) {
    case 'trial': {
      const days = status.daysLeft;
      if (days === null) return 'Free trial active.';
      if (days <= 0) return 'Free trial — ends today.';
      return days === 1 ? 'Free trial — 1 day left.' : `Free trial — ${days} days left.`;
    }
    case 'trial_expired':
      return 'Your free trial has ended. Your existing meetings stay fully accessible.';
    case 'licensed':
      return 'This device is licensed.';
    case 'revoked':
      return `${status.reason?.trim() || 'Your license is no longer active.'} Your existing meetings stay fully accessible.`;
  }
}

/** Locale date for an ISO8601 expiry, or null when absent/unparseable. */
function formatExpiry(expiresAt: string | null): string | null {
  if (!expiresAt) return null;
  const date = new Date(expiresAt);
  return Number.isNaN(date.getTime()) ? null : date.toLocaleDateString();
}

export function LicenseSettings() {
  const { status, deactivate, openActivateDialog } = useLicensing();
  const [confirmingDeactivate, setConfirmingDeactivate] = useState(false);
  const [deactivating, setDeactivating] = useState(false);

  const handleDeactivate = async () => {
    setDeactivating(true);
    try {
      await deactivate();
      setConfirmingDeactivate(false);
      toast.success('License deactivated', {
        description: 'This seat is now free for another device.',
      });
    } catch (error) {
      toast.error('Could not deactivate the license', {
        description: licensingErrorText(error) || 'Please try again.',
      });
    } finally {
      setDeactivating(false);
    }
  };

  if (!status) {
    return (
      <div className="bg-card rounded-lg border border-border p-6 shadow-sm">
        <p className="text-sm text-muted-foreground">Checking license status…</p>
      </div>
    );
  }

  const pill = STATE_PILLS[status.state];
  const expiry = formatExpiry(status.expiresAt);
  const isLicensed = status.state === 'licensed';

  return (
    <div className="space-y-6">
      <div className="bg-card rounded-lg border border-border p-6 shadow-sm">
        <div className="flex items-center gap-2 mb-2">
          <KeyRound className="h-5 w-5 text-muted-foreground" aria-hidden="true" />
          <h3 className="text-lg font-semibold text-foreground">License</h3>
          <span className={`px-2 py-0.5 text-xs font-medium rounded-full ${pill.className}`}>
            {pill.label}
          </span>
        </div>

        <p className="text-sm text-muted-foreground">{statusLine(status)}</p>

        {isLicensed && (
          <div className="mt-3 space-y-1 text-sm text-muted-foreground">
            {status.displayKey && (
              <p>
                Key: <span className="font-mono text-foreground">{status.displayKey}</span>
              </p>
            )}
            {status.plan && (
              <p>
                Plan: <span className="font-medium capitalize text-foreground">{status.plan}</span>
              </p>
            )}
            {expiry && <p>Valid until {expiry}.</p>}
          </div>
        )}

        <div className="mt-4 flex flex-wrap items-center gap-2">
          {!isLicensed && (
            <>
              <Button onClick={() => openActivateDialog()}>Activate license</Button>
              <Button variant="outline" data-todo="checkout">
                Buy Mityu Pro
              </Button>
            </>
          )}

          {isLicensed && !confirmingDeactivate && (
            <Button variant="outline" onClick={() => setConfirmingDeactivate(true)}>
              Deactivate on this device
            </Button>
          )}

          {isLicensed && confirmingDeactivate && (
            <div className="flex flex-wrap items-center gap-2">
              <span className="text-sm text-muted-foreground">
                Free this seat for another device?
              </span>
              <Button
                variant="destructive"
                size="sm"
                disabled={deactivating}
                onClick={() => void handleDeactivate()}
              >
                {deactivating && <Loader2 className="h-4 w-4 animate-spin" aria-hidden="true" />}
                Deactivate
              </Button>
              <Button
                variant="ghost"
                size="sm"
                disabled={deactivating}
                onClick={() => setConfirmingDeactivate(false)}
              >
                Cancel
              </Button>
            </div>
          )}
        </div>
      </div>

      {/* Never-locked promise (ADR-0023 §5) — mirrors the BetaSettings info box. */}
      <div className="p-4 bg-accent border border-primary/20 rounded-lg">
        <p className="text-sm text-primary">
          <strong>Your data is never locked.</strong> Every existing meeting — transcripts,
          summaries, search and export — stays available without a license. A license is only
          needed to record or import new audio after the trial.
        </p>
      </div>
    </div>
  );
}

export default LicenseSettings;
