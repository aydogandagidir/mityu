'use client';

/**
 * Settings → Beta → Live copilot (BACKLOG I1, ADR-0038; moved there by ADR-0045).
 *
 * Three things the user controls: whether the copilot exists at all, whether it
 * asks the OS to keep it out of screen captures, and its shortcut.
 *
 * The tab is written to make two facts unmissable, because both are places
 * where a competitor's product makes a promise Mityu will not:
 *
 * - **The screen-sharing sentence comes from the backend**, per platform, and is
 *   shown whether the setting is on or off. On macOS 15+ it says the panel can
 *   still be captured; on Linux it says there is no mechanism at all. The UI
 *   never composes its own wording for this, so the copy cannot drift from what
 *   the platform does.
 * - **A shortcut row says whether the OS is actually holding the combination.**
 *   Ask and Capture-screen have no implementation yet (I3, I6), so their
 *   bindings are stored and shown but never registered — and the row says why,
 *   rather than looking like a shortcut that silently does not work.
 */

import { useCallback, useEffect, useState } from 'react';
import { Switch } from '@/components/ui/switch';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { AlertTriangle, Cloud, Info, Loader2, PanelRight, ShieldCheck } from 'lucide-react';
import { copilotService } from '@/services/copilotService';
import type { CopilotConfig, CopilotStatus } from '@/types/copilot';

