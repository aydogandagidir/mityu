'use client';

/**
 * LicensingContext (ADR-0023)
 *
 * Owns the licensing status snapshot and the single, app-level activate/paywall
 * dialog (same shape as `RecordingConsentContext`: the provider renders the
 * prop-driven dialog as a sibling of `children`).
 *
 * - Fetches the status once on mount. Outside the Tauri shell the service
 *   short-circuits to a safe "licensed, not configured" stub, so browser and
 *   design routes render with zero licensing chrome and no `invoke` call.
 * - `activate` / `deactivate` update the context state from the status the
 *   backend returns, so every consumer (banner, settings) refreshes at once.
 * - `openActivateDialog({ paywall: true })` is the paywall entry point used
 *   when a gated command (recording start / audio import) rejects with a
 *   LICENSE_REQUIRED error.
 *
 * Local-first: reading the status never blocks on the network (the backend's
 * re-validation is fire-and-forget on its side, per ADR-0023 §7).
 */

import React, {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
} from 'react';
import { ActivateLicenseDialog } from '@/components/licensing/ActivateLicenseDialog';
import { licensingService } from '@/services/licensingService';
import type { LicensingStatus } from '@/types/licensing';

export interface OpenActivateDialogOptions {
  /** Open in paywall mode (a gated action was just rejected with LICENSE_REQUIRED). */
  paywall?: boolean;
}

interface LicensingContextType {
  /** Latest licensing snapshot; `null` until the first fetch resolves. */
  status: LicensingStatus | null;
  /** Re-fetch the status from the backend. */
  refresh: () => Promise<void>;
  /** Activate a license key; updates `status` and resolves with it. Rejects with a string error. */
  activate: (key: string) => Promise<LicensingStatus>;
  /** Deactivate this device's license (frees the seat); updates `status`. Rejects with a string error. */
  deactivate: () => Promise<LicensingStatus>;
  /** Open the shared activate-license dialog (optionally in paywall mode). */
  openActivateDialog: (options?: OpenActivateDialogOptions) => void;
}

const LicensingContext = createContext<LicensingContextType | null>(null);

export function useLicensing(): LicensingContextType {
  const ctx = useContext(LicensingContext);
  if (!ctx) {
    throw new Error('useLicensing must be used within a LicensingProvider');
  }
  return ctx;
}

export function LicensingProvider({ children }: { children: React.ReactNode }) {
  const [status, setStatus] = useState<LicensingStatus | null>(null);
  const [dialogOpen, setDialogOpen] = useState(false);
  const [dialogPaywall, setDialogPaywall] = useState(false);

  const refresh = useCallback(async () => {
    try {
      // Outside Tauri the service resolves a stub without touching invoke().
      setStatus(await licensingService.getLicensingStatus());
    } catch (error) {
      // Fail quiet: with no status the UI shows no licensing chrome; the
      // capture gates are enforced by the backend regardless.
      console.error('[Licensing] Failed to load licensing status:', error);
    }
  }, []);

  // Fetch once on mount.
  useEffect(() => {
    void refresh();
  }, [refresh]);

  const activate = useCallback(async (key: string): Promise<LicensingStatus> => {
    const next = await licensingService.activateLicense(key);
    setStatus(next);
    return next;
  }, []);

  const deactivate = useCallback(async (): Promise<LicensingStatus> => {
    const next = await licensingService.deactivateLicense();
    setStatus(next);
    return next;
  }, []);

  const openActivateDialog = useCallback((options?: OpenActivateDialogOptions) => {
    setDialogPaywall(options?.paywall === true);
    setDialogOpen(true);
  }, []);

  const value = useMemo(
    () => ({ status, refresh, activate, deactivate, openActivateDialog }),
    [status, refresh, activate, deactivate, openActivateDialog],
  );

  return (
    <LicensingContext.Provider value={value}>
      {children}
      <ActivateLicenseDialog
        open={dialogOpen}
        onOpenChange={setDialogOpen}
        paywall={dialogPaywall}
        activate={activate}
      />
    </LicensingContext.Provider>
  );
}
