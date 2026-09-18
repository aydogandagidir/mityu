'use client'

/**
 * The main application shell: every provider, listener and piece of chrome the
 * product UI needs (sidebar, onboarding, drag-to-import, update checks, trial
 * banner, tour).
 *
 * Extracted from `app/layout.tsx` for BACKLOG I1. The copilot panel is a second
 * OS window loading `/copilot` from the same static export, and it must not
 * mount any of this — a 380 px panel has no room for a sidebar, and mounting
 * the shell there would give a second window its own onboarding check, update
 * checker, file-drop listeners and trial banner. Because a React component
 * cannot skip its hooks conditionally, the only way for the panel to avoid them
 * is for the shell that owns them not to be mounted at all. Hence this file:
 * the root layout now picks a shell instead of being one.
 *
 * The move was verbatim — same providers, same order, same effects — so the
 * behaviour of the main window is unchanged.
 */

import Sidebar from '@/components/Sidebar'
import { SidebarProvider } from '@/components/Sidebar/SidebarProvider'
import MainContent from '@/components/MainContent'
import AnalyticsProvider from '@/components/AnalyticsProvider'
import { toast } from 'sonner'
import { useState, useEffect, useCallback } from 'react'
import { listen, UnlistenFn } from '@tauri-apps/api/event'
import { invoke } from '@tauri-apps/api/core'
import { TooltipProvider } from '@/components/ui/tooltip'
import { RecordingStateProvider } from '@/contexts/RecordingStateContext'
import { RecordingConsentProvider } from '@/contexts/RecordingConsentContext'
import { OllamaDownloadProvider } from '@/contexts/OllamaDownloadContext'
import { TranscriptProvider } from '@/contexts/TranscriptContext'
import { ConfigProvider, useConfig } from '@/contexts/ConfigContext'
import { OnboardingProvider } from '@/contexts/OnboardingContext'
import { OnboardingFlow } from '@/components/onboarding'
import { loadBetaFeatures } from '@/types/betaFeatures'
import { DownloadProgressToastProvider } from '@/components/shared/DownloadProgressToast'
import { UpdateCheckProvider } from '@/components/UpdateCheckProvider'
import { RecordingPostProcessingProvider } from '@/contexts/RecordingPostProcessingProvider'
import { ImportAudioDialog, ImportDropOverlay } from '@/components/ImportAudio'
import { ImportDialogProvider } from '@/contexts/ImportDialogContext'
import { SystemNotices } from '@/components/shell/SystemNotices'
import { LicensingProvider } from '@/contexts/LicensingContext'
import { isAudioExtension, getAudioFormatsDisplayList } from '@/constants/audioFormats'
import { isTauri } from '@/lib/isTauri'
import { TourProvider } from '@/components/tour'
import { CommandPalette } from '@/components/shell/CommandPalette'
import { RecordingSessionProvider } from '@/contexts/RecordingSessionContext'
import { ShellSessionDock } from '@/components/shell/ShellSessionDock'
import { WhatsNew } from '@/components/WhatsNew'


// Module-level component — stable reference across RootLayout re-renders.
// Defined here (not inside RootLayout) so React never sees a new function type
// on re-render, which would cause unmount/remount and break initialization logic.
function ConditionalImportDialog({
  showImportDialog,
  handleImportDialogClose,
  importFilePath,
}: {
  showImportDialog: boolean;
  handleImportDialogClose: (open: boolean) => void;
  importFilePath: string | null;
}) {
  const { betaFeatures } = useConfig();

  // Only mount ImportAudioDialog (and its hooks/listeners) when feature is enabled
  if (!betaFeatures.importAndRetranscribe) {
    return null;
  }

  return (
    <ImportAudioDialog
      open={showImportDialog}
      onOpenChange={handleImportDialogClose}
      preselectedFile={importFilePath}
    />
  );
}

