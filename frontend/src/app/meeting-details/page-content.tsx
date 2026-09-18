"use client";
import { useState, useEffect, useRef } from 'react';
import { motion } from 'framer-motion';
import { Summary, SummaryResponse } from '@/types';
import { useSidebar } from '@/components/Sidebar/SidebarProvider';
import Analytics from '@/lib/analytics';
import { invoke } from '@tauri-apps/api/core';
import { toast } from 'sonner';
import { TranscriptPanel } from '@/components/MeetingDetails/TranscriptPanel';
import { SummaryPanel } from '@/components/MeetingDetails/SummaryPanel';
import { ModelConfig } from '@/components/ModelSettingsModal';
import { FileText } from 'lucide-react';
import { ReportHeader } from '@/components/report/ReportHeader';
import { TopicsTimeline } from '@/components/report/TopicsTimeline';
import { PlaybackBar, PlaybackBarHandle } from '@/components/report/PlaybackBar';
import { EvidenceDrawer } from '@/components/report/EvidenceDrawer';
import { Button } from '@/components/ui/button';

// Custom hooks
import { useMeetingData } from '@/hooks/meeting-details/useMeetingData';
import { useSummaryGeneration } from '@/hooks/meeting-details/useSummaryGeneration';
import { useTemplates } from '@/hooks/meeting-details/useTemplates';
import { useCopyOperations } from '@/hooks/meeting-details/useCopyOperations';
import { useMeetingOperations } from '@/hooks/meeting-details/useMeetingOperations';
import { useConfig } from '@/contexts/ConfigContext';
import { useTour } from '@/components/tour';
import { TOUR_ANCHORS } from '@/lib/tour';

