'use client';

/**
 * /design/settings — the settings shell and its primitives, Tauri-free.
 *
 * The section column and the cards are what this package changed; the individual setting
 * components below them each read their own store through `invoke` on mount and cannot
 * run in a browser, so this renders the SHELL with representative cards rather than
 * mounting them. What it proves is the thing WP12 built: the navigation model, the card
 * and switch primitives, and that both themes hold.
 */

import React, { useState } from 'react';
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
import { PageHeader } from '@/components/shell/PageHeader';
import { SettingsCard } from '@/components/settings/SettingsCard';
import { SwitchRow } from '@/components/settings/SwitchRow';
import { TranscriptSettings, type TranscriptModelProps } from '@/components/TranscriptSettings';
import { ThemeToggle } from '@/components/ThemeToggle';
import { Button } from '@/components/ui/button';
import { Notice } from '@/components/ui/notice';
import { focusRing } from '@/components/ui/focus-ring';
import { cn } from '@/lib/utils';

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
];

function Shell({ active }: { active: string }) {
  const [notify, setNotify] = useState(true);
  const [reminder, setReminder] = useState(true);
  const [redactPii, setRedactPii] = useState(true);
  const [analyticsOptIn, setAnalyticsOptIn] = useState(false);
  const [transcriptConfig, setTranscriptConfig] = useState<TranscriptModelProps>({
    provider: 'nemotron',
    model: JSON.stringify({
      model: 'nvidia/NVIDIA-Nemotron-3.5-Transcribe-30B-A4B',
      endpoint: 'http://127.0.0.1:8000/v1',
      speaker_attribution: true,
      inline_pii_redaction: true,
      target_language: 'Turkish',
      context_prompt: 'Mityu, SQLCipher, Sortformer, KVKK, Q3',
    }),
    apiKey: null,
    hasApiKey: true,
  });
  const current = SECTIONS.find((s) => s.id === active) ?? SECTIONS[0];

  return (
    <div
      className={cn(
        'flex flex-col overflow-hidden rounded-lg border border-border bg-background',
        active === 'transcription' ? 'h-[900px]' : 'h-[640px]'
      )}
    >
      <PageHeader eyebrow="Settings" title={current.label} />
      <div className="flex min-h-0 flex-1 overflow-hidden">
        <nav
          aria-label="Settings sections"
          className="w-[13rem] shrink-0 overflow-y-auto border-r border-border p-2"
        >
          <ul className="space-y-0.5">
            {SECTIONS.map((s) => {
              const Icon = s.icon;
              const isActive = s.id === active;
              return (
                <li key={s.id}>
                  <button
                    type="button"
                    aria-current={isActive ? 'page' : undefined}
                    className={cn(
                      'flex w-full items-center gap-2 rounded-md px-3 py-2 text-left text-body',
                      focusRing('background'),
                      isActive
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
            {active === 'transcription' ? (
              <TranscriptSettings
                transcriptModelConfig={transcriptConfig}
                setTranscriptModelConfig={setTranscriptConfig}
              />
            ) : active === 'general' ? (
              <>
                <SettingsCard
                  title="Appearance"
                  description="Follow your system theme, or force light or dark."
                  action={<ThemeToggle />}
                />
                <SettingsCard>
                  <SwitchRow
                    label="Meeting start and end notifications"
                    description="A system notification when a recording starts and when it finishes saving. Separate from the participant reminder in Recording."
                    checked={notify}
                    onCheckedChange={setNotify}
                  />
                </SettingsCard>
                <SettingsCard
                  title="Product tour"
                  description="Replay the guided walkthrough on the sample meeting."
                  action={<Button variant="outline">Replay product tour</Button>}
                />
              </>
            ) : (
              <>
                <SettingsCard title="Recording consent">
                  <Notice tone="consent" title="Mityu cannot verify consent for you">
                    Recording laws vary by jurisdiction. This reminder is not legal advice.
                  </Notice>
                </SettingsCard>
                <SettingsCard>
                  <div className="divide-y [&>*+*]:border-t-border">
                    <SwitchRow
                      label="Participant reminder"
                      description="Show the reminder to inform participants when a recording starts."
                      checked={reminder}
                      onCheckedChange={setReminder}
                    />
                    <SwitchRow
                      label="Redact sensitive content"
                      description="Replace personal emails, phone numbers, and IDs with [REDACTED_*] tokens before summaries or exports leave the transcript."
                      checked={redactPii}
                      onCheckedChange={setRedactPii}
                    />
                    <SwitchRow
                      label="Usage analytics is off by default"
                      description="No telemetry is sent unless you explicitly opt in. Meeting audio and transcripts are never included."
                      checked={analyticsOptIn}
                      onCheckedChange={setAnalyticsOptIn}
                    />
                  </div>
                </SettingsCard>
                <SettingsCard
                  title="Where your data is stored"
                  description="Everything stays on this device."
                >
                  <div className="rounded-md border border-border bg-surface-2 p-3">
                    <div className="text-label text-foreground">Meeting recordings</div>
                    <div className="mt-1 break-all font-mono text-caption text-muted-foreground">
                      ~/Movies/mityu-recordings
                    </div>
                  </div>
                </SettingsCard>
              </>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}

function Panel({ theme }: { theme: 'light' | 'dark' }) {
  return (
    <div className={theme === 'dark' ? 'dark' : ''}>
      <div className="space-y-6 bg-background p-6 text-foreground">
        <header className="space-y-1">
          <div className="text-eyebrow uppercase text-subtle-foreground">{theme} theme</div>
          <h1 className="text-display">Settings</h1>
          <p className="text-body text-muted-foreground">
            §6.5. Nine sections, each its own URL — where there were six tabs held in
            component state and a nine-card General dump.
          </p>
        </header>
        <section className="space-y-2">
          <h2 className="text-title">Transcription — NVIDIA Nemotron 3.5 Transcribe (30B-A4B MoE)</h2>
          <Shell active="transcription" />
        </section>
        <section className="space-y-2">
          <h2 className="text-title">General</h2>
          <Shell active="general" />
        </section>
        <section className="space-y-2">
          <h2 className="text-title">Privacy</h2>
          <Shell active="privacy" />
        </section>
      </div>
    </div>
  );
}

export default function DesignSettingsPage() {
  return (
    <div>
      <Panel theme="light" />
      <Panel theme="dark" />
    </div>
  );
}
