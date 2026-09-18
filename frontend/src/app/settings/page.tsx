'use client';

/**
 * Settings — DESIGN_SYSTEM.md §6.5.
 *
 * It was six tabs held in `useState`, so nothing could deep-link to a section and the
 * back control was `router.back()` sitting beside a permanent navigation rail. "General"
 * was nine unrelated cards in one column: appearance, notifications, the product tour,
 * storage paths, recording consent, redaction, the learning loop and analytics.
 *
 * Now each section is a URL — `/settings?section=privacy` — so an empty state, a toast or
 * the tray can send someone exactly where the setting is, and the sections are grouped by
 * what a person is trying to do rather than by which component was written first.
 *
 * TWO SETTINGS EXISTED TWICE. The recordings folder was printed in both General and
 * Recording, and two different switches called themselves notifications while writing
 * different stores. The folder now lives only where a person looks for it (Recording),
 * and the two switches say which is which: meeting start and end notifications in
 * General, the participant reminder in Recording.
 */

import React, { Suspense, useEffect } from 'react';
import { useRouter, useSearchParams } from 'next/navigation';
import {
  FlaskConical,
  GraduationCap,
  Info,
  KeyRound,
  Mic,
  Settings2,
  ShieldCheck,
  Sparkles,
  Waves,
} from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { PreferenceSettings, StorageLocations } from '@/components/PreferenceSettings';
import { RecordingSettings } from '@/components/RecordingSettings';
import { TranscriptSettings } from '@/components/TranscriptSettings';
import { SummaryModelSettings } from '@/components/SummaryModelSettings';
import { BetaSettings } from '@/components/BetaSettings';
import { LicenseSettings } from '@/components/licensing/LicenseSettings';
import RecordingConsentSettings from '@/components/RecordingConsentSettings';
import RedactionSettings from '@/components/RedactionSettings';
import LearningSettings from '@/components/LearningSettings';
import AnalyticsConsentSwitch from '@/components/AnalyticsConsentSwitch';
import { About } from '@/components/About';
import { SettingsCard } from '@/components/settings/SettingsCard';
import { PageHeader } from '@/components/shell/PageHeader';
import { useConfig } from '@/contexts/ConfigContext';
import { focusRing } from '@/components/ui/focus-ring';
import { cn } from '@/lib/utils';
import { resolveTabFromSearch } from './tabs';

const SECTIONS = [
  { id: 'general', label: 'General', icon: Settings2 },
  { id: 'recording', label: 'Recording', icon: Mic },
  { id: 'transcription', label: 'Transcription', icon: Waves },
  { id: 'summary', label: 'Summary', icon: Sparkles },
  { id: 'privacy', label: 'Privacy', icon: ShieldCheck },
  { id: 'learning', label: 'Learning', icon: GraduationCap },
  { id: 'beta', label: 'Beta', icon: FlaskConical },
  { id: 'license', label: 'License', icon: KeyRound },
  { id: 'about', label: 'About', icon: Info },
] as const;

/**
 * `tray.rs:79` hard-codes `/settings?tab=beta` for the copilot switch, written against the
 * old TABS list. The redesign's URL is `?section=`, so the old names are translated rather
 * than broken — a Rust-side link has no compile-time tie to this file, which is exactly how
 * it would have failed silently.
 */
const LEGACY_TAB_TO_SECTION: Record<string, SectionId> = {
  general: 'general',
  recording: 'recording',
  Transcriptionmodels: 'transcription',
  summaryModels: 'summary',
  beta: 'beta',
  license: 'license',
};

type SectionId = (typeof SECTIONS)[number]['id'];

function isSection(value: string | null): value is SectionId {
  return !!value && SECTIONS.some((s) => s.id === value);
}

