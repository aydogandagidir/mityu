import React, { useCallback, useEffect } from 'react';
import { useOnboarding } from '@/contexts/OnboardingContext';
import { useSidebar } from '@/components/Sidebar/SidebarProvider';
import { useLicensing } from '@/contexts/LicensingContext';
import {
  WelcomeStep,
  PermissionsStep,
  DownloadProgressStep,
  SetupOverviewStep,
} from './steps';

interface OnboardingFlowProps {
  onComplete: () => void;
}

export function OnboardingFlow({ onComplete }: OnboardingFlowProps) {
  const { currentStep } = useOnboarding();
  const [isMac, setIsMac] = React.useState(false);
  const { refetchMeetings } = useSidebar();
  const { refresh: refreshLicensing } = useLicensing();

  /**
   * Finishing setup used to call `window.location.reload()` from three places, and the
   * `onComplete` this component is handed was never called at all. The reload was doing
   * real work — it is what made the providers re-read a database that did not exist when
   * they first mounted — so removing it means doing that work explicitly. This runs
   * INSIDE the provider tree, which is why it lives here rather than in the shell that
   * owns the callback.
   */
  const finish = useCallback(async () => {
    await Promise.allSettled([refetchMeetings(), refreshLicensing()]);
    onComplete();
  }, [onComplete, refetchMeetings, refreshLicensing]);

  useEffect(() => {
    // Check if running on macOS
    const checkPlatform = async () => {
      try {
        // Dynamic import to avoid SSR issues if any
        const { platform } = await import('@tauri-apps/plugin-os');
        setIsMac(platform() === 'macos');
      } catch (e) {
        console.error('Failed to detect platform:', e);
        // Fallback
        setIsMac(navigator.userAgent.includes('Mac'));
      }
    };
    checkPlatform();
  }, []);

  // 4-Step Onboarding Flow (System-Recommended Models):
  // Step 1: Welcome - Introduce Mityu features
  // Step 2: Setup Overview - Database initialization + show recommended downloads
  // Step 3: Download Progress - Download Parakeet + Summary Model (auto-selected based on platform/RAM)
  // Step 4: Permissions - Request mic + system audio (macOS only)

  // A persisted `current_step === 4` on a machine that is not macOS would render
  // nothing at all — the user would open the app to a blank setup screen with no way
  // out. It falls back to the last step that exists on this platform.
  const step = currentStep === 4 && !isMac ? 3 : currentStep;

  return (
    <div className="onboarding-flow">
      {step === 1 && <WelcomeStep />}
      {step === 2 && <SetupOverviewStep />}
      {step === 3 && <DownloadProgressStep onFinish={finish} />}
      {step === 4 && isMac && <PermissionsStep onFinish={finish} />}
    </div>
  );
}