export default function PageContent({
  meeting,
  summaryData,
  initialSegmentId,
  initialJumpId,
  shouldAutoGenerate = false,
  onAutoGenerateComplete,
  onMeetingUpdated,
  onRefetchTranscripts,
  // Pagination props for efficient transcript loading
  segments,
  hasMore,
  isLoadingMore,
  totalCount,
  loadedCount,
  onLoadMore,
}: {
  meeting: any;
  summaryData: Summary | null;
  initialSegmentId?: string | null;
  initialJumpId?: string | null;
  shouldAutoGenerate?: boolean;
  onAutoGenerateComplete?: () => void;
  onMeetingUpdated?: () => Promise<void>;
  onRefetchTranscripts?: () => Promise<void>;
  // Pagination props
  segments?: any[];
  hasMore?: boolean;
  isLoadingMore?: boolean;
  totalCount?: number;
  loadedCount?: number;
  onLoadMore?: () => void;
}) {
  console.log('📄 PAGE CONTENT: Initializing with data:', {
    meetingId: meeting.id,
    summaryDataKeys: summaryData ? Object.keys(summaryData) : null,
    transcriptsCount: meeting.transcripts?.length
  });

  // State
  const [customPrompt, setCustomPrompt] = useState<string>('');
  const [isRecording] = useState(false);
  const [summaryResponse] = useState<SummaryResponse | null>(null);

  // BACKLOG C1.6 — jump-to-source: the source_chunk_id the review surface asked
  // to reveal, plus a nonce so repeat clicks on the same source re-trigger the
  // scroll+flash. `source_chunk_id` shares the transcripts-table row id space,
  // so it is used directly as the target segment id.
  const [scrollToSegmentId, setScrollToSegmentId] = useState<string | null>(null);
  const [scrollNonce, setScrollNonce] = useState(0);

  // The report is ONE document (docs/DESIGN_READAI.md:45-50) and the transcript is
  // evidence you open beside a claim. What used to be a transcript-primary split pane
  // with a collapsible, capped summary — plus a tab bar on narrow windows that hid one
  // of them outright — is now a single column with a drawer.
  //
  // 🔒 Both panes stay MOUNTED at all times, exactly as before: unmounting the
  // transcript loses its scroll position and breaks jump-to-source, which retries its
  // scroll after pagination brings the segment in. The drawer hides by width, never by
  // unmounting.
  const [isEvidenceOpen, setIsEvidenceOpen] = useState(false);
  const evidenceToggleRef = useRef<HTMLButtonElement>(null);

  // Product-tour step 2 points at a source-linked summary block. Reveal the
  // summary before the coach-mark looks for it: expand it if collapsed and, on
  // narrow windows, switch to the summary tab. Without this the block can be
  // display:hidden, and the step would land centered instead of on it.
  const { activeAnchor } = useTour();
  useEffect(() => {
    if (activeAnchor === TOUR_ANCHORS.transcriptPanel) {
      // The coach-mark needs its anchor VISIBLE, not merely mounted: the transcript
      // lives in the drawer now, so step 1 opens it.
      setIsEvidenceOpen(true);
    }
  }, [activeAnchor]);

  // Evidence-search deep links reuse the existing C1.6 jump-to-source path.
  // Reset the target when the meeting changes so a prior search hit cannot be
  // retried against a different meeting; on narrow screens always reveal the
  // transcript tab before the virtualized view scrolls to the source.
  useEffect(() => {
    setScrollToSegmentId(initialSegmentId ?? null);
    if (initialSegmentId) {
      // A deep link from search, the Action Center or Home names a segment: show it.
      setIsEvidenceOpen(true);
      setScrollNonce((nonce) => nonce + 1);
    }
  }, [initialSegmentId, initialJumpId, meeting.id]);

  // Ref to store the modal open function from SummaryGeneratorButtonGroup
  const openModelSettingsRef = useRef<(() => void) | null>(null);

  // Phase D (playback sync): imperative handle into the meeting's audio bar so
  // transcript timestamps and chapter blocks can click-to-play.
  const playbackRef = useRef<PlaybackBarHandle>(null);
  const handleSeekToTime = (sec: number) => playbackRef.current?.seekTo(sec);

  // Sidebar context
  const { serverAddress } = useSidebar();

  // Get model config + beta features from ConfigContext
  const { modelConfig, setModelConfig } = useConfig();

  // Custom hooks
  const meetingData = useMeetingData({ meeting, summaryData, onMeetingUpdated });
  const templates = useTemplates();

  // Callback to register the modal open function
  const handleRegisterModalOpen = (openFn: () => void) => {
    console.log('📝 Registering modal open function in PageContent');
    openModelSettingsRef.current = openFn;
  };

  // Callback to trigger modal open (called from error handler)
  const handleOpenModelSettings = () => {
    console.log('🔔 Opening model settings from PageContent');
    if (openModelSettingsRef.current) {
      openModelSettingsRef.current();
    } else {
      console.warn('⚠️ Modal open function not yet registered');
    }
  };

  // Only evidence-linked rows enter the HITL review surface. Pre-v1.0.4 summaries
  // remain visible through an explicitly unverified, read-only upgrade view.
  const structuredEnabled = meetingData.hasSummaryDraft;

  // BACKLOG C1.6 — jump from a draft block/action item to its transcript segment.
  const handleJumpToSource = (sourceChunkId: string) => {
    setScrollToSegmentId(sourceChunkId);
    setScrollNonce((n) => n + 1);
    // Scrolling a hidden pane happens out of sight, so opening a source opens the
    // evidence beside the claim. Without this the control silently does nothing.
    setIsEvidenceOpen(true);
  };

  // The target segment isn't in the loaded page: pull the next page so the
  // transcript view can retry the scroll once it arrives.
  const handleRequestSegment = () => {
    if (hasMore && !isLoadingMore) {
      onLoadMore?.();
    }
  };

  // Save model config to backend database and sync via event
  const handleSaveModelConfig = async (config?: ModelConfig) => {
    if (!config) return;
    try {
      await invoke('api_save_model_config', {
        provider: config.provider,
        model: config.model,
        whisperModel: config.whisperModel,
        apiKey: config.apiKey ?? null,
        ollamaEndpoint: config.ollamaEndpoint ?? null,
      });

      // Emit event so ConfigContext and other listeners stay in sync
      const { emit } = await import('@tauri-apps/api/event');
      await emit('model-config-updated', config);

      toast.success('Model settings saved successfully');
    } catch (error) {
      console.error('Failed to save model config:', error);
      toast.error('Failed to save model settings');
    }
  };

  const summaryGeneration = useSummaryGeneration({
    meeting,
    transcripts: meetingData.transcripts,
    modelConfig: modelConfig,
    isModelConfigLoading: false, // ConfigContext loads on mount
    selectedTemplate: templates.selectedTemplate,
    onMeetingUpdated,
    updateMeetingTitle: meetingData.updateMeetingTitle,
    setAiSummary: meetingData.setAiSummary,
    onOpenModelSettings: handleOpenModelSettings,
    // Rust enforces structured drafts; this compatibility field stays true while
    // older clients and generated bindings still carry it.
    structuredSummaries: true,
    onStructuredGenerated: meetingData.refetchDraft,
  });

  const copyOperations = useCopyOperations({
    meeting,
    meetingTitle: meetingData.meetingTitle,
  });

  const meetingOperations = useMeetingOperations({
    meeting,
  });

  // Track page view
  useEffect(() => {
    Analytics.trackPageView('meeting_details');
  }, []);

  // Auto-generate summary when flag is set
  useEffect(() => {
    let cancelled = false;

    const autoGenerate = async () => {
      if (shouldAutoGenerate && meetingData.transcripts.length > 0 && !cancelled) {
        console.log(`🤖 Auto-generating summary with ${modelConfig.provider}/${modelConfig.model}...`);
        await summaryGeneration.handleGenerateSummary('');

        // Notify parent that auto-generation is complete (only if not cancelled)
        if (onAutoGenerateComplete && !cancelled) {
          onAutoGenerateComplete();
        }
      }
    };

    autoGenerate();

    // Cleanup: cancel if component unmounts or meeting changes
    return () => {
      cancelled = true;
    };
  }, [shouldAutoGenerate, meeting.id]); // Re-run if meeting changes

  return (
    <motion.div
      initial={{ opacity: 0, y: 20 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: 0.3, ease: 'easeOut' }}
      className="flex h-full flex-col bg-background"
    >
      {/* read.ai-style report header: title, date/duration meta, and an on-device
          overview-metrics strip computed from the local transcript. */}
      <ReportHeader
        title={meetingData.meetingTitle}
        createdAt={meeting.created_at}
        transcripts={meetingData.transcripts}
      />

      {/* On-device chapters strip (pause-based, deterministic). Clicking a chapter
          jumps the transcript to its first segment via the C1.6 mechanism. Renders
          nothing for short/gap-less meetings. */}
      <TopicsTimeline
        transcripts={meetingData.transcripts}
        onJumpToSegment={(segmentId, startSec) => {
          handleJumpToSource(segmentId);
          handleSeekToTime(startSec);
        }}
      />

      {/* Local audio playback (asset protocol); transcript timestamps + chapters seek into it. */}
      <div className="flex items-center gap-2 border-b border-border bg-card px-gutter py-1.5">
        <div className="min-w-0 flex-1">
          {/* Local audio playback (asset protocol); transcript timestamps + chapters
              seek into it. Renders nothing when there is no recording to play. */}
          <PlaybackBar ref={playbackRef} folderPath={meeting.folder_path} />
        </div>
        <Button
          ref={evidenceToggleRef}
          variant="outline"
          size="sm"
          onClick={() => setIsEvidenceOpen((open) => !open)}
          aria-expanded={isEvidenceOpen}
          aria-controls="evidence-drawer"
        >
          <FileText className="size-4" aria-hidden="true" />
          Transcript
        </Button>
      </div>

      <div className="flex min-h-0 flex-1 overflow-hidden">
        {/* THE REPORT — one scrolling document at a reading measure, not a pane in a
            split. The summary panel owns its own toolbar and body; this wrapper only
            gives it the column. */}
        <div className="flex min-w-0 flex-1 flex-col overflow-hidden">
          <SummaryPanel
            meeting={meeting}
            meetingTitle={meetingData.meetingTitle}
            onTitleChange={meetingData.handleTitleChange}
            isEditingTitle={meetingData.isEditingTitle}
            onStartEditTitle={() => meetingData.setIsEditingTitle(true)}
            onFinishEditTitle={() => meetingData.setIsEditingTitle(false)}
            summaryRef={meetingData.blockNoteSummaryRef}
            aiSummary={meetingData.aiSummary}
            summaryStatus={summaryGeneration.summaryStatus}
            transcripts={meetingData.transcripts}
            modelConfig={modelConfig}
            setModelConfig={setModelConfig}
            onSaveModelConfig={handleSaveModelConfig}
            onGenerateSummary={summaryGeneration.handleGenerateSummary}
            onStopGeneration={summaryGeneration.handleStopGeneration}
            customPrompt={customPrompt}
            summaryResponse={summaryResponse}
            onSaveSummary={meetingData.handleSaveSummary}
            onSummaryChange={meetingData.handleSummaryChange}
            onDirtyChange={meetingData.setIsSummaryDirty}
            summaryError={summaryGeneration.summaryError}
            onRegenerateSummary={summaryGeneration.handleRegenerateSummary}
            getSummaryStatusMessage={summaryGeneration.getSummaryStatusMessage}
            availableTemplates={templates.availableTemplates}
            selectedTemplate={templates.selectedTemplate}
            onTemplateSelect={templates.handleTemplateSelection}
            isModelConfigLoading={false}
            onOpenModelSettings={handleRegisterModalOpen}
            // Source-linked structured draft review (C1.6)
            structuredEnabled={structuredEnabled}
            draftResponse={meetingData.draftResponse}
            isDraftLoading={meetingData.isDraftLoading}
            draftError={meetingData.draftError}
            onJumpToSource={handleJumpToSource}
            onSummaryApproved={meetingData.refetchDraft}
          />
        </div>

        {/* THE EVIDENCE — always mounted, opened beside the claim. */}
        <EvidenceDrawer
          open={isEvidenceOpen}
          onClose={() => setIsEvidenceOpen(false)}
          returnFocusTo={evidenceToggleRef}
        >
          <TranscriptPanel
            transcripts={meetingData.transcripts}
            customPrompt={customPrompt}
            onPromptChange={setCustomPrompt}
            onCopyTranscript={copyOperations.handleCopyTranscript}
            onOpenMeetingFolder={meetingOperations.handleOpenMeetingFolder}
            isRecording={isRecording}
            disableAutoScroll={true}
            // Pagination props for efficient loading
            usePagination={true}
            segments={segments}
            hasMore={hasMore}
            isLoadingMore={isLoadingMore}
            totalCount={totalCount}
            loadedCount={loadedCount}
            onLoadMore={onLoadMore}
            // Retranscription props
            meetingId={meeting.id}
            meetingFolderPath={meeting.folder_path}
            onRefetchTranscripts={onRefetchTranscripts}
            // Jump-to-source (C1.6)
            scrollToSegmentId={scrollToSegmentId}
            scrollNonce={scrollNonce}
            onRequestSegment={handleRequestSegment}
            onSeekToTime={handleSeekToTime}
          />
        </EvidenceDrawer>
      </div>
    </motion.div>
  );
}
