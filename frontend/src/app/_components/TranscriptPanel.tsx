import { VirtualizedTranscriptView } from '@/components/VirtualizedTranscriptView';
import { PermissionWarning } from '@/components/PermissionWarning';
import { Button } from '@/components/ui/button';
import { ButtonGroup } from '@/components/ui/button-group';
import { InlinePiiBadge, swatchFor } from '@/components/report/SpeakerTurns';
import { parseNemotronModelConfig } from '@/components/TranscriptSettings';
import { Copy, GlobeIcon, Users } from 'lucide-react';
import { useTranscripts } from '@/contexts/TranscriptContext';
import { useConfig } from '@/contexts/ConfigContext';
import { useRecordingState } from '@/contexts/RecordingStateContext';
import { usePermissionCheck } from '@/hooks/usePermissionCheck';
import { ModalType } from '@/hooks/useModalState';
import { useIsLinux } from '@/hooks/usePlatform';
import { formatTalkTime, hasCrosstalk, talkTime } from '@/lib/speakerTurns';
import { useMemo } from 'react';

/**
 * TranscriptPanel Component
 *
 * Displays live and recorded transcript content with multi-speaker attribution pills,
 * Sortformer overlap indicator badge, inline [REDACTED_*] PII guard chips, and
 * controls for copying and language settings.
 */

interface TranscriptPanelProps {
  // indicates stop-processing state for transcripts; derived from backend statuses.
  isProcessingStop: boolean;
  isStopping: boolean;
  showModal: (name: ModalType, message?: string) => void;
}

function formatRecordingTimestamp(seconds: number | undefined): string {
  if (seconds === undefined || !Number.isFinite(seconds)) return '[00:00]';
  const totalSeconds = Math.max(0, Math.floor(seconds));
  const mins = String(Math.floor(totalSeconds / 60)).padStart(2, '0');
  const secs = String(totalSeconds % 60).padStart(2, '0');
  return `[${mins}:${secs}]`;
}

