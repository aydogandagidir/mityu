// @vitest-environment jsdom

/**
 * Settings → Beta is where the live copilot can be turned on (ADR-0045).
 *
 * v1.2.0 shipped `CopilotSettings` and `ModesSettings` mounted only inside
 * `SettingTabs.tsx`, a component that nothing in the app renders — so the one
 * switch that sets `enabled: true` could not be reached, and the whole of
 * EPIC I was invisible. These tests pin the wiring, not the children: the
 * children have their own suites. Remove either `<CopilotSettings />` or
 * `<ModesSettings />` from `BetaSettings` and the matching test fails.
 */

import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import { BetaSettings } from './BetaSettings';
import { copilotService } from '@/services/copilotService';
import { modesService } from '@/services/modesService';
import type { CopilotStatus } from '@/types/copilot';

vi.mock('@/contexts/ConfigContext', () => ({
  useConfig: () => ({
    betaFeatures: { importAndRetranscribe: false, structuredSummaries: true },
    toggleBetaFeature: vi.fn(),
  }),
}));

const STATUS: CopilotStatus = {
  config: {
    enabled: false,
    contentProtection: true,
    shortcutsEnabled: true,
    keybinds: { togglePanel: 'CommandOrControl+Shift+M', ask: 'CommandOrControl+Shift+A', captureScreen: 'CommandOrControl+Shift+S' },
    liveWindowSecs: 180,
    allowCloudInsights: false,
  },
  protection: { level: 'enforced', headline: 'Hidden from screen shares', detail: 'The panel is excluded from screen capture.' },
  panelOpen: false,
  recording: false,
  shortcuts: [],
  liveContext: { subscribed: false, windowSecs: 180, turns: 0, windowTurns: 0, evicted: 0 },
  liveActions: [],
  activeModeId: 'general',
  activeModeName: 'General',
};

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

describe('the copilot is reachable from Settings → Beta', () => {
  it('mounts the copilot switch', async () => {
    vi.spyOn(copilotService, 'getStatus').mockResolvedValue(STATUS);
    vi.spyOn(modesService, 'list').mockResolvedValue({ modes: [], activeModeId: 'general' });
    render(<BetaSettings />);
    expect(await screen.findByRole('switch', { name: 'Enable the live copilot panel' })).toBeTruthy();
  });

  it('mounts the modes editor', async () => {
    vi.spyOn(copilotService, 'getStatus').mockResolvedValue(STATUS);
    vi.spyOn(modesService, 'list').mockResolvedValue({ modes: [], activeModeId: 'general' });
    render(<BetaSettings />);
    expect(await screen.findByLabelText('Meeting modes')).toBeTruthy();
  });

  it('keeps the beta warning above both cards', async () => {
    vi.spyOn(copilotService, 'getStatus').mockResolvedValue(STATUS);
    vi.spyOn(modesService, 'list').mockResolvedValue({ modes: [], activeModeId: 'general' });
    render(<BetaSettings />);
    const warning = screen.getByText('Beta Features');
    const copilot = await screen.findByRole('switch', { name: 'Enable the live copilot panel' });
    expect(warning.compareDocumentPosition(copilot) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });
});