export default function CopilotSettings() {
  const [status, setStatus] = useState<CopilotStatus | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [loadFailed, setLoadFailed] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [draftKeybind, setDraftKeybind] = useState('');

  useEffect(() => {
    let cancelled = false;
    const load = async () => {
      try {
        const loaded = await copilotService.getStatus();
        if (cancelled) return;
        setStatus(loaded);
        setDraftKeybind(loaded.config.keybinds.togglePanel);
      } catch (e) {
        console.error('Failed to load copilot settings:', e);
        if (!cancelled) setLoadFailed(true);
      } finally {
        if (!cancelled) setIsLoading(false);
      }
    };
    load();
    return () => {
      cancelled = true;
    };
  }, []);

  const persist = useCallback(
    async (next: CopilotConfig) => {
      setIsSaving(true);
      setError(null);
      try {
        const updated = await copilotService.setConfig(next);
        setStatus(updated);
        setDraftKeybind(updated.config.keybinds.togglePanel);
      } catch (e) {
        // The backend validates every binding before storing any of them, so a
        // rejected save changed nothing — showing the previous state is honest.
        setError(String(e));
      } finally {
        setIsSaving(false);
      }
    },
    []
  );

  if (isLoading) {
    return (
      <div className="flex items-center gap-2 p-4 text-sm text-muted-foreground">
        <Loader2 className="h-4 w-4 animate-spin" /> Loading copilot settings…
      </div>
    );
  }

  if (loadFailed || !status) {
    return (
      <div className="flex items-start gap-2 p-4 text-sm text-muted-foreground">
        <AlertTriangle className="mt-0.5 h-4 w-4 shrink-0" />
        <span>Copilot settings could not be loaded.</span>
      </div>
    );
  }

  const { config, protection, shortcuts } = status;

  return (
    <div className="space-y-6 p-1">
      <section className="space-y-3 rounded-lg border border-border p-4">
        <div className="flex items-start justify-between gap-4">
          <div className="space-y-1">
            <h3 className="text-sm font-medium text-foreground">Live copilot panel</h3>
            <p className="max-w-xl text-xs text-muted-foreground">
              A small always-on-top panel that follows a recording you start and mirrors the live
              transcript. It never records on its own, and it is off until you turn it on.
            </p>
          </div>
          <Switch
            checked={config.enabled}
            disabled={isSaving}
            onCheckedChange={(enabled) => persist({ ...config, enabled })}
            aria-label="Enable the live copilot panel"
          />
        </div>

        {config.enabled && (
          <div className="space-y-2">
            {/* The one instruction a user actually needs, with the binding they
                actually have. "I could not work out how to use the Copilot" was
                the report; a switch with no next step is what produced it. The
                panel is empty without a recording, so say that here rather than
                letting the user discover it by opening an empty panel. */}
            <p className="text-xs text-muted-foreground">
              Start a recording, then press{' '}
              <kbd className="rounded border border-border bg-muted px-1 py-0.5 font-mono text-[10px]">
                {config.keybinds.togglePanel.replace('CommandOrControl', 'Ctrl')}
              </kbd>{' '}
              to open the panel. It answers only from the meeting you are recording, so it stays
              empty until one is running.
            </p>
            <Button
              variant="outline"
              size="sm"
              onClick={() => void copilotService.togglePanel()}
              className="gap-1"
            >
              <PanelRight className="h-3.5 w-3.5" /> Show the panel
            </Button>
          </div>
        )}
      </section>

      {/* Where a live insight may be sent.
          *
          * This switch existed in the config, was enforced on every request,
          * and had NO user interface anywhere. A user whose summary provider is
          * a cloud one (OpenAI, Anthropic, Groq, a remote Ollama) was refused on
          * every single action press, told to change a setting, and given no
          * setting to change — the feature was untestable for them and there was
          * no way to find that out from inside the app.
          *
          * It stays OFF by default and the wording says plainly what turning it
          * on means, because a live insight fires from a hotkey mid-sentence:
          * the user is authorising something that then happens without another
          * prompt. */}
      <section className="space-y-3 rounded-lg border border-border p-4">
        <div className="flex items-start justify-between gap-4">
          <div className="space-y-1">
            <h3 className="flex items-center gap-1.5 text-sm font-medium text-foreground">
              <Cloud className="h-4 w-4" /> Allow cloud insights
            </h3>
            <p className="max-w-xl text-xs text-muted-foreground">
              Off by default. While it is off, the copilot answers only with a model running on this
              computer, and refuses rather than sending anything away. Turning it on lets the last
              few minutes of the conversation be sent to the AI provider you configured under
              Summary — on every press of an action button, with no further prompt.
            </p>
          </div>
          <Switch
            checked={config.allowCloudInsights}
            disabled={isSaving}
            onCheckedChange={(allowCloudInsights) => persist({ ...config, allowCloudInsights })}
            aria-label="Allow live insights to be sent to a cloud provider"
          />
        </div>
      </section>

      <section className="space-y-3 rounded-lg border border-border p-4">
        <div className="flex items-start justify-between gap-4">
          <div className="space-y-1">
            <h3 className="flex items-center gap-1.5 text-sm font-medium text-foreground">
              <ShieldCheck className="h-4 w-4" /> Keep the panel out of screen shares
            </h3>
            <p className="max-w-xl text-xs text-muted-foreground">
              So your private notes do not appear when you share your screen. Mityu never hides
              itself from you or from anyone looking at your computer — the app stays in the
              taskbar and the recording indicator stays visible.
            </p>
          </div>
          <Switch
            checked={config.contentProtection}
            disabled={isSaving}
            onCheckedChange={(contentProtection) => persist({ ...config, contentProtection })}
            aria-label="Keep the copilot panel out of screen shares"
          />
        </div>

        {/* The platform's own answer, in the backend's words. Shown whatever the
            switch says, because it is a fact about this computer. */}
        <p className="flex items-start gap-2 rounded-md bg-muted/60 p-3 text-xs text-muted-foreground">
          <Info className="mt-0.5 h-3.5 w-3.5 shrink-0" />
          <span>
            <span className="font-medium text-foreground">{protection.headline}.</span>{' '}
            {protection.detail}
          </span>
        </p>
      </section>

      <section className="space-y-3 rounded-lg border border-border p-4">
        <div className="flex items-start justify-between gap-4">
          <div className="space-y-1">
            <h3 className="text-sm font-medium text-foreground">Keyboard shortcuts</h3>
            <p className="max-w-xl text-xs text-muted-foreground">
              A global shortcut works while any application is in front, so Mityu asks the system
              to reserve the combination. Turn them all off if you would rather not.
            </p>
          </div>
          <Switch
            checked={config.shortcutsEnabled}
            disabled={isSaving}
            onCheckedChange={(shortcutsEnabled) => persist({ ...config, shortcutsEnabled })}
            aria-label="Enable global shortcuts"
          />
        </div>

        <div className="space-y-2">
          <label htmlFor="copilot-toggle-keybind" className="text-xs font-medium text-foreground">
            Show or hide the panel
          </label>
          <div className="flex items-center gap-2">
            <Input
              id="copilot-toggle-keybind"
              value={draftKeybind}
              spellCheck={false}
              disabled={isSaving}
              onChange={(e) => setDraftKeybind(e.target.value)}
              className="max-w-xs font-mono text-xs"
              placeholder="CommandOrControl+Shift+M"
            />
            <Button
              size="sm"
              variant="outline"
              disabled={isSaving || draftKeybind === config.keybinds.togglePanel}
              onClick={() =>
                persist({
                  ...config,
                  keybinds: { ...config.keybinds, togglePanel: draftKeybind },
                })
              }
            >
              Save
            </Button>
          </div>
          {error && (
            <p role="alert" className="text-xs text-destructive">
              {error}
            </p>
          )}
        </div>

        <ul className="space-y-1 text-xs text-muted-foreground">
          {shortcuts.map((shortcut) => (
            <li key={shortcut.action} className="flex flex-wrap items-center gap-2">
              <span className="font-mono text-[11px] text-foreground">{shortcut.keybind}</span>
              <span>{shortcut.label}</span>
              {shortcut.registered ? (
                <span className="text-emerald-600 dark:text-emerald-400">· active</span>
              ) : (
                <span>
                  · not active{shortcut.unavailableReason ? ` — ${shortcut.unavailableReason}` : ''}
                </span>
              )}
            </li>
          ))}
        </ul>
      </section>
    </div>
  );
}