export function TranscriptPanel({
  isProcessingStop,
  isStopping,
  showModal,
}: TranscriptPanelProps) {
  // Contexts
  const { transcripts, liveSpeakerTurns, transcriptContainerRef, copyTranscript } = useTranscripts();
  const { transcriptModelConfig } = useConfig();
  const { isRecording, isPaused } = useRecordingState();
  const { checkPermissions, isChecking, hasSystemAudio, hasMicrophone } = usePermissionCheck();
  const isLinux = useIsLinux();

  // Convert transcripts to segments for virtualized view
  const segments = useMemo(
    () =>
      transcripts.map((t) => ({
        id: t.id,
        timestamp: t.audio_start_time ?? 0,
        endTime: t.audio_end_time,
        text: t.text,
        confidence: t.confidence,
      })),
    [transcripts]
  );

  const speakerRows = useMemo(() => talkTime(liveSpeakerTurns), [liveSpeakerTurns]);
  const speakerOrder = useMemo(() => speakerRows.map((r) => r.label), [speakerRows]);

  const overlapCount = useMemo(
    () =>
      segments.filter((seg) =>
        hasCrosstalk({ start: seg.timestamp, end: seg.endTime }, liveSpeakerTurns)
      ).length,
    [segments, liveSpeakerTurns]
  );

  const redactedTokens = useMemo(() => {
    const matches = transcripts.flatMap(
      (t) => t.text.match(/\[REDACTED(?:_[A-Z0-9_]+)?\]/g) ?? []
    );
    return Array.from(new Set(matches));
  }, [transcripts]);

  const isNemotron = transcriptModelConfig.provider === 'nemotron';
  const nemotronCfg = useMemo(
    () => (isNemotron ? parseNemotronModelConfig(transcriptModelConfig.model) : null),
    [isNemotron, transcriptModelConfig.model]
  );

  const latestTimestamp =
    segments.length > 0 ? segments[segments.length - 1].timestamp : undefined;

  const showLiveAttributionBar =
    isNemotron || speakerRows.length > 0 || overlapCount > 0 || redactedTokens.length > 0;

  return (
    <div
      ref={transcriptContainerRef}
      className="flex w-full flex-col overflow-y-auto border-r border-border bg-card"
    >
      {/* Title area - Sticky header */}
      <div className="sticky top-0 z-10 border-b border-border bg-card/95 px-4 py-3 backdrop-blur">
        <div className="flex flex-col gap-2.5">
          <div className="flex flex-wrap items-center justify-between gap-2">
            <div className="flex items-center gap-2">
              <span className="text-label font-semibold text-foreground">Live Transcript</span>
              <span className="rounded-full border border-border bg-surface-2 px-2 py-0.5 font-mono text-micro tabular-nums text-muted-foreground">
                {segments.length}
              </span>
              <span
                className="font-mono text-micro tabular-nums text-subtle-foreground"
                title="Latest segment timestamp"
              >
                {formatRecordingTimestamp(latestTimestamp)}
              </span>
            </div>

            <div className="flex items-center gap-2">
              <ButtonGroup>
                {transcripts?.length > 0 && (
                  <Button
                    variant="outline"
                    size="sm"
                    onClick={copyTranscript}
                    title="Copy Transcript"
                  >
                    <Copy />
                    <span className="hidden md:inline">Copy</span>
                  </Button>
                )}
                {(transcriptModelConfig.provider === 'localWhisper' ||
                  transcriptModelConfig.provider === 'nemotron') && (
                  <Button
                    variant="outline"
                    size="sm"
                    onClick={() => showModal('languageSettings')}
                    title="Language"
                  >
                    <GlobeIcon />
                    <span className="hidden md:inline">Language</span>
                  </Button>
                )}
              </ButtonGroup>
            </div>
          </div>

          {showLiveAttributionBar && (
            <div
              className="flex flex-wrap items-center gap-1.5"
              aria-label="Live speaker attribution and redaction status"
            >
              {speakerRows.length > 0 ? (
                speakerRows.map((row) => {
                  const swatch = swatchFor(row.label, speakerOrder);
                  return (
                    <span
                      key={row.label}
                      className="inline-flex items-center gap-1.5 rounded-full border border-border bg-surface-2 px-2 py-0.5 text-micro font-medium tabular-nums text-foreground"
                    >
                      <span
                        className={`h-1.5 w-1.5 shrink-0 rounded-full ${swatch.dot}`}
                        aria-hidden="true"
                      />
                      <span>{row.label}</span>
                      <span className="font-mono text-subtle-foreground">
                        {formatTalkTime(row.ms)}
                      </span>
                    </span>
                  );
                })
              ) : (
                nemotronCfg?.speaker_attribution && (
                  <span className="inline-flex items-center gap-1.5 rounded-full border border-border bg-surface-2 px-2 py-0.5 text-micro font-medium tabular-nums text-muted-foreground">
                    <Users className="h-3 w-3 shrink-0" aria-hidden="true" />
                    <span>Speaker 1–8 Live Diarization</span>
                  </span>
                )
              )}

              {(overlapCount > 0 || nemotronCfg?.speaker_attribution) && (
                <span
                  className="inline-flex items-center gap-1 rounded-full border border-warning-border bg-warning-surface px-2 py-0.5 text-micro font-medium tabular-nums text-warning-ink"
                  title="Simultaneous speech overlap detection"
                >
                  {overlapCount > 0 ? `Overlap · ${overlapCount}` : 'Overlap'}
                </span>
              )}

              {redactedTokens.length > 0
                ? redactedTokens.map((token) => <InlinePiiBadge key={token} token={token} />)
                : nemotronCfg?.inline_pii_redaction && <InlinePiiBadge token="[REDACTED_*]" />}
            </div>
          )}
        </div>
      </div>

      {/* Permission Warning - Not needed on Linux */}
      {!isRecording && !isChecking && !isLinux && (
        <div className="flex justify-center px-4 pt-4">
          <PermissionWarning
            hasMicrophone={hasMicrophone}
            hasSystemAudio={hasSystemAudio}
            onRecheck={checkPermissions}
            isRechecking={isChecking}
          />
        </div>
      )}

      {/* Transcript content */}
      <div className="pb-20">
        <div className="flex justify-center">
          <div className="w-2/3 max-w-[750px]">
            <VirtualizedTranscriptView
              segments={segments}
              speakerTurns={liveSpeakerTurns}
              isRecording={isRecording}
              isPaused={isPaused}
              isProcessing={isProcessingStop}
              isStopping={isStopping}
              enableStreaming={isRecording}
              showConfidence={true}
            />
          </div>
        </div>
      </div>
    </div>
  );
}
