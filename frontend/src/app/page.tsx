'use client';

import { useCallback, useEffect, useState } from 'react';
import { motion } from 'framer-motion';
import { RecordingControls } from '@/components/RecordingControls';
import { useSidebar } from '@/components/Sidebar/SidebarProvider';
import { usePermissionCheck } from '@/hooks/usePermissionCheck';
import { useRecordingState, RecordingStatus } from '@/contexts/RecordingStateContext';
import { useTranscripts } from '@/contexts/TranscriptContext';
import { useConfig } from '@/contexts/ConfigContext';
import { WrapUpPipeline } from '@/app/_components/WrapUpPipeline';
import Analytics from '@/lib/analytics';
import { SettingsModals } from './_components/SettingsModal';
import { TranscriptPanel } from './_components/TranscriptPanel';
import { HomeDashboard } from './_components/HomeDashboard';
import { useModalState } from '@/hooks/useModalState';
import { useRecordingStart } from '@/hooks/useRecordingStart';
import { useRecordingSession } from '@/contexts/RecordingSessionContext';
import { useTranscriptRecovery } from '@/hooks/useTranscriptRecovery';
import { TranscriptRecovery } from '@/components/TranscriptRecovery';
import { indexedDBService } from '@/services/indexedDBService';
import { toast } from 'sonner';
import { useRouter } from 'next/navigation';