function SettingsContent() {
  const router = useRouter();
  const params = useSearchParams();
  const { transcriptModelConfig, setTranscriptModelConfig } = useConfig();

  const section: SectionId = isSection(params.get('section'))
    ? (params.get('section') as SectionId)
    : 'general';

  // Translate a legacy `?tab=` link into this page's `?section=`. In an effect, never in
  // render: this app ships as a static export, and resolving the query while rendering is
  // what made the previous version throw "Hydration failed..." and rebuild the tree.
  useEffect(() => {
    if (params.get('section')) return;
    const legacy = resolveTabFromSearch(window.location.search);
    const mapped = legacy ? LEGACY_TAB_TO_SECTION[legacy] : undefined;
    if (mapped) router.replace(`/settings?section=${mapped}`);
  }, [params, router]);

  // Load saved transcript configuration on mount (unchanged behaviour).
  useEffect(() => {
    const loadTranscriptConfig = async () => {
      try {
        const config = (await invoke('api_get_transcript_config')) as any;
        if (config) {
          setTranscriptModelConfig({
            provider: config.provider || 'localWhisper',
            model: config.model || 'large-v3',
            apiKey: config.apiKey || null,
          });
        }
      } catch (error) {
        console.error('Failed to load transcript config:', error);
      }
    };
    loadTranscriptConfig();
  }, [setTranscriptModelConfig]);

  const current = SECTIONS.find((s) => s.id === section)!;

  return (
    <div className="flex min-h-0 flex-1 flex-col overflow-hidden">
      <PageHeader eyebrow="Settings" title={current.label} />

      <div className="flex min-h-0 flex-1 overflow-hidden">
        {/* The section column lives INSIDE the content pane, so navigating settings never
            opens or closes the meetings pane as a side effect. */}
        <nav
          aria-label="Settings sections"
          className="w-[13rem] shrink-0 overflow-y-auto border-r border-border p-2"
        >
          <ul className="space-y-0.5">
            {SECTIONS.map((s) => {
              const Icon = s.icon;
              const active = s.id === section;
              return (
                <li key={s.id}>
                  <button
                    type="button"
                    onClick={() => router.push(`/settings?section=${s.id}`)}
                    aria-current={active ? 'page' : undefined}
                    className={cn(
                      'flex w-full items-center gap-2 rounded-md px-3 py-2 text-left text-body',
                      'transition-colors duration-fast ease-out',
                      focusRing('background'),
                      active
                        ? 'bg-accent font-medium text-accent-foreground'
                        : 'text-foreground hover:bg-muted'
                    )}
                  >
                    <Icon className="size-4 shrink-0" aria-hidden="true" />
                    {s.label}
                  </button>
                </li>
              );
            })}
          </ul>
        </nav>

        <div className="min-h-0 flex-1 overflow-y-auto">
          <div className="mx-auto w-full max-w-3xl space-y-4 p-gutter">
            {section === 'general' && <PreferenceSettings />}

            {section === 'recording' && <RecordingSettings />}

            {section === 'transcription' && (
              <TranscriptSettings
                transcriptModelConfig={transcriptModelConfig}
                setTranscriptModelConfig={setTranscriptModelConfig}
              />
            )}

            {section === 'summary' && <SummaryModelSettings />}

            {/* Consent, redaction, analytics and storage in one place: everything that
                decides what leaves this device, or what is kept on it. */}
            {section === 'privacy' && (
              <>
                <SettingsCard>
                  <RecordingConsentSettings />
                </SettingsCard>
                <SettingsCard>
                  <RedactionSettings />
                </SettingsCard>
                <SettingsCard>
                  <AnalyticsConsentSwitch />
                </SettingsCard>
                <StorageLocations />
              </>
            )}

            {section === 'learning' && (
              <SettingsCard>
                <LearningSettings />
              </SettingsCard>
            )}

            {section === 'beta' && <BetaSettings />}

            {section === 'license' && <LicenseSettings />}

            {section === 'about' && (
              <SettingsCard>
                <About />
              </SettingsCard>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}

export default function SettingsPage() {
  return (
    <Suspense fallback={null}>
      <SettingsContent />
    </Suspense>
  );
}
