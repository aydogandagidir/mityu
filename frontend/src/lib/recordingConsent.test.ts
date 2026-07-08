/**
 * Unit tests for `computeConsentGate` (BACKLOG C5).
 *
 * Run with `pnpm test` (Vitest, `environment: 'node'` — see `vitest.config.ts`).
 * These are pure-logic assertions: no DOM, no mocks, no I/O.
 *
 * The gate is the safety-critical decision "show the multi-party consent reminder
 * before this recording starts?". These cases pin the full truth table plus the
 * fail-open default (never skip the prompt when nothing is acknowledged).
 */

import { describe, it, expect } from 'vitest';
import {
  computeConsentGate,
  DEFAULT_RECORDING_CONSENT_STATE,
  type RecordingConsentState,
} from './recordingConsent';

describe('computeConsentGate', () => {
  it('prompts on a fresh install (never acknowledged)', () => {
    expect(computeConsentGate({ acknowledged: false, alwaysAsk: false })).toBe(true);
  });

  it('does not prompt once acknowledged with alwaysAsk off (one-time gate satisfied)', () => {
    expect(computeConsentGate({ acknowledged: true, alwaysAsk: false })).toBe(false);
  });

  it('prompts every time when alwaysAsk is re-armed, even if acknowledged', () => {
    expect(computeConsentGate({ acknowledged: true, alwaysAsk: true })).toBe(true);
  });

  it('prompts when not acknowledged and alwaysAsk is on', () => {
    expect(computeConsentGate({ acknowledged: false, alwaysAsk: true })).toBe(true);
  });

  // Fail-open safety: a store read that returns defaults must never skip the reminder.
  it('prompts for DEFAULT_RECORDING_CONSENT_STATE (fail-open default)', () => {
    expect(computeConsentGate(DEFAULT_RECORDING_CONSENT_STATE)).toBe(true);
  });

  it('is deterministic — same input, same output (no hidden state)', () => {
    const s: RecordingConsentState = { acknowledged: true, alwaysAsk: false };
    expect(computeConsentGate(s)).toBe(computeConsentGate(s));
  });
});
