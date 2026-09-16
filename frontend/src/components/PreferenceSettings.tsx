"use client"

import { useEffect, useState, useRef } from "react"
import { Switch } from "./ui/switch"
import { FolderOpen, Compass, Sparkles } from "lucide-react"
import { openDatabaseFolder, openModelsFolder, openRecordingsFolder } from "@/services/systemService"
import Analytics from "@/lib/analytics"
import { useTour } from "@/components/tour"
import AnalyticsConsentSwitch from "./AnalyticsConsentSwitch"
import RecordingConsentSettings from "./RecordingConsentSettings"
import RedactionSettings from "./RedactionSettings"
import LearningSettings from "./LearningSettings"
import { ThemeToggle } from "./ThemeToggle"
import { WhatsNew } from "./WhatsNew"
import { SettingCard } from "./ui/setting-card"
import { APP_VERSION } from "@/lib/appVersion"
import { useConfig, NotificationSettings } from "@/contexts/ConfigContext"

export function PreferenceSettings() {
  const {
    notificationSettings,
    storageLocations,
    isLoadingPreferences,
    loadPreferences,
    updateNotificationSettings
  } = useConfig();

  const { replayTour } = useTour();
  const [whatsNewOpen, setWhatsNewOpen] = useState(false);

  const [notificationsEnabled, setNotificationsEnabled] = useState<boolean | null>(null);
  const [isInitialLoad, setIsInitialLoad] = useState(true);
  const [previousNotificationsEnabled, setPreviousNotificationsEnabled] = useState<boolean | null>(null);
  const hasTrackedViewRef = useRef(false);

  // Lazy load preferences on mount (only loads if not already cached)
  useEffect(() => {
    loadPreferences();
    // Reset tracking ref on mount (every tab visit)
    hasTrackedViewRef.current = false;
  }, [loadPreferences]);

  // Track preferences viewed analytics on every tab visit (once per mount)
  useEffect(() => {
    if (hasTrackedViewRef.current) return;

    const trackPreferencesViewed = async () => {
      // Wait for notification settings to be available (either from cache or after loading)
      if (notificationSettings) {
        await Analytics.track('preferences_viewed', {
          notifications_enabled: notificationSettings.notification_preferences.show_recording_started ? 'true' : 'false'
        });
        hasTrackedViewRef.current = true;
      } else if (!isLoadingPreferences) {
        // If not loading and no settings available, track with default value
        await Analytics.track('preferences_viewed', {
          notifications_enabled: 'false'
        });
        hasTrackedViewRef.current = true;
      }
    };

    trackPreferencesViewed();
  }, [notificationSettings, isLoadingPreferences]);

  // Update notificationsEnabled when notificationSettings are loaded from global state
  useEffect(() => {
    if (notificationSettings) {
      // Notification enabled means both started and stopped notifications are enabled
      const enabled =
        notificationSettings.notification_preferences.show_recording_started &&
        notificationSettings.notification_preferences.show_recording_stopped;
      setNotificationsEnabled(enabled);
      if (isInitialLoad) {
        setPreviousNotificationsEnabled(enabled);
        setIsInitialLoad(false);
      }
    } else if (!isLoadingPreferences) {
      // If not loading and no settings, use default
      setNotificationsEnabled(true);
      if (isInitialLoad) {
        setPreviousNotificationsEnabled(true);
        setIsInitialLoad(false);
      }
    }
  }, [notificationSettings, isLoadingPreferences, isInitialLoad])

  useEffect(() => {
    // Skip update on initial load or if value hasn't actually changed
    if (isInitialLoad || notificationsEnabled === null || notificationsEnabled === previousNotificationsEnabled) return;
    if (!notificationSettings) return;

    const handleUpdateNotificationSettings = async () => {
      console.log("Updating notification settings to:", notificationsEnabled);

      try {
        // Update the notification preferences
        const updatedSettings: NotificationSettings = {
          ...notificationSettings,
          notification_preferences: {
            ...notificationSettings.notification_preferences,
            show_recording_started: notificationsEnabled,
            show_recording_stopped: notificationsEnabled,
          }
        };

        console.log("Calling updateNotificationSettings with:", updatedSettings);
        await updateNotificationSettings(updatedSettings);
        setPreviousNotificationsEnabled(notificationsEnabled);
        console.log("Successfully updated notification settings to:", notificationsEnabled);

        // Track notification preference change - only fires when user manually toggles
        await Analytics.track('notification_settings_changed', {
          notifications_enabled: notificationsEnabled.toString()
        });
      } catch (error) {
        console.error('Failed to update notification settings:', error);
      }
    };

    handleUpdateNotificationSettings();
  }, [notificationsEnabled, notificationSettings, isInitialLoad, previousNotificationsEnabled, updateNotificationSettings])

  const handleOpenFolder = async (folderType: 'database' | 'models' | 'recordings') => {
    try {
      switch (folderType) {
        case 'database':
          await openDatabaseFolder();
          break;
        case 'models':
          await openModelsFolder();
          break;
        case 'recordings':
          await openRecordingsFolder();
          break;
      }

      // Track storage folder access
      await Analytics.track('storage_folder_opened', {
        folder_type: folderType
      });
    } catch (error) {
      console.error(`Failed to open ${folderType} folder:`, error);
    }
  };

  // Show loading only if we're actually loading and don't have cached data
  if (isLoadingPreferences && !notificationSettings && !storageLocations) {
    return <div className="max-w-2xl mx-auto p-6">Loading Preferences...</div>
  }

  // Show loading if notificationsEnabled hasn't been determined yet
  if (notificationsEnabled === null && !isLoadingPreferences) {
    return <div className="max-w-2xl mx-auto p-6">Loading Preferences...</div>
  }

  // Ensure we have a boolean value for the Switch component
  const notificationsEnabledValue = notificationsEnabled ?? false;

  return (
    <div className="space-y-4">
      {/* Every card here is the same primitive now (G4). Before, each was a
          hand-rolled div at p-6 with a text-lg heading, so a theme toggle and
          the storage-and-deletion caveats carried identical weight and the
          screen read as one wall of text. */}
      <SettingCard
        title="Appearance"
        description="Follow your system theme, or force light or dark."
        action={<ThemeToggle />}
      />

      <SettingCard
        title="Notifications"
        description="Tell me when a meeting starts and ends."
        action={
          <Switch checked={notificationsEnabledValue} onCheckedChange={setNotificationsEnabled} />
        }
      />

      <SettingCard
        title="Product tour"
        description="Replay the guided walkthrough on the sample meeting."
        action={
          <button
            onClick={() => {
              void Analytics.trackButtonClick('replay_product_tour', 'settings');
              replayTour();
            }}
            className="inline-flex items-center gap-2 rounded-md border border-input bg-background px-4 py-2 text-meta font-medium transition-colors hover:bg-accent hover:text-accent-foreground"
          >
            <Compass className="h-4 w-4" />
            Replay
          </button>
        }
        details="It walks through a sample meeting: the transcript, the source-linked summary, and starting your first recording."
        detailsLabel="What the tour covers"
      />

      {/* What's new (G2). The update dialog shows itself once; this is how a
          user reads it again after dismissing it. */}
      <SettingCard
        title="What's new"
        description={`What changed in version ${APP_VERSION}, including what is still off by default.`}
        action={
          <button
            onClick={() => {
              void Analytics.trackButtonClick('open_whats_new', 'settings');
              setWhatsNewOpen(true);
            }}
            className="inline-flex items-center gap-2 rounded-md border border-input bg-background px-4 py-2 text-meta font-medium transition-colors hover:bg-accent hover:text-accent-foreground"
          >
            <Sparkles className="h-4 w-4" />
            Open
          </button>
        }
      />
      <WhatsNew open={whatsNewOpen} onOpenChange={setWhatsNewOpen} />

      <SettingCard
        title="Where your data is stored"
        description="Everything stays on this computer."
        details={
          <>
            <p>
              The database and the transcription models live together in the application data
              directory.
            </p>
            {/* This stays a claim the user can read, not a claim that was cut.
                Deleting a meeting has limits, and saying so is an honesty
                obligation — folding it keeps the sentence, it does not remove
                it (the summary above states the limit in one line). */}
            <p>
              Deleting a meeting removes the database, search, recording and recovery-cache data
              Mityu manages. Copies outside Mityu can survive it: SSD wear-levelling,
              copy-on-write filesystems, snapshots, backups, exports and browser storage are not
              something the app can reach.
            </p>
          </>
        }
        detailsLabel="What deleting a meeting does and does not erase"
      >
        <div className="rounded-lg border border-border bg-muted p-4">
          <div className="text-meta font-medium text-foreground">Meeting recordings</div>
          <div className="mt-1 break-all font-mono text-caption text-muted-foreground">
            {storageLocations?.recordings || 'Loading...'}
          </div>
          <button
            onClick={() => handleOpenFolder('recordings')}
            className="mt-3 flex items-center gap-2 rounded-md border border-border px-3 py-2 text-meta transition-colors hover:bg-background"
          >
            <FolderOpen className="h-4 w-4" />
            Open folder
          </button>
        </div>
      </SettingCard>

      {/* Recording Consent Section */}
      <div className="bg-card rounded-lg border border-border p-6 shadow-sm">
        <RecordingConsentSettings />
      </div>

      {/* Redaction Section */}
      <div className="bg-card rounded-lg border border-border p-6 shadow-sm">
        <RedactionSettings />
      </div>

      {/* Learning Section */}
      <div className="bg-card rounded-lg border border-border p-6 shadow-sm">
        <LearningSettings />
      </div>

      {/* Analytics Section */}
      <div className="bg-card rounded-lg border border-border p-6 shadow-sm">
        <AnalyticsConsentSwitch />
      </div>
    </div>
  )
}
