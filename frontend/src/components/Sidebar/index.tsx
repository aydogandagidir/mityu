'use client';

/**
 * The application shell's left side — DESIGN_SYSTEM.md §3.2.
 *
 * This file used to be 893 lines that rendered two different navigations depending on
 * one boolean: a rail of icons when collapsed, and a list with its own duplicate Home,
 * Actions and record controls when expanded. It is now a composition of two components
 * with one job each — `AppRail` (navigation, always visible) and `MeetingsPane` (the
 * library, the user's choice) — and the dead weight it carried is gone.
 *
 * WHAT WAS REMOVED, AND WHY IT IS SAFE. The old file also held a model-config fetch, a
 * transcript-config fetch, a `model-config-updated` listener and two save handlers.
 * None of them reached the screen: they fed `modelConfig` / `transcriptModelConfig`
 * state whose only consumer was `SettingTabs`, which this file imported and never
 * rendered — and ADR-0045 already recorded that `SettingTabs` is rendered by nothing at
 * all. The two save handlers were never called, so
 * `Analytics.trackSettingsChanged('transcript_config')` was an identifier that could not
 * fire; `…('model_config')` still fires from `hooks/meeting-details/useModelConfiguration.ts`,
 * which is a live path. Giving the transcript config a real save belongs to the settings
 * work, and the identifier belongs with it.
 *
 * 🔒 `window.openSettings` is kept — `src-tauri/src/tray.rs` calls it — and now does what
 * its name says. It previously set a piece of state that nothing rendered, so the tray's
 * Settings item silently did nothing from this side.
 */

import React, { useEffect } from 'react';
import { useRouter } from 'next/navigation';
import { AppRail } from '@/components/shell/AppRail';
import { MeetingsPane } from '@/components/shell/MeetingsPane';

const Sidebar: React.FC = () => {
  const router = useRouter();

  // 🔒 Rust → webview contract (tray.rs). Installed for the lifetime of the shell.
  useEffect(() => {
    (window as unknown as { openSettings?: () => void }).openSettings = () => {
      router.push('/settings');
    };
    return () => {
      delete (window as unknown as { openSettings?: () => void }).openSettings;
    };
  }, [router]);

  return (
    <div className="flex shrink-0">
      <AppRail />
      <MeetingsPane />
    </div>
  );
};

export default Sidebar;
