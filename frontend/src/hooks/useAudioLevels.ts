'use client';

/**
 * The real capture level, for anything that wants to draw one.
 *
 * WHY THIS EXISTS. The recording pill drew three bars whose heights were
 * `Math.random() * 20 + 10` on a 300ms interval. Users read that as a level meter — it
 * is drawn exactly like one, beside a live recording — so it answered "is it hearing
 * me?" with a number that had nothing to do with the microphone. A meter that moves
 * while the room is silent is worse than no meter: it is a confident wrong answer to
 * the one question a person asks before they trust a recording.
 *
 * The backend already emits `audio-levels` with RMS and peak per device; `DeviceSelection`
 * has been consuming it all along. This is the same subscription, shared.
 */

import { useEffect, useState } from 'react';
import { isTauri } from '@/lib/isTauri';
import type { AudioLevelData, AudioLevelUpdate } from '@/components/DeviceSelection';

export interface AudioLevelsState {
  /** Per device name, newest reading. */
  byDevice: Map<string, AudioLevelData>;
  /** Loudest RMS across devices, 0..1 — what a single meter should draw. */
  rms: number;
  /** Loudest peak across devices, 0..1. */
  peak: number;
  /** True when at least one device reported activity in the last reading. */
  active: boolean;
  /** False until the first reading arrives, so a meter can say "waiting" honestly. */
  hasReading: boolean;
}

const EMPTY: AudioLevelsState = {
  byDevice: new Map(),
  rms: 0,
  peak: 0,
  active: false,
  hasReading: false,
};

export function useAudioLevels(enabled: boolean): AudioLevelsState {
  const [state, setState] = useState<AudioLevelsState>(EMPTY);

  useEffect(() => {
    if (!enabled || !isTauri()) {
      setState(EMPTY);
      return;
    }
    let unlisten: (() => void) | undefined;
    let cancelled = false;

    (async () => {
      try {
        const { listen } = await import('@tauri-apps/api/event');
        const stop = await listen<AudioLevelUpdate>('audio-levels', (event) => {
          const levels = event.payload?.levels ?? [];
          const byDevice = new Map<string, AudioLevelData>();
          let rms = 0;
          let peak = 0;
          let active = false;
          for (const level of levels) {
            byDevice.set(level.device_name, level);
            rms = Math.max(rms, level.rms_level ?? 0);
            peak = Math.max(peak, level.peak_level ?? 0);
            active = active || !!level.is_active;
          }
          setState({ byDevice, rms, peak, active, hasReading: true });
        });
        if (cancelled) stop();
        else unlisten = stop;
      } catch {
        // No level stream is not an error the user needs to see: the recording is
        // unaffected. The meter simply reports that it has no reading.
        if (!cancelled) setState(EMPTY);
      }
    })();

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [enabled]);

  return state;
}