export default function Home() {
  /**
   * The recording session now lives in the shell (ADR-F): `window.handleRecordingStop`
   * has to survive navigation, and the dock's Stop has to exist on every route. This
   * page reads that session instead of mounting the lifecycle hooks itself — the ONLY
   * thing still rooted here is starting, because the rail's Record button navigates to
   * `/` first and the consent gate and device pickers are written against that.
   */
  const {
    isRecording,
    setIsRecording: setIsRecordingState,
    isRecordingDisabled,
    handleRecordingStop,
    setIsStopping,
  } = useRecordingSession();
  const [showRecoveryDialog, setShowRecoveryDialog] = useState(false);
  /**
   * The wrap-up panel outlives the statuses that open it: SAVING flips to COMPLETED
   * two seconds before `useRecordingStop` navigates to the report, and to ERROR when
   * the save fails. Gating on the status alone would tear the panel down at exactly
   * the two moments it finally has something to say.
   */
  const [showWrapUp, setShowWrapUp] = useState(false);

  // Use contexts for state management
  const { meetingTitle, transcripts } = useTranscripts();
  const { transcriptModelConfig, selectedDevices } = useConfig();
  const recordingState = useRecordingState();

  // Extract status from global state
  const { status, statusMessage, isStopping, isProcessing } = recordingState;

  // Hooks
  const { hasMicrophone } = usePermissionCheck();
  const { refetchMeetings, currentMeeting } = useSidebar();
  const { modals, messages, showModal, hideModal } = useModalState(transcriptModelConfig);
  const { handleRecordingStart } = useRecordingStart(isRecording, setIsRecordingState, showModal);

  // Recovery hook
  const {
    recoverableMeetings,
    isLoading: isLoadingRecovery,
    isRecovering,
    checkForRecoverableTranscripts,
    recoverMeeting,
    loadMeetingTranscripts,
    deleteRecoverableMeeting
  } = useTranscriptRecovery();

  const router = useRouter();

  useEffect(() => {
    // Track page view
    Analytics.trackPageView('home');
  }, []);

  // Startup recovery check
  useEffect(() => {
    const performStartupChecks = async () => {
      try {
        // Skip recovery check if currently recording or processing stop
        // This prevents the recovery dialog from showing when:
        if (recordingState.isRecording ||
          status === RecordingStatus.STOPPING ||
          status === RecordingStatus.PROCESSING_TRANSCRIPTS ||
          status === RecordingStatus.SAVING) {
          console.log('Skipping recovery check - recording in progress or processing');
          return;
        }

        // 1. Immediately purge legacy plaintext copies that older releases
        // retained after the same meeting had already been saved to SQLite.
        try {
          await indexedDBService.purgeSavedMeetings();
        } catch (error) {
          console.warn('⚠️ Failed to purge saved recovery copies:', error);
        }

        // 2. Apply the seven-day retention limit to remaining crash-recovery data.
        try {
          await indexedDBService.deleteOldMeetings(7);
        } catch (error) {
          console.warn('⚠️ Failed to clean up old recovery data:', error);
        }

        // 3. Always check for recoverable meetings on startup
        // Don't skip based on sessionStorage - we need to check every time
        await checkForRecoverableTranscripts();
      } catch (error) {
        console.error('Failed to perform startup checks:', error);
      }
    };

    performStartupChecks();
  }, [checkForRecoverableTranscripts, recordingState.isRecording, status]);

  // Watch for recoverable meetings changes and show dialog once per session
  useEffect(() => {
    // Only show dialog if we have meetings and haven't shown it yet this session
    if (recoverableMeetings.length > 0) {
      const shownThisSession = sessionStorage.getItem('recovery_dialog_shown');
      if (!shownThisSession) {
        setShowRecoveryDialog(true);
        sessionStorage.setItem('recovery_dialog_shown', 'true');
      }
    }
  }, [recoverableMeetings]);

  // Handle recovery with toast notifications and navigation
  const handleRecovery = async (meetingId: string) => {
    try {
      const result = await recoverMeeting(meetingId);

      if (result.success) {
        toast.success('Meeting recovered successfully!', {
          description: result.audioRecoveryStatus?.status === 'success'
            ? 'Transcripts and audio recovered'
            : 'Transcripts recovered (no audio available)',
          action: result.meetingId ? {
            label: 'View Meeting',
            onClick: () => {
              router.push(`/meeting-details?id=${result.meetingId}`);
            }
          } : undefined,
          duration: 10000,
        });

        // Refresh sidebar to show the newly recovered meeting
        await refetchMeetings();

        // If no more recoverable meetings, clear session flag so dialog can show again
        if (recoverableMeetings.length === 0) {
          sessionStorage.removeItem('recovery_dialog_shown');
        }

        // Auto-navigate after a short delay
        if (result.meetingId) {
          setTimeout(() => {
            router.push(`/meeting-details?id=${result.meetingId}`);
          }, 2000);
        }
      }
    } catch (error) {
      toast.error('Failed to recover meeting', {
        description: error instanceof Error ? error.message : 'Unknown error occurred',
      });
      throw error;
    }
  };

  // Handle dialog close - clear session flag if no meetings left
  const handleDialogClose = () => {
    setShowRecoveryDialog(false);
    // If user closes dialog and there are no more meetings, clear the flag
    // This allows the dialog to show again next session if new meetings appear
    if (recoverableMeetings.length === 0) {
      sessionStorage.removeItem('recovery_dialog_shown');
    }
  };

  // Computed values using global status
  const isProcessingStop = status === RecordingStatus.PROCESSING_TRANSCRIPTS || isProcessing;

  useEffect(() => {
    if (status === RecordingStatus.PROCESSING_TRANSCRIPTS || status === RecordingStatus.SAVING) {
      setShowWrapUp(true);
    } else if (status === RecordingStatus.IDLE || recordingState.isRecording) {
      setShowWrapUp(false);
    }
  }, [status, recordingState.isRecording]);

  const dismissWrapUp = useCallback(() => setShowWrapUp(false), []);
  const openLastReport = useCallback(() => {
    if (currentMeeting?.id) {
      router.push(`/meeting-details?id=${currentMeeting.id}`);
    }
  }, [currentMeeting?.id, router]);

  return (
    <motion.div
      initial={{ opacity: 0, y: 20 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: 0.3, ease: 'easeOut' }}
      className="flex h-full flex-col bg-muted"
    >
      {/* All Modals supported*/}
      <SettingsModals
        modals={modals}
        messages={messages}
        onClose={hideModal}
      />

      {/* Recovery Dialog */}
      <TranscriptRecovery
        isOpen={showRecoveryDialog}
        onClose={handleDialogClose}
        recoverableMeetings={recoverableMeetings}
        onRecover={handleRecovery}
        onDelete={deleteRecoverableMeeting}
        onLoadPreview={loadMeetingTranscripts}
      />
      {/* `relative` is the containing block for the record pill and the status
          overlays, which are absolute inside the content pane rather than fixed to the
          window — that is what removed their copies of the sidebar width. */}
      <div className="relative flex flex-1 overflow-hidden">
        {/* Phase C: while idle (nothing recorded yet this session), the home route
            is a dashboard of recent meeting reports; the live transcript panel
            takes over the moment a recording starts. */}
        {(status === RecordingStatus.IDLE || status === RecordingStatus.COMPLETED) &&
        !recordingState.isRecording &&
        transcripts.length === 0 ? (
          <HomeDashboard />
        ) : (
          <TranscriptPanel
            isProcessingStop={isProcessingStop}
            isStopping={isStopping}
            showModal={showModal}
          />
        )}

        {/* Recording controls - only show when permissions are granted or already recording and not showing status messages */}
        {(hasMicrophone || isRecording) &&
          status !== RecordingStatus.PROCESSING_TRANSCRIPTS &&
          status !== RecordingStatus.SAVING && (
            <div className="absolute inset-x-0 bottom-12 z-10 flex justify-center px-gutter">
              <div className="flex justify-center">
                <div className="flex items-center rounded-full border border-border bg-card shadow-elev-2">
                  <div className="flex items-center">
                    <RecordingControls
                      isRecording={recordingState.isRecording}
                      onRecordingStop={(callApi = true) => handleRecordingStop(callApi)}
                      onRecordingStart={handleRecordingStart}
                      onTranscriptReceived={() => { }} // Not actually used by RecordingControls
                      onStopInitiated={() => setIsStopping(true)}
                      onTranscriptionError={(message) => {
                        showModal('errorAlert', message);
                      }}
                      isRecordingDisabled={isRecordingDisabled}
                      isParentProcessing={isProcessingStop}
                      selectedDevices={selectedDevices}
                      meetingName={meetingTitle}
                    />
                  </div>
                </div>
              </div>
            </div>
          )}

        {/* What happens after Stop: the status message the app has always written and
            never shown, with the steps around it (DESIGN_SYSTEM.md §6.2). */}
        {showWrapUp && !recordingState.isRecording && (
          <div className="absolute inset-0 z-20 grid place-items-center overflow-y-auto bg-background/80 backdrop-blur-sm">
            <WrapUpPipeline
              status={status}
              statusMessage={statusMessage}
              segmentCount={transcripts.length}
              meetingId={currentMeeting?.id}
              onOpenReport={openLastReport}
              onRecordAnother={dismissWrapUp}
            />
          </div>
        )}
      </div>
    </motion.div>
  );
}
