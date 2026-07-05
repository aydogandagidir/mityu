/**
 * Pure unit tests for `computeConsentGate` (BACKLOG C5).
 *
 * NO TEST RUNNER EXISTS in `frontend/` (no jest/vitest in package.json). These
 * are framework-free assertions: type-checked by `pnpm exec tsc --noEmit` and
 * runnable ad-hoc via any TS runner (e.g.
 * `pnpm dlx tsx src/lib/recordingConsent.test.ts`). `runRecordingConsentTests`
 * throws on the first failed assertion and returns the passed-count otherwise.
 *
 * The gate is the safety-critical decision "show the multi-party consent reminder
 * before this recording starts?". These cases pin the full truth table plus the
 * fail-open default (never skip the prompt when nothing is acknowledged).
 */

import {
  computeConsentGate,
  DEFAULT_RECORDING_CONSENT_STATE,
  type RecordingConsentState,
} from './recordingConsent';

function assert(cond: boolean, msg: string): void {
  if (!cond) {
    throw new Error(`computeConsentGate test failed: ${msg}`);
  }
}

export function runRecordingConsentTests(): number {
  let passed = 0;

  // 1. Fresh install (nothing acknowledged) -> must prompt.
  assert(
    computeConsentGate({ acknowledged: false, alwaysAsk: false }) === true,
    'never-acknowledged must prompt',
  );
  passed++;

  // 2. Acknowledged once, not always-ask -> one-time gate satisfied, skip.
  assert(
    computeConsentGate({ acknowledged: true, alwaysAsk: false }) === false,
    'acknowledged + !alwaysAsk must NOT prompt',
  );
  passed++;

  // 3. Acknowledged but always-ask re-armed -> prompt every time.
  assert(
    computeConsentGate({ acknowledged: true, alwaysAsk: true }) === true,
    'alwaysAsk must prompt even when acknowledged',
  );
  passed++;

  // 4. Not acknowledged + always-ask -> prompt.
  assert(
    computeConsentGate({ acknowledged: false, alwaysAsk: true }) === true,
    '!acknowledged + alwaysAsk must prompt',
  );
  passed++;

  // 5. The exported default state must be a "prompt" state (fail-open safety:
  //    a store read that returns defaults must never skip the reminder).
  assert(
    computeConsentGate(DEFAULT_RECORDING_CONSENT_STATE) === true,
    'DEFAULT_RECORDING_CONSENT_STATE must prompt',
  );
  passed++;

  // 6. Determinism: same input -> same output (no hidden state).
  const s: RecordingConsentState = { acknowledged: true, alwaysAsk: false };
  assert(
    computeConsentGate(s) === computeConsentGate(s),
    'gate must be deterministic',
  );
  passed++;

  return passed;
}

// Auto-run when executed directly through a TS runner (parity with the C4 tests).
runRecordingConsentTests();
