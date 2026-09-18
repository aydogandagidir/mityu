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
import { SettingsCard } from "@/components/settings/SettingsCard"
import { SwitchRow } from "@/components/settings/SwitchRow"
import { Button } from "@/components/ui/button"
import { WhatsNew } from "./WhatsNew"
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
      <SettingsCard
        title="Appearance"
        description="Follow your system theme, or force light or dark."
        action={<ThemeToggle />}
      />

      <SettingsCard>
        <SwitchRow
          label="Meeting start and end notifications"
          description="A system notification when a recording starts and when it finishes saving. Separate from the participant reminder in Recording."
          checked={notificationsEnabledValue}
          onCheckedChange={setNotificationsEnabled}
        />
      </SettingsCard>

      {/* Ported from main: the update dialog shows itself once, and this is how a user
          reads it again after dismissing it. Rebuilt on the SettingsCard primitive so it
          matches the rest of the column (§6.5) rather than main's hand-rolled button. */}
      <SettingsCard
        title="What's new"
        description={`What changed in version ${APP_VERSION}, including what is still off by default.`}
        action={
          <Button
            variant="outline"
            onClick={() => {
              void Analytics.trackButtonClick('open_whats_new', 'settings');
              setWhatsNewOpen(true);
            }}
          >
            <Sparkles className="size-4" aria-hidden="true" />
            Open
          </Button>
        }
      />
      <WhatsNew open={whatsNewOpen} onOpenChange={setWhatsNewOpen} />

      <SettingsCard
        title="Product tour"
        description="Replay the guided walkthrough on the sample meeting — transcript, source-linked summary, and your first recording."
        action={
          <Button
            variant="outline"
            onClick={() => {
              void Analytics.trackButtonClick('replay_product_tour', 'settings');
              replayTour();
            }}
          >
            <Compass className="size-4" aria-hidden="true" />
            Replay product tour
          </Button>
        }
      />
    </div>
  )
}

/**
 * Where this app keeps things, and what deleting a meeting can and cannot reach.
 *
 * 🔒 The erasure disclosure is verbatim. It lives in Privacy now, beside consent and
 * redaction, rather than in a "General" tab it had nothing to do with — and the
 * recordings path it used to print is no longer duplicated here: Recording owns the save
 * location, which is where a person looks for it.
 */
export function StorageLocations() {
  const { storageLocations, isLoadingPreferences, loadPreferences } = useConfig()

  useEffect(() => {
    void loadPreferences()
  }, [loadPreferences])

  return (
    <SettingsCard
      title="Where your data is stored"
      description="Everything stays on this device."
    >
      <div className="space-y-3">
        <div className="rounded-md border border-border bg-surface-2 p-3">
          <div className="text-label text-foreground">Meeting recordings</div>
          <div className="mt-1 break-all font-mono text-caption text-muted-foreground">
            {isLoadingPreferences && !storageLocations
              ? 'Reading…'
              : storageLocations?.recordings || 'Default folder'}
          </div>
          <p className="mt-2 text-caption text-muted-foreground">
            The database and downloaded models sit together in your application data
            directory. Change the recordings folder in Recording.
          </p>
        </div>

        <p className="text-caption text-muted-foreground">
          Meeting deletion covers Mityu-managed database/search, recording, and
          recovery-cache data. Physical traces or separate copies may remain on SSD
          wear-leveling, copy-on-write filesystems, snapshots, backups, exports, or
          WebView/browser storage; Mityu cannot erase those external layers.
        </p>
      </div>
    </SettingsCard>
  )
}
