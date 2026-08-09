'use client';

/**
 * The speaker-separation pass for one meeting: what state it is in, and the two
 * things a person can ask for.
 *
 * The pass is post-hoc by design (ADR-0034) -- it never runs during capture, so
 * nothing here can disturb a recording in progress. It is also entirely
 * on-device apart from one explicit model download, which is why fetching the
 * models is a separate action a person has to choose rather than something that
 * happens on first view.
 */

import { useCallback, useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';

import type { DiarizationAvailability, SpeakerTurn } from '@/lib/speakerTurns';
import type { TalkTimeState } from '@/components/report/SpeakerTurns';

/**
 * Availability plus the turns themselves.
 *
 * `availability` alone reports only how many turns exist, because that is all
 * the four-state decision needs; the rows are fetched separately so a meeting
 * that was never diarized costs one query instead of two.
 */
export function useDiarization(meetingId: string | undefined) {
  const [state, setState] = useState<TalkTimeState | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    if (!meetingId) {
      setState(null);
      return;
    }
    try {
      const availability = await invoke<DiarizationAvailability>('api_diarization_availability', {
        meetingId,
      });
      if (availability.status !== 'done') {
        setState({ kind: availability.status });
        return;
      }
      // Only now are the rows worth fetching.
      const turns = await invoke<SpeakerTurn[]>('api_get_speaker_turns', { meetingId });
      setState({ kind: 'done', diarizedAt: availability.diarized_at, turns });
    } catch (e) {
      // A failure to ASK is not a state of the pass, so it must not be rendered
      // as one -- reporting it as `noAudio` would tell the user their recording
      // has no audio when the truth is that we could not find out.
      setError(String(e));
      setState(null);
    }
  }, [meetingId]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const downloadModels = useCallback(async () => {
    setBusy(true);
    setError(null);
    try {
      await invoke('api_diarization_download_models');
      await refresh();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }, [refresh]);

  const run = useCallback(async () => {
    if (!meetingId) return;
    setBusy(true);
    setError(null);
    try {
      await invoke<number>('api_diarize_meeting', { meetingId });
      // Deliberately re-read rather than trusting the returned count: zero turns
      // is a real outcome, and the stamp that distinguishes it from "never ran"
      // only exists in the database.
      await refresh();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }, [meetingId, refresh]);

  const turns: SpeakerTurn[] = state?.kind === 'done' ? state.turns : [];

  return { state, turns, busy, error, run, downloadModels, refresh };
}
