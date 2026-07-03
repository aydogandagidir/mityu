'use client';

import React, { useEffect, useState } from 'react';
import { LockOpen } from 'lucide-react';
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert';
import { getDbEncryptionStatus } from '@/services/dbService';

/**
 * EncryptionStatusBanner (ADR-0014 follow-up)
 *
 * Renders a prominent, NON-dismissable warning ONLY when the local database
 * opened UNENCRYPTED at rest (the fail-open plaintext branch ran because the
 * OS keychain key was unavailable). In the normal encrypted case, and while the
 * status is still unknown (loading or the query rejected because it was called
 * before DB init finished — e.g. first launch / onboarding), it renders nothing
 * so we never raise a false alarm.
 *
 * This is a transparency/consent affordance: it reflects real at-rest state and
 * clears itself automatically once the DB is encrypted again, so there is no
 * user-facing dismiss control by design.
 */

// Possible outcomes of the encryption-status probe.
// `unknown` covers both the initial load and any rejection (pre-init / transient),
// and maps to rendering nothing.
type EncryptionState = 'unknown' | 'encrypted' | 'unencrypted';

// The command can REJECT if called before DB init completes (first-launch /
// onboarding). Retry once shortly after in case init finishes momentarily.
const RETRY_DELAY_MS = 3000;

export function EncryptionStatusBanner() {
  const [state, setState] = useState<EncryptionState>('unknown');

  useEffect(() => {
    let cancelled = false;
    let retryTimer: ReturnType<typeof setTimeout> | undefined;

    const check = async (): Promise<boolean> => {
      try {
        const { encrypted } = await getDbEncryptionStatus();
        if (cancelled) return true;
        setState(encrypted ? 'encrypted' : 'unencrypted');
        return true;
      } catch {
        // Thrown = status unknown (likely called before DB init completed).
        // Stay silent; the caller decides whether to retry.
        if (!cancelled) setState('unknown');
        return false;
      }
    };

    const run = async () => {
      const ok = await check();
      if (!ok && !cancelled) {
        // Retry exactly once in case DB init finishes right after first launch.
        retryTimer = setTimeout(() => {
          void check();
        }, RETRY_DELAY_MS);
      }
    };

    void run();

    return () => {
      cancelled = true;
      if (retryTimer) clearTimeout(retryTimer);
    };
  }, []);

  // Only the confirmed-unencrypted case is user-visible. `unknown` (loading or a
  // rejected pre-init call) and `encrypted` (the normal case) render nothing.
  if (state !== 'unencrypted') {
    return null;
  }

  return (
    <div className="px-4 pt-4">
      <Alert
        variant="destructive"
        className="border-amber-400 bg-amber-50"
        aria-live="polite"
      >
        <LockOpen className="h-5 w-5 text-amber-600" />
        <AlertTitle className="text-amber-900 font-semibold">
          Local data is currently stored unencrypted
        </AlertTitle>
        <AlertDescription className="text-amber-800 mt-1">
          The encryption key is unavailable, so Mityu saved your local data
          without at-rest encryption. Mityu will re-encrypt it automatically on
          the next launch once your OS keychain is available.
        </AlertDescription>
      </Alert>
    </div>
  );
}

export default EncryptionStatusBanner;
