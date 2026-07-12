/**
 * Checkout / "Buy Mityu Pro" destination (ADR-0023).
 *
 * The purchase URL is build-time configuration, injected via
 * `NEXT_PUBLIC_MITYU_CHECKOUT_URL` (Next.js inlines `NEXT_PUBLIC_*` at build
 * time — the frontend twin of the Rust `MITYU_POLAR_ORG_ID` `option_env!`).
 * When it is unset the button falls back to the live public pricing page, which
 * is where a buyer reaches checkout today — so the button is always truthful and
 * never dead. At 1.0 the Polar product checkout URL is injected and the button
 * jumps straight there.
 */

import { isTauri } from '@/lib/isTauri';
import { openExternalUrl } from '@/services/systemService';

/** Live public pricing page — the destination until a Polar checkout URL is injected. */
const FALLBACK_CHECKOUT_URL = 'https://mityu.vercel.app/#pricing';

/** Resolved at build time, trimmed; falls back to the live pricing page when unset. */
export const CHECKOUT_URL =
  process.env.NEXT_PUBLIC_MITYU_CHECKOUT_URL?.trim() || FALLBACK_CHECKOUT_URL;

/**
 * Open the purchase page in the user's default browser, outside the app webview.
 *
 * Fire-and-forget from an `onClick`: it logs on failure and never throws. In the
 * Tauri shell it uses the OS "open external URL" command; on a plain browser
 * (design/dev routes) it falls back to `window.open`.
 */
export function openCheckout(): void {
  if (isTauri()) {
    openExternalUrl(CHECKOUT_URL).catch((error) => {
      console.error('[checkout] failed to open external URL', error);
    });
  } else if (typeof window !== 'undefined') {
    window.open(CHECKOUT_URL, '_blank', 'noopener,noreferrer');
  }
}
