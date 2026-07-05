/**
 * Recording-consent persistence + gate logic (BACKLOG C5).
 *
 * A local-first, pre-recording multi-party consent acknowledgment. Mirrors the
 * `analytics.json` plugin-store pattern (see AnalyticsProvider /
 * AnalyticsConsentSwitch) so it works fully offline with no new dependency.
 *
 * Two persisted keys in `recording-consent.json`:
 *  - `acknowledged` (default false): the user has confirmed, at least once on
 *    this device, that they are responsible for informing participants. This is
 *    what makes the gate a ONE-TIME prompt by default.
 *  - `alwaysAsk` (default false): when true, the reminder is shown before EVERY
 *    recording regardless of `acknowledged` (user re-arms the gate from Settings).
 *
 * The store call surface is deliberately tiny and every read fail-opens to the
 * SAFE side (prompt the user) — a store error must never silently start a
 * recording without showing the consent reminder.
 */

import { load } from '@tauri-apps/plugin-store';

const STORE_FILE = 'recording-consent.json';
const KEY_ACKNOWLEDGED = 'recordingConsentAcknowledged';
const KEY_ALWAYS_ASK = 'recordingConsentAlwaysAsk';

/** Snapshot of the two persisted consent flags. */
export interface RecordingConsentState {
  /** User has confirmed participant-consent responsibility at least once. */
  acknowledged: boolean;
  /** User wants the reminder before every recording (re-armed gate). */
  alwaysAsk: boolean;
}

export const DEFAULT_RECORDING_CONSENT_STATE: RecordingConsentState = {
  acknowledged: false,
  alwaysAsk: false,
};

/**
 * Pure gate decision — the single source of truth for "must we show the consent
 * dialog before this recording starts?". Kept pure (no I/O) so it is unit-testable
 * without a Tauri runtime.
 *
 * Show the dialog when the user has never acknowledged, OR when they have opted
 * into being reminded every time. Only a prior acknowledgment WITHOUT always-ask
 * skips the prompt.
 */
export function computeConsentGate(state: RecordingConsentState): boolean {
  return state.alwaysAsk || !state.acknowledged;
}

async function openStore() {
  return load(STORE_FILE, {
    autoSave: false,
    defaults: {
      [KEY_ACKNOWLEDGED]: false,
      [KEY_ALWAYS_ASK]: false,
    },
  });
}

/**
 * Read both flags. On any store failure, returns the DEFAULT state
 * (acknowledged=false), which makes {@link computeConsentGate} return true — i.e.
 * we fail toward showing the consent reminder, never toward silently recording.
 */
export async function readRecordingConsentState(): Promise<RecordingConsentState> {
  try {
    const store = await openStore();
    const acknowledged = (await store.get<boolean>(KEY_ACKNOWLEDGED)) ?? false;
    const alwaysAsk = (await store.get<boolean>(KEY_ALWAYS_ASK)) ?? false;
    return { acknowledged, alwaysAsk };
  } catch (error) {
    console.error('[recordingConsent] Failed to read consent state:', error);
    return { ...DEFAULT_RECORDING_CONSENT_STATE };
  }
}

/**
 * Should the consent dialog be shown before starting a recording right now?
 * Convenience wrapper: read state, then apply the pure gate. Fail-open to `true`.
 */
export async function shouldPromptBeforeRecording(): Promise<boolean> {
  const state = await readRecordingConsentState();
  return computeConsentGate(state);
}

/** Persist the one-time acknowledgment flag (set on dialog confirm). */
export async function setRecordingConsentAcknowledged(acknowledged: boolean): Promise<void> {
  const store = await openStore();
  await store.set(KEY_ACKNOWLEDGED, acknowledged);
  await store.save();
}

/** Persist the "ask before every recording" flag (Settings toggle). */
export async function setRecordingConsentAlwaysAsk(alwaysAsk: boolean): Promise<void> {
  const store = await openStore();
  await store.set(KEY_ALWAYS_ASK, alwaysAsk);
  await store.save();
}