export function AppShell({
  children,
}: {
  children: React.ReactNode
}) {
  const [showOnboarding, setShowOnboarding] = useState(false)
  const [onboardingCompleted, setOnboardingCompleted] = useState(false)

  // Import audio state
  const [showDropOverlay, setShowDropOverlay] = useState(false)
  const [showImportDialog, setShowImportDialog] = useState(false)
  const [importFilePath, setImportFilePath] = useState<string | null>(null)

  useEffect(() => {
    // Outside the Tauri shell (browser dev-preview / design route) there is no
    // backend to ask and `invoke` would throw. Render the main shell directly so
    // the UI is inspectable without the desktop app; never show onboarding here.
    if (!isTauri()) {
      setOnboardingCompleted(true)
      setShowOnboarding(false)
      return
    }

    // Check onboarding status first
    invoke<{ completed: boolean } | null>('get_onboarding_status')
      .then((status) => {
        const isComplete = status?.completed ?? false
        setOnboardingCompleted(isComplete)

        if (!isComplete) {
          console.log('[Layout] Onboarding not completed, showing onboarding flow')
          setShowOnboarding(true)
        } else {
          console.log('[Layout] Onboarding completed, showing main app')
        }
      })
      .catch((error) => {
        console.error('[Layout] Failed to check onboarding status:', error)
        // Default to showing onboarding if we can't check
        setShowOnboarding(true)
        setOnboardingCompleted(false)
      })
  }, [])

  // The production build used to `preventDefault()` every context menu, which killed
  // right-click copy and paste in the BlockNote editor and in the transcript — the two
  // places in this app where a person most needs them. Whatever it was guarding against
  // (a developer-tools entry the WebView does not offer in a release build anyway), the
  // cost was the standard editing menu of the platform.
  useEffect(() => {
    // Listen for tray recording toggle request
    const unlisten = listen('request-recording-toggle', () => {
      console.log('[Layout] Received request-recording-toggle from tray');

      if (showOnboarding) {
        toast.error("Please complete setup first", {
          description: "You need to finish onboarding before you can start recording."
        });
      } else if (window.location.pathname === '/') {
        // On the home page the listener is mounted, so dispatch directly.
        console.log('[Layout] Forwarding to start-recording-from-sidebar');
        window.dispatchEvent(new CustomEvent('start-recording-from-sidebar'));
      } else {
        // Anywhere else it is NOT: `useRecordingStart` — the only listener for
        // that event — is mounted by the home page alone, so dispatching here
        // did nothing at all. Pressing record from the tray while viewing a
        // meeting or settings was silently ignored. Navigate first and let the
        // home page pick it up, which is what the sidebar button already does.
        console.log('[Layout] Not on home; routing there to start the recording');
        sessionStorage.setItem('autoStartRecording', 'true');
        window.location.assign('/');
      }
    });

    return () => {
      unlisten.then(fn => fn());
    };
  }, [showOnboarding]);

  // The backend's own recording failures.
  //
  // `recording-error` is emitted by the Rust recording pipeline when a start or
  // a stop fails (audio/recording_commands.rs), carrying a sentence written for
  // a user. Nothing listened to it, so a whole class of failures — the device
  // disappeared, the stream could not open, the engine would not load — reached
  // the user as a button that did nothing. Mounted app-wide because the failure
  // can arrive long after the click, and on any route.
  useEffect(() => {
    const unlisten = listen<string>('recording-error', (event) => {
      const message = typeof event.payload === 'string' ? event.payload : String(event.payload);
      console.error('[Layout] recording-error from the backend:', message);
      toast.error('Recording problem', {
        description: message || 'The recording pipeline reported a failure with no detail.',
        duration: 10000,
      });
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  // Handle file drop for audio import
  const handleFileDrop = useCallback((paths: string[]) => {
    // Check if beta features are enabled (read from localStorage directly since we're outside ConfigProvider)
    const betaFeatures = loadBetaFeatures();

    if (!betaFeatures.importAndRetranscribe) {
      toast.error('Beta feature disabled', {
        description: 'Enable "Import Audio & Retranscribe" in Settings > Beta to use this feature.'
      });
      return;
    }

    // Find the first audio file
    const audioFile = paths.find(p => {
      const ext = p.split('.').pop()?.toLowerCase();
      return !!ext && isAudioExtension(ext);
    });

    if (audioFile) {
      console.log('[Layout] Audio file dropped:', audioFile);
      setImportFilePath(audioFile);
      setShowImportDialog(true);
    } else if (paths.length > 0) {
      toast.error('Please drop an audio file', {
        description: `Supported formats: ${getAudioFormatsDisplayList()}`
      });
    }
  }, []);

  // Listen for drag-drop events
  useEffect(() => {
    if (showOnboarding) return; // Don't handle drops during onboarding

    const unlisteners: UnlistenFn[] = [];
    const cleanedUpRef = { current: false };

    const setupListeners = async () => {
      // Drag enter/over - show overlay only if beta feature is enabled
      const unlistenDragEnter = await listen('tauri://drag-enter', () => {
        if (loadBetaFeatures().importAndRetranscribe) {
          setShowDropOverlay(true);
        }
      });
      if (cleanedUpRef.current) {
        unlistenDragEnter();
        return;
      }
      unlisteners.push(unlistenDragEnter);

      // Drag leave - hide overlay
      const unlistenDragLeave = await listen('tauri://drag-leave', () => {
        setShowDropOverlay(false);
      });
      if (cleanedUpRef.current) {
        unlistenDragLeave();
        unlisteners.forEach(u => u());
        return;
      }
      unlisteners.push(unlistenDragLeave);

      // Drop - process files
      const unlistenDrop = await listen<{ paths: string[] }>('tauri://drag-drop', (event) => {
        setShowDropOverlay(false);
        handleFileDrop(event.payload.paths);
      });
      if (cleanedUpRef.current) {
        unlistenDrop();
        unlisteners.forEach(u => u());
        return;
      }
      unlisteners.push(unlistenDrop);
    };

    setupListeners();

    return () => {
      cleanedUpRef.current = true;
      unlisteners.forEach((unlisten) => unlisten());
    };
  }, [showOnboarding, handleFileDrop]);

  // Handle import dialog close
  const handleImportDialogClose = useCallback((open: boolean) => {
    setShowImportDialog(open);
    if (!open) {
      setImportFilePath(null);
    }
  }, []);

  // Handler for ImportDialogProvider - opens import dialog from any child component
  const handleOpenImportDialog = useCallback((filePath?: string | null) => {
    setImportFilePath(filePath ?? null);
    setShowImportDialog(true);
  }, []);

  const handleOnboardingComplete = () => {
    // No reload. `OnboardingFlow` refetches what the reload was really for — it runs
    // inside the provider tree, which this callback does not — and the shell just
    // switches surfaces. The white flash that used to end setup is gone with it.
    setShowOnboarding(false)
    setOnboardingCompleted(true)
  }

  return (
    <AnalyticsProvider>
          <RecordingStateProvider>
            <RecordingConsentProvider>
            <TranscriptProvider>
              <ConfigProvider>
                <OllamaDownloadProvider>
                  <OnboardingProvider>
                    <UpdateCheckProvider>
                      <SidebarProvider>
                        <TooltipProvider>
                          <RecordingPostProcessingProvider>
                            <ImportDialogProvider onOpen={handleOpenImportDialog}>
                            {/* ADR-0023 licensing: status provider + the shared activate/paywall
                                dialog. Innermost position that still wraps every consumer:
                                TrialBanner (below), the Settings License section and
                                useRecordingStart (both under {children}), and
                                ImportAudioDialog's paywall interception. */}
                            <LicensingProvider>
                              {/* Download progress toast provider - listens for background downloads */}
                              <DownloadProgressToastProvider />

                              {/* ⌘K, the shortcuts sheet and the global keys. Mounted
                                  inside the providers it reads, and NOT during
                                  onboarding — a palette that can navigate away from
                                  setup is a way to skip it by accident. */}
                              {!showOnboarding && <CommandPalette />}

                              {/* Show onboarding or main app */}
                              {showOnboarding ? (
                                <OnboardingFlow onComplete={handleOnboardingComplete} />
                              ) : (
                                // First-run product tour lives ONLY in the main-app shell
                                // (never during setup onboarding). It renders the welcome
                                // overlay + coach-marks and, post-onboarding, routes to the
                                // pre-seeded sample meeting. isTauri()-gated internally.
                                <RecordingSessionProvider>
                                <TourProvider>
                                  <div className="flex h-screen flex-col overflow-hidden">
                                  <div className="flex min-h-0 flex-1 overflow-hidden">
                                    <Sidebar />
                                    <MainContent>
                                      {/* One slot for app-wide notices, so the trial chrome
                                          (ADR-0023) and the unencrypted-at-rest warning
                                          (ADR-0014) cannot each bring their own top gutter.
                                          Both keep their own visibility rules and render
                                          nothing in the normal case. */}
                                      <SystemNotices />
                                      {children}
                                    </MainContent>
                                  </div>
                                  {/* The shell's own recording indicator and the only Stop
                                      that exists on every route (ADR-F). It publishes
                                      --bottom-chrome so toasts and page bodies clear it. */}
                                  <ShellSessionDock />
                                  </div>
                                </TourProvider>
                                </RecordingSessionProvider>
                              )}
                              {/* What changed in this version (G2). Renders
                                  nothing on a fresh install and nothing when
                                  the version has not moved; mounted outside the
                                  onboarding branch so it can never interrupt
                                  first-run setup. */}
                              {!showOnboarding && <WhatsNew />}
                              {/* Import audio overlay and dialog */}
                              <ImportDropOverlay visible={showDropOverlay} />
                              <ConditionalImportDialog
                                showImportDialog={showImportDialog}
                                handleImportDialogClose={handleImportDialogClose}
                                importFilePath={importFilePath}
                              />
                            </LicensingProvider>
                            </ImportDialogProvider>
                          </RecordingPostProcessingProvider>
                        </TooltipProvider>
                      </SidebarProvider>
                    </UpdateCheckProvider>
                  </OnboardingProvider>

                </OllamaDownloadProvider>
              </ConfigProvider>
            </TranscriptProvider>
            </RecordingConsentProvider>
          </RecordingStateProvider>
        </AnalyticsProvider>
  )
}
