'use client';

/**
 * Floating Cluely-style Copilot HUD Overlay (DESIGN_SYSTEM.md §6.7, ADR-0038).
 *
 * Wraps the live `CopilotPanel` inside a translucent glassmorphic HUD frame with
 * a top control pill bar (`shadow-elev-3`, `backdrop-blur-md`, crisp `border-border`
 * hairline, and `tabular-nums` live session telemetry) while preserving all
 * honesty and consent invariants:
 * - Never starts or stops capture on its own.
 * - Reports platform screen-sharing posture verbatim from `CopilotStatus`.
 */

import React from 'react';
import { Eye, EyeOff, ShieldAlert, ShieldCheck, Sparkles } from 'lucide-react';
import { CopilotPanel, type CopilotPanelProps } from './CopilotPanel';
import { cn } from '@/lib/utils';
import { focusRing } from '@/components/ui/focus-ring';

export interface CopilotOverlayProps extends CopilotPanelProps {
  /** Elapsed recording duration in seconds, rendered in `tabular-nums`. */
  elapsedSeconds?: number;
  /** Optional handler to hide the floating HUD overlay. */
  onHideOverlay?: () => void;
  className?: string;
}

function formatElapsed(seconds: number): string {
  const safe = Math.max(0, Math.floor(seconds));
  const mins = Math.floor(safe / 60);
  const secs = safe % 60;
  return `${mins.toString().padStart(2, '0')}:${secs.toString().padStart(2, '0')}`;
}

export function CopilotOverlay({
  status,
  elapsedSeconds = 147,
  onHideOverlay,
  className,
  ...panelProps
}: CopilotOverlayProps) {
  const recording = status?.recording ?? false;
  const paused = panelProps.paused ?? false;
  const contentProtection = status?.config.contentProtection ?? false;
  const protectionLevel = status?.protection.level ?? 'unsupported';
  const toggleKeybind =
    status?.config.keybinds.togglePanel.replace('CommandOrControl', 'Ctrl') ?? 'Ctrl+Shift+M';

  const ProtectionIcon = !contentProtection
    ? Eye
    : protectionLevel === 'enforced'
      ? ShieldCheck
      : protectionLevel === 'bestEffort'
        ? ShieldAlert
        : EyeOff;

  const protectionTone = !contentProtection
    ? 'border-warning-border bg-warning-surface text-warning-ink'
    : protectionLevel === 'enforced'
      ? 'border-success-border bg-success-surface text-success-ink'
      : protectionLevel === 'bestEffort'
        ? 'border-warning-border bg-warning-surface text-warning-ink'
        : 'border-border bg-surface-2 text-muted-foreground';

  const protectionLabel = !contentProtection
    ? 'Visible in screen shares'
    : status?.protection.headline ?? 'Not hidden from screen sharing';

  return (
    <div className={cn('inline-flex flex-col items-center gap-2', className)}>
      {/* Top Floating Control Pill Bar */}
      <div
        data-tauri-drag-region
        className="flex items-center gap-2 rounded-full border border-border bg-card/95 px-3 py-1.5 text-card-foreground shadow-elev-3 backdrop-blur-md tabular-nums"
      >
        <span className="inline-flex items-center gap-1.5 text-caption font-medium text-foreground">
          {recording ? (
            <span
              className={cn(
                'size-2 shrink-0',
                paused ? 'bg-warning' : 'rounded-full bg-recording'
              )}
              aria-hidden
            />
          ) : (
            <Sparkles className="h-3 w-3 shrink-0 text-ai-ink" aria-hidden />
          )}
          <span>{recording ? (paused ? 'Paused' : formatElapsed(elapsedSeconds)) : 'Standby'}</span>
        </span>

        <span className="h-3.5 w-px bg-border" aria-hidden />

        <span
          className={cn(
            'inline-flex items-center gap-1 rounded-full border px-2 py-0.5 text-micro font-medium',
            protectionTone
          )}
        >
          <ProtectionIcon className="h-3 w-3 shrink-0" aria-hidden />
          <span className="truncate">{protectionLabel}</span>
        </span>

        {onHideOverlay && (
          <>
            <span className="h-3.5 w-px bg-border" aria-hidden />
            <button
              type="button"
              onClick={onHideOverlay}
              className={cn(
                'inline-flex min-h-[28px] items-center gap-1.5 rounded-full border border-border bg-surface-2 px-2.5 py-0.5 text-caption font-medium text-foreground transition-colors duration-fast ease-out hover:bg-surface-3',
                focusRing('card')
              )}
            >
              <span>Hide</span>
              <kbd className="rounded-xs border border-border bg-card px-1 py-0.5 font-mono text-micro text-muted-foreground">
                {toggleKeybind}
              </kbd>
            </button>
          </>
        )}
      </div>

      {/* Main 380px Translucent Glassmorphic HUD Panel */}
      <div className="h-[460px] w-[380px] overflow-hidden rounded-xl border border-border bg-card/95 shadow-elev-3 backdrop-blur-md">
        <CopilotPanel status={status} {...panelProps} />
      </div>
    </div>
  );
}
