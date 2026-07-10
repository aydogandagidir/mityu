'use client';

/**
 * PlaybackBar — compact audio playback strip for the meeting report (Phase B/D,
 * docs/DESIGN_READAI.md): play/pause + scrubber + time, backed by the meeting's
 * local audio.mp4 through Tauri's asset protocol. Everything stays on-device.
 *
 * Exposes an imperative `seekTo(sec)` handle so transcript segments and chapter
 * blocks can jump the audio. Renders nothing outside Tauri, when the meeting has
 * no folder, or when the file fails to load (e.g. a custom recordings dir outside
 * the asset-protocol scope) — playback is an enhancement, never a blocker.
 */

import {
  forwardRef,
  useEffect,
  useImperativeHandle,
  useMemo,
  useRef,
  useState,
} from 'react';
import { Pause, Play } from 'lucide-react';
import { convertFileSrc } from '@tauri-apps/api/core';
import { isTauri } from '@/lib/isTauri';

export interface PlaybackBarHandle {
  /** Seek to a position (seconds from recording start) and start playing. */
  seekTo: (sec: number) => void;
}

function fmt(sec: number): string {
  if (!Number.isFinite(sec) || sec < 0) return '0:00';
  const m = Math.floor(sec / 60);
  const s = Math.floor(sec % 60);
  const h = Math.floor(m / 60);
  if (h > 0) return `${h}:${String(m % 60).padStart(2, '0')}:${String(s).padStart(2, '0')}`;
  return `${m}:${String(s).padStart(2, '0')}`;
}

export const PlaybackBar = forwardRef<PlaybackBarHandle, { folderPath?: string | null }>(
  function PlaybackBar({ folderPath }, ref) {
    const audioRef = useRef<HTMLAudioElement | null>(null);
    const [playing, setPlaying] = useState(false);
    const [current, setCurrent] = useState(0);
    const [duration, setDuration] = useState(0);
    const [failed, setFailed] = useState(false);

    const src = useMemo(() => {
      if (!folderPath || !isTauri()) return null;
      try {
        return convertFileSrc(`${folderPath}/audio.mp4`);
      } catch {
        return null;
      }
    }, [folderPath]);

    useImperativeHandle(ref, () => ({
      seekTo: (sec: number) => {
        const el = audioRef.current;
        if (!el) return;
        el.currentTime = Math.max(0, sec);
        void el.play().catch(() => setFailed(true));
      },
    }));

    // Reset state when the meeting (src) changes.
    useEffect(() => {
      setPlaying(false);
      setCurrent(0);
      setDuration(0);
      setFailed(false);
    }, [src]);

    if (!src || failed) return null;

    return (
      <div className="flex items-center gap-3 border-b border-border bg-background px-6 py-2">
        {/* Keep the element mounted even while metadata loads */}
        <audio
          ref={audioRef}
          src={src}
          preload="metadata"
          onLoadedMetadata={(e) => setDuration(e.currentTarget.duration || 0)}
          onTimeUpdate={(e) => setCurrent(e.currentTarget.currentTime)}
          onPlay={() => setPlaying(true)}
          onPause={() => setPlaying(false)}
          onEnded={() => setPlaying(false)}
          onError={() => setFailed(true)}
        />
        <button
          type="button"
          onClick={() => {
            const el = audioRef.current;
            if (!el) return;
            if (el.paused) void el.play().catch(() => setFailed(true));
            else el.pause();
          }}
          title={playing ? 'Pause' : 'Play recording'}
          aria-label={playing ? 'Pause' : 'Play recording'}
          className="grid h-8 w-8 shrink-0 place-items-center rounded-full bg-primary text-primary-foreground shadow-sm transition-colors hover:bg-primary/90"
        >
          {playing ? <Pause className="h-4 w-4" /> : <Play className="ml-0.5 h-4 w-4" />}
        </button>
        <span className="w-12 shrink-0 text-right text-xs tabular-nums text-muted-foreground">{fmt(current)}</span>
        <input
          type="range"
          min={0}
          max={duration || 0}
          step={0.1}
          value={Math.min(current, duration || 0)}
          onChange={(e) => {
            const el = audioRef.current;
            if (el) el.currentTime = Number(e.target.value);
          }}
          aria-label="Seek"
          className="h-1.5 flex-1 cursor-pointer appearance-none rounded-full bg-muted accent-[hsl(var(--primary))]"
        />
        <span className="w-12 shrink-0 text-xs tabular-nums text-muted-foreground">{fmt(duration)}</span>
      </div>
    );
  },
);
