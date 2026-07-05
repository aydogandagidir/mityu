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
import { FileText, Sparkles, PanelRightOpen } from 'lucide-react';

// Custom hooks
import { useMeetingData } from '@/hooks/meeting-details/useMeetingData';
import { useSummaryGeneration } from '@/hooks/meeting-details/useSummaryGeneration';
import { useTemplates } from '@/hooks/meeting-details/useTemplates';
import { useCopyOperations } from '@/hooks/meeting-details/useCopyOperations';
import { useMeetingOperations } from '@/hooks/meeting-details/useMeetingOperations';
import { useConfig } from '@/contexts/ConfigContext';

export default function PageContent({
  meeting,
  summaryData,
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

  // Meeting-details split layout (frontend-only rebalance):
  //  - Desktop (md+): transcript (primary) grows to fill; summary is capped and
  //    can be COLLAPSED so the transcript reclaims the full width.
  //  - Narrow (< md): the two panels are switched via an in-page tab bar so the
  //    transcript (primary content) is always reachable — it used to be hidden.
  // Both panels stay mounted at all times (CSS show/hide, never unmount) so the
  // BlockNote editor / draft-review state and the transcript scroll position
  // survive collapsing and tab switches.
  const [isSummaryCollapsed, setIsSummaryCollapsed] = useState(false);
  const [mobileTab, setMobileTab] = useState<'transcript' | 'summary'>('transcript');

  // Ref to store the modal open function from SummaryGeneratorButtonGroup
  const openModelSettingsRef = useRef<(() => void) | null>(null);

  // Sidebar context
  const { serverAddress } = useSidebar();

  // Get model config + beta features from ConfigContext
  const { modelConfig, setModelConfig, betaFeatures } = useConfig();

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

  // Route to the structured HITL review surface when the beta flag is on OR the
  // meeting already has a source-linked draft (so existing structured drafts keep
  // rendering even if the flag is later turned off). Otherwise the legacy summary
  // views are used unchanged.
  const structuredEnabled = betaFeatures.structuredSummaries || meetingData.hasSummaryDraft;

  // BACKLOG C1.6 — jump from a draft block/action item to its transcript segment.
  const handleJumpToSource = (sourceChunkId: string) => {
    setScrollToSegmentId(sourceChunkId);
    setScrollNonce((n) => n + 1);
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
    // C1.6: request structured drafts when the beta flag is on (generation keys
    // off the flag only), and refresh the draft when one completes so the review
    // surface populates.
    structuredSummaries: betaFeatures.structuredSummaries,
    onStructuredGenerated: meetingData.refetchDraft,
  });

  const copyOperations = useCopyOperations({
    meeting,
    transcripts: meetingData.transcripts,
    meetingTitle: meetingData.meetingTitle,
    aiSummary: meetingData.aiSummary,
    blockNoteSummaryRef: meetingData.blockNoteSummaryRef,
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
      className="flex flex-col h-screen bg-gray-50"
    >
      {/* Narrow-screen (< md) tab bar: switches which panel is visible so the
          transcript (primary content) is reachable on mobile/tablet. Hidden on
          md+ where both panels sit side by side. */}
      <div className="md:hidden flex items-center gap-2 px-3 py-2 border-b border-gray-200 bg-white">
        <button
          type="button"
          onClick={() => setMobileTab('transcript')}
          aria-pressed={mobileTab === 'transcript'}
          className={`flex-1 inline-flex items-center justify-center gap-2 rounded-md px-3 py-1.5 text-sm font-medium transition-colors ${
            mobileTab === 'transcript'
              ? 'bg-blue-50 text-blue-700 border border-blue-200'
              : 'text-gray-600 hover:bg-gray-50 border border-transparent'
          }`}
        >
          <FileText size={16} />
          Transcript
        </button>
        <button
          type="button"
          onClick={() => setMobileTab('summary')}
          aria-pressed={mobileTab === 'summary'}
          className={`flex-1 inline-flex items-center justify-center gap-2 rounded-md px-3 py-1.5 text-sm font-medium transition-colors ${
            mobileTab === 'summary'
              ? 'bg-blue-50 text-blue-700 border border-blue-200'
              : 'text-gray-600 hover:bg-gray-50 border border-transparent'
          }`}
        >
          <Sparkles size={16} />
          Summary
        </button>
      </div>

      <div className="flex flex-1 overflow-hidden">
        {/* Transcript wrapper — PRIMARY content.
            - Mobile: full width, visible only when its tab is active.
            - md+: always visible and grows to fill (flex-1), so it is never the
              cramped panel and it reclaims space when the summary collapses.
            The right border only shows on md+ when the summary is visible. */}
        <div
          className={`${
            mobileTab === 'transcript' ? 'flex' : 'hidden'
          } w-full min-w-0 md:flex md:flex-1 md:min-w-0 ${
            isSummaryCollapsed ? '' : 'md:border-r md:border-gray-200'
          }`}
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
          />
        </div>

        {/* Summary wrapper — the CAPPED / SHRINKING panel (no longer dominant).
            - Mobile: full width, visible only when its tab is active.
            - md+: capped to ~half the width (max 640px) and does not grow, so
              the transcript keeps comfortable room. Hidden when collapsed. */}
        <div
          className={`${
            mobileTab === 'summary' ? 'flex' : 'hidden'
          } w-full min-w-0 md:w-1/2 md:max-w-[640px] md:shrink-0 ${
            isSummaryCollapsed ? 'md:hidden' : 'md:flex'
          }`}
        >
          <SummaryPanel
          meeting={meeting}
          meetingTitle={meetingData.meetingTitle}
          onTitleChange={meetingData.handleTitleChange}
          isEditingTitle={meetingData.isEditingTitle}
          onStartEditTitle={() => meetingData.setIsEditingTitle(true)}
          onFinishEditTitle={() => meetingData.setIsEditingTitle(false)}
          isTitleDirty={meetingData.isTitleDirty}
          summaryRef={meetingData.blockNoteSummaryRef}
          isSaving={meetingData.isSaving}
          onSaveAll={meetingData.saveAllChanges}
          onCopySummary={copyOperations.handleCopySummary}
          onOpenFolder={meetingOperations.handleOpenMeetingFolder}
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
          // Desktop collapse control (chevron lives in the summary header).
          showCollapseButton
          onCollapse={() => setIsSummaryCollapsed(true)}
          />
        </div>

        {/* Collapsed-state expand rail (md+ only): a slim edge affordance to bring
            the summary panel back after it has been collapsed. */}
        {isSummaryCollapsed && (
          <div className="hidden md:flex flex-col items-center border-l border-gray-200 bg-white shrink-0">
            <button
              type="button"
              onClick={() => setIsSummaryCollapsed(false)}
              title="Show summary panel"
              aria-label="Show summary panel"
              className="p-2 m-1 rounded-md text-gray-500 hover:text-gray-800 hover:bg-gray-100"
            >
              <PanelRightOpen size={18} />
            </button>
          </div>
        )}
      </div>
    </motion.div>
  );
}
