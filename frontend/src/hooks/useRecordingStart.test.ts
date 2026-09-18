/**
 * The recording-start gate (the bug that stopped the product working).
 *
 * `useRecordingStart` asked `parakeet_has_available_models` on all three start
 * paths, whatever engine the user had configured. A Local Whisper user with a
 * working Whisper model was told "Transcription model not ready", shown the
 * model picker, and could never start a recording — while the Rust check
 * further down the same path would have accepted it.
 *
 * The engine decision now lives in `api_transcription_readiness` and nowhere
 * else. These tests pin that: the source must not ask one engine about
 * another's readiness, and the refusal the user sees must be the backend's own
 * sentence rather than one composed here.
 */

import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import { readinessMessage } from './useRecordingStart';
import type { TranscriptionReadiness } from '@/types';

const SOURCE = readFileSync(join(__dirname, 'useRecordingStart.ts'), 'utf8');

describe('the start path asks the configured engine, not a fixed one', () => {
  it('never calls an engine-specific readiness command', () => {
    for (const engineSpecific of [
      'parakeet_has_available_models',
      'parakeet_get_available_models',
      'whisper_has_available_models',
      'whisper_get_available_models',
      'parakeet_init',
      'whisper_init',
    ]) {
      expect(
        SOURCE.includes(engineSpecific),
        `${engineSpecific} decides readiness for one engine; the start path must ask api_transcription_readiness`,
      ).toBe(false);
    }
  });

  it('asks the engine-aware backend command', () => {
    expect(SOURCE).toContain('api_transcription_readiness');
  });

  /**
   * A pre-flight that cannot run must not become a refusal: the authoritative
   * validation still runs in the backend when recording starts, so proceeding
   * is safe and blocking is the silent failure we set out to remove.
   */
  it('treats an unreadable pre-flight as ready rather than blocking', () => {
    expect(SOURCE).toMatch(/catch[\s\S]{0,320}ready:\s*true/);
  });
});

describe('what the user is told', () => {
  const base: TranscriptionReadiness = {
    ready: false,
    provider: 'localWhisper',
    downloading: false,
    reason: null,
  };

  it('renders the backend sentence verbatim', () => {
    const reason = 'No Local Whisper model is installed yet. Download one in Settings before recording.';
    expect(readinessMessage({ ...base, reason }).description).toBe(reason);
  });

  it('says wait, not install, while a model is downloading', () => {
    const downloading = readinessMessage({
      ...base,
      downloading: true,
      reason: 'The Local Whisper model is still downloading.',
    });
    expect(downloading.kind).toBe('downloading');
    expect(downloading.title).toMatch(/download/i);
  });

  it('routes a missing model to the picker path', () => {
    expect(readinessMessage({ ...base, reason: 'No model' }).kind).toBe('missing');
  });

  /** Never a blank card: a refusal with no sentence still says something. */
  it('supplies a sentence only when the backend sent none', () => {
    expect(readinessMessage(base).description).toBeTruthy();
  });
});

describe('a start that does not happen is never silent', () => {
  it('tells the user when consent was not confirmed', () => {
    const cancels = SOURCE.match(/cancelled at consent gate/g) ?? [];
    expect(cancels.length).toBe(3);
    // Every one of the three paths pairs its log with something visible.
    for (const path of ['Recording start cancelled', 'Auto-start cancelled', 'Direct start cancelled']) {
      const idx = SOURCE.indexOf(path);
      expect(idx, path).toBeGreaterThan(-1);
      expect(SOURCE.slice(idx, idx + 400)).toContain('Recording not started');
    }
  });

  it('never tells the user to check a console they cannot open', () => {
    expect(SOURCE).not.toContain('alert(');
    expect(SOURCE).not.toContain('Check console for details');
  });
});
