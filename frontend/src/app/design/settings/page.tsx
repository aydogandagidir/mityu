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
  const current = SECTIONS.find((s) => s.id === active) ?? SECTIONS[0];

  return (
    <div className="flex h-[560px] flex-col overflow-hidden rounded-lg border border-border bg-background">
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
            {active === 'general' ? (
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
                  <SwitchRow
                    label="Participant reminder"
                    description="Show the reminder to inform participants when a recording starts."
                    checked={reminder}
                    onCheckedChange={setReminder}
                  />
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
