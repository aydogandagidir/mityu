// @vitest-environment jsdom

/**
 * Settings → Beta → Live copilot: can a user work out what to do next?
 *
 * The report that produced these tests was "Copilot nasıl kullanılıyor
 * anlayamadım" — I could not work out how to use the Copilot. The switch
 * existed and did its job; what was missing was the next step. A user who
 * turns it on has to learn two things that nothing told them: the panel opens
 * on a shortcut, and it stays empty until a recording is running.
 *
 * So these assert the instruction is present AND that it names the binding the
 * user actually has, rather than a hard-coded one that drifts the moment
 * somebody rebinds it.
 */

import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import CopilotSettings from './CopilotSettings';
import { copilotService } from '@/services/copilotService';
import type { CopilotStatus } from '@/types/copilot';

function status(over: Partial<CopilotStatus> = {}): CopilotStatus {
  return {
    config: {
      enabled: true,
      contentProtection: true,
      shortcutsEnabled: true,
      keybinds: {
        togglePanel: 'CommandOrControl+Shift+M',
        ask: 'CommandOrControl+Shift+A',
        captureScreen: 'CommandOrControl+Shift+S',
      },
      liveWindowSecs: 180,
      allowCloudInsights: false,
      ...(over.config ?? {}),
    },
    protection: {
      level: 'enforced',
      headline: 'Hidden from screen shares',
      detail: 'The panel is excluded from screen capture.',
    },
    panelOpen: false,
    recording: false,
    shortcuts: [],
    liveContext: { subscribed: false, windowSecs: 180, turns: 0, windowTurns: 0, evicted: 0 },
    liveActions: [],
    activeModeId: 'general',
    activeModeName: 'General',
    pinnedClaimIds: [],
    pendingPins: 0,
    ...over,
  };
}

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

describe('what a user is told after switching the copilot on', () => {
  it('names the shortcut that actually opens the panel', async () => {
    vi.spyOn(copilotService, 'getStatus').mockResolvedValue(status());
    render(<CopilotSettings />);
    expect(await screen.findByText('Ctrl+Shift+M')).toBeTruthy();
  });

  it('shows the user their own binding, not a hard-coded one', async () => {
    vi.spyOn(copilotService, 'getStatus').mockResolvedValue(
      status({
        config: {
          ...status().config,
          keybinds: {
            togglePanel: 'Alt+F9',
            ask: 'CommandOrControl+Shift+A',
            captureScreen: 'CommandOrControl+Shift+S',
          },
        },
      }),
    );
    render(<CopilotSettings />);
    expect(await screen.findByText('Alt+F9')).toBeTruthy();
    expect(screen.queryByText('Ctrl+Shift+M')).toBeNull();
  });

  /**
   * The panel is empty without a recording. Saying so here is the difference
   * between "it works" and "it looks broken".
   */
  it('says the panel stays empty until a recording is running', async () => {
    vi.spyOn(copilotService, 'getStatus').mockResolvedValue(status());
    render(<CopilotSettings />);
    expect(await screen.findByText(/Start a recording/i)).toBeTruthy();
    expect(screen.getByText(/empty until one is running/i)).toBeTruthy();
  });

  /**
   * The switch that had no interface.
   *
   * `allowCloudInsights` was enforced on every insight request and settable
   * nowhere, so a user whose summary provider is a cloud one was refused on
   * every action press and pointed at a setting that did not exist. The feature
   * was untestable for them, which is precisely the report this work started
   * from.
   */
  it('exposes the cloud-insights switch', async () => {
    vi.spyOn(copilotService, 'getStatus').mockResolvedValue(status());
    render(<CopilotSettings />);
    const cloud = await screen.findByRole('switch', {
      name: 'Allow live insights to be sent to a cloud provider',
    });
    expect(cloud.getAttribute('aria-checked')).toBe('false');
  });

  it('says what turning cloud insights on actually does', async () => {
    vi.spyOn(copilotService, 'getStatus').mockResolvedValue(status());
    render(<CopilotSettings />);
    const copy = await screen.findByText(/Off by default/i);
    // The two facts a user needs before authorising it: what leaves, and that
    // it then happens without another prompt.
    expect(copy.textContent).toMatch(/last\s+few minutes/i);
    expect(copy.textContent).toMatch(/no further prompt/i);
  });

  it('persists the cloud-insights choice through the backend', async () => {
    vi.spyOn(copilotService, 'getStatus').mockResolvedValue(status());
    const setConfig = vi
      .spyOn(copilotService, 'setConfig')
      .mockResolvedValue(status({ config: { ...status().config, allowCloudInsights: true } }));
    render(<CopilotSettings />);
    fireEvent.click(
      await screen.findByRole('switch', {
        name: 'Allow live insights to be sent to a cloud provider',
      }),
    );
    await waitFor(() => expect(setConfig).toHaveBeenCalledTimes(1));
    expect(setConfig.mock.calls[0][0].allowCloudInsights).toBe(true);
  });

  it('offers no instruction and no panel button while the copilot is off', async () => {
    vi.spyOn(copilotService, 'getStatus').mockResolvedValue(
      status({ config: { ...status().config, enabled: false } }),
    );
    render(<CopilotSettings />);
    // Wait for the loaded state before asserting an absence.
    expect(await screen.findByRole('switch', { name: 'Enable the live copilot panel' })).toBeTruthy();
    expect(screen.queryByText(/Start a recording/i)).toBeNull();
    expect(screen.queryByRole('button', { name: /Show the panel/i })).toBeNull();
  });
});
