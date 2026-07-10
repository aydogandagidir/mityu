"use client";

import { Transcript, TranscriptSegmentData } from '@/types';
import { TranscriptView } from '@/components/TranscriptView';
import { VirtualizedTranscriptView } from '@/components/VirtualizedTranscriptView';
import { TranscriptButtonGroup } from './TranscriptButtonGroup';
import { FileText } from 'lucide-react';
import { useMemo } from 'react';

interface TranscriptPanelProps {
  transcripts: Transcript[];
  customPrompt: string;
  onPromptChange: (value: string) => void;
  onCopyTranscript: () => void;
  onOpenMeetingFolder: () => Promise<void>;
  isRecording: boolean;
  disableAutoScroll?: boolean;

  // Optional pagination props (when using virtualization)
  usePagination?: boolean;
  segments?: TranscriptSegmentData[];
  hasMore?: boolean;
  isLoadingMore?: boolean;
  totalCount?: number;
  loadedCount?: number;
  onLoadMore?: () => void;

  // Retranscription props
  meetingId?: string;
  meetingFolderPath?: string | null;
  onRefetchTranscripts?: () => Promise<void>;

  // Jump-to-source (BACKLOG C1.6): plumbed from the summary draft review surface
  // down to the virtualized transcript view. Additive; default undefined = today.
  scrollToSegmentId?: string | null;
  scrollNonce?: number;
  onRequestSegment?: (segmentId: string) => void;
  /** Click-to-play: seek meeting audio to a segment's start time. */
  onSeekToTime?: (sec: number) => void;
}

export function TranscriptPanel({
  transcripts,
  customPrompt,
  onPromptChange,
  onCopyTranscript,
  onOpenMeetingFolder,
  isRecording,
  disableAutoScroll = false,
  usePagination = false,
  segments,
  hasMore,
  isLoadingMore,
  totalCount,
  loadedCount,
  onLoadMore,
  meetingId,
  meetingFolderPath,
  onRefetchTranscripts,
  scrollToSegmentId,
  scrollNonce,
  onRequestSegment,
  onSeekToTime,
}: TranscriptPanelProps) {
  // Convert transcripts to segments if pagination is not used but we want virtualization
  const convertedSegments = useMemo(() => {
    if (usePagination && segments) {
      return segments;
    }
    // Convert transcripts to segments for virtualization
    return transcripts.map(t => ({
      id: t.id,
      timestamp: t.audio_start_time ?? 0,
      endTime: t.audio_end_time,
      text: t.text,
      confidence: t.confidence,
    }));
  }, [transcripts, usePagination, segments]);

  return (
    // Layout-neutral root: width, borders, and responsive show/hide are owned by
    // the wrapper in page-content.tsx so the split can be rebalanced and made
    // responsive/collapsible without threading layout state through every prop.
    <div className="flex w-full h-full min-w-0 bg-background flex-col relative">
      {/* Panel toolbar: identity (icon + title + segment count) on the left,
          transcript actions on the right — replaces the floating centered row. */}
      <div className="flex items-center justify-between gap-3 border-b border-border px-4 py-2.5">
        <div className="flex min-w-0 items-center gap-2">
          <span className="grid h-6 w-6 shrink-0 place-items-center rounded-md bg-accent text-accent-foreground">
            <FileText className="h-3.5 w-3.5" aria-hidden />
          </span>
          <h2 className="truncate text-sm font-semibold text-foreground">Transcript</h2>
          <span className="shrink-0 rounded-full bg-muted px-2 py-0.5 text-xs tabular-nums text-muted-foreground">
            {usePagination ? (totalCount ?? convertedSegments.length) : (transcripts?.length || 0)}
          </span>
        </div>
        <TranscriptButtonGroup
          transcriptCount={usePagination ? (totalCount ?? convertedSegments.length) : (transcripts?.length || 0)}
          onCopyTranscript={onCopyTranscript}
          onOpenMeetingFolder={onOpenMeetingFolder}
          meetingId={meetingId}
          meetingFolderPath={meetingFolderPath}
          onRefetchTranscripts={onRefetchTranscripts}
        />
      </div>

      {/* Transcript content - use virtualized view for better performance */}
      <div className="flex-1 overflow-hidden pb-4">
        <VirtualizedTranscriptView
          segments={convertedSegments}
          isRecording={isRecording}
          isPaused={false}
          isProcessing={false}
          isStopping={false}
          enableStreaming={false}
          showConfidence={true}
          disableAutoScroll={disableAutoScroll}
          hasMore={hasMore}
          isLoadingMore={isLoadingMore}
          totalCount={totalCount}
          loadedCount={loadedCount}
          onLoadMore={onLoadMore}
          scrollToSegmentId={scrollToSegmentId}
          scrollNonce={scrollNonce}
          onRequestSegment={onRequestSegment}
          onSeekToTime={onSeekToTime}
        />
      </div>

      {/* Custom prompt input at bottom of transcript section */}
      {!isRecording && convertedSegments.length > 0 && (
        <div className="border-t border-border p-3">
          <textarea
            placeholder="Add context for the AI summary — people involved, meeting overview, objective…"
            className="min-h-[72px] w-full resize-y rounded-lg border border-input bg-card px-3 py-2 text-sm text-foreground placeholder:text-muted-foreground/70 shadow-sm focus:border-transparent focus:outline-none focus:ring-2 focus:ring-ring"
            value={customPrompt}
            onChange={(e) => onPromptChange(e.target.value)}
          />
        </div>
      )}
    </div>
  );
}
