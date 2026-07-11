'use client';

/**
 * ActivateLicenseDialog (ADR-0023)
 *
 * The single license-activation dialog, rendered once by `LicensingProvider`
 * (prop-driven like `RecordingConsentDialog` — it does not read the licensing
 * context itself, which keeps the module graph cycle-free).
 *
 * Two modes:
 * - default   — opened from Settings or the trial chip/banner.
 * - `paywall` — opened because a gated action (recording start / audio import)
 *               was rejected with a LICENSE_REQUIRED error; adds a one-line
 *               explainer and a Buy button. Per ADR-0023 §5 the gate blocks
 *               NEW capture only — existing meetings are never locked, and the
 *               copy says so explicitly.
 */

import React, { useCallback, useEffect, useRef, useState } from 'react';
import { CheckCircle2, KeyRound, Loader2 } from 'lucide-react';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import {
  isNotConfiguredError,
  licensingErrorText,
  type LicensingStatus,
} from '@/types/licensing';
import { openCheckout } from '@/lib/checkout';

export interface ActivateLicenseDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** Paywall mode: opened because a gated action was rejected (LICENSE_REQUIRED). */
  paywall?: boolean;
  /** Performs the activation; resolves with the fresh licensing status. */
  activate: (key: string) => Promise<LicensingStatus>;
}

/** How long the success state stays visible before the dialog auto-closes. */
const SUCCESS_AUTO_CLOSE_MS = 1800;

/** Map a raw activation rejection to a friendly, user-facing sentence. */
function friendlyActivationError(e: unknown): string {
  if (isNotConfiguredError(e)) {
    return "This build isn't connected to the store yet.";
  }
  const text = licensingErrorText(e).trim();
  return text.length > 0
    ? text
    : 'Could not activate the license. Please check the key and try again.';
}

type Phase = 'input' | 'activating' | 'success';

export function ActivateLicenseDialog({
  open,
  onOpenChange,
  paywall = false,
  activate,
}: ActivateLicenseDialogProps) {
  const [key, setKey] = useState('');
  const [phase, setPhase] = useState<Phase>('input');
  const [error, setError] = useState<string | null>(null);
  const [activated, setActivated] = useState<LicensingStatus | null>(null);

  // Reset only on the closed -> open transition (ImportAudioDialog pattern),
  // so a background re-render never wipes what the user typed.
  const prevOpenRef = useRef(false);
  useEffect(() => {
    const wasOpen = prevOpenRef.current;
    prevOpenRef.current = open;
    if (open && !wasOpen) {
      setKey('');
      setPhase('input');
      setError(null);
      setActivated(null);
    }
  }, [open]);

  // Success: linger for a beat, then auto-close.
  useEffect(() => {
    if (!open || phase !== 'success') return;
    const timer = setTimeout(() => onOpenChange(false), SUCCESS_AUTO_CLOSE_MS);
    return () => clearTimeout(timer);
  }, [open, phase, onOpenChange]);

  const handleActivate = useCallback(async () => {
    const trimmed = key.trim();
    if (!trimmed || phase === 'activating') return;

    setPhase('activating');
    setError(null);
    try {
      const status = await activate(trimmed);
      setActivated(status);
      setPhase('success');
    } catch (e) {
      setPhase('input');
      setError(friendlyActivationError(e));
    }
  }, [key, phase, activate]);

  const isBusy = phase === 'activating';

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            {phase === 'success' ? (
              <>
                <CheckCircle2 className="h-5 w-5 text-primary" aria-hidden="true" />
                License activated
              </>
            ) : (
              <>
                <KeyRound className="h-5 w-5 text-primary" aria-hidden="true" />
                {paywall ? 'License required' : 'Activate license'}
              </>
            )}
          </DialogTitle>
          <DialogDescription>
            {phase === 'success'
              ? 'This device is now licensed.'
              : 'Paste your license key to use it on this device.'}
          </DialogDescription>
        </DialogHeader>

        {phase === 'success' ? (
          <div className="flex items-start gap-3 rounded-lg border border-border bg-accent p-4">
            <CheckCircle2 className="h-5 w-5 shrink-0 text-primary" aria-hidden="true" />
            <div className="min-w-0 text-sm">
              <p className="font-medium text-accent-foreground">
                {activated?.plan ? (
                  <>
                    Plan: <span className="capitalize">{activated.plan}</span>
                  </>
                ) : (
                  'Thanks for supporting Mityu.'
                )}
              </p>
              {activated?.displayKey && (
                <p className="mt-1 font-mono text-xs text-muted-foreground">
                  {activated.displayKey}
                </p>
              )}
            </div>
          </div>
        ) : (
          <div className="space-y-3 py-1">
            {paywall && (
              <p className="rounded-lg border border-border bg-accent p-3 text-sm text-accent-foreground">
                The free trial has ended — recording new meetings requires a
                license. Your existing meetings are never locked.
              </p>
            )}

            <Input
              value={key}
              onChange={(e) => setKey(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter') {
                  e.preventDefault();
                  void handleActivate();
                }
              }}
              placeholder="Paste your license key"
              aria-label="License key"
              className="font-mono"
              autoFocus
              disabled={isBusy}
              spellCheck={false}
              autoComplete="off"
            />

            {error && (
              <p
                role="alert"
                className="rounded-lg border border-destructive/30 bg-destructive/10 p-3 text-sm text-destructive"
              >
                {error}
              </p>
            )}
          </div>
        )}

        {phase !== 'success' && (
          <DialogFooter>
            {paywall && (
              <Button
                type="button"
                variant="outline"
                onClick={() => openCheckout()}
                className="sm:mr-auto"
              >
                Buy Mityu Pro
              </Button>
            )}
            <Button type="button" variant="ghost" onClick={() => onOpenChange(false)} disabled={isBusy}>
              Cancel
            </Button>
            <Button type="button" onClick={() => void handleActivate()} disabled={!key.trim() || isBusy}>
              {isBusy && <Loader2 className="h-4 w-4 animate-spin" aria-hidden="true" />}
              {isBusy ? 'Activating…' : 'Activate'}
            </Button>
          </DialogFooter>
        )}
      </DialogContent>
    </Dialog>
  );
}

export default ActivateLicenseDialog;
