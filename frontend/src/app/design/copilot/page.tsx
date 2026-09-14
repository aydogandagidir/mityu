'use client';

/**
 * `/design/copilot` — the copilot panel with fixture data (BACKLOG I1).
 *
 * The real panel (`/copilot`) renders nothing outside the Tauri shell: it waits
 * on `copilot_get_status` and on the `transcript-update` stream, neither of
 * which exists in a browser. That is exactly the trap `docs/CONVENTIONS.md`
 * describes — a screenshot of a page that "renders nothing" still produces a
 * valid PNG — so this route feeds the same component fixed props instead, which
 * is what `tools/ui/shoot.py` can actually verify.
 *
 * Four states side by side, because the differences between them are the
 * product decisions worth reviewing: what the panel says when no recording is
 * running (it never records on its own), how honestly it states the
 * screen-sharing posture on a platform that enforces it versus one that cannot,
 * and what it says when the user has switched that request off — where the
 * platform's verdict must not be shown at all, because Mityu never asked.
 */

import { CopilotPanel, PanelLine } from '@/components/copilot/CopilotPanel';
import type { CopilotStatus } from '@/types/copilot';

const SHORTCUTS: CopilotStatus['shortcuts'] = [
  {
    action: 'togglePanel',
    label: 'Show or hide the panel',
    keybind: 'CommandOrControl+Shift+M',
    registered: true,
    unavailableReason: null,
  },
  {
    action: 'ask',
    label: 'Ask the copilot',
    keybind: 'CommandOrControl+Shift+A',
    registered: false,
    unavailableReason: "Arrives with the copilot's first insights.",
  },
];

const WINDOWS_RECORDING: CopilotStatus = {
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
  },
  protection: {
    level: 'enforced',
    headline: 'Hidden from screen sharing',
    detail:
      'Windows excludes this panel from screen sharing and recording (Windows 10 build 19041 and newer). A photograph of the screen still shows it.',
  },
  panelOpen: true,
  recording: true,
  shortcuts: SHORTCUTS,
  liveContext: {
    subscribed: true,
    windowSecs: 180,
    turns: 3,
    windowTurns: 3,
    evicted: 0,
  },
};

const MACOS_RECORDING: CopilotStatus = {
  ...WINDOWS_RECORDING,
  protection: {
    level: 'bestEffort',
    headline: 'Best effort on macOS',
    detail:
      'macOS 15 and newer ignore this for ScreenCaptureKit-based sharing, which is what Zoom, Meet and Teams use, so the panel can still be captured. Apple provides no API to prevent it.',
  },
};

/** The same computer as `WINDOWS_RECORDING`, with the request switched off. */
const PROTECTION_OFF: CopilotStatus = {
  ...WINDOWS_RECORDING,
  config: { ...WINDOWS_RECORDING.config, contentProtection: false },
};

const LINUX_IDLE: CopilotStatus = {
  ...WINDOWS_RECORDING,
  protection: {
    level: 'unsupported',
    headline: 'Not hidden from screen sharing',
    detail:
      'This platform has no capture-exclusion API, so treat the panel as visible to anyone you share your screen with.',
  },
  recording: false,
};

const LINES: PanelLine[] = [
  { id: 1, text: 'We agreed the pilot starts in the Ankara site first.', timestamp: '14:02:11' },
  { id: 2, text: 'Right — and the second site follows once the audit closes.', timestamp: '14:02:19' },
  { id: 3, text: 'Can you send the retention policy before Friday?', timestamp: '14:02:27' },
];

function Frame({ title, note, children }: { title: string; note: string; children: React.ReactNode }) {
  return (
    <figure className="space-y-2">
      <figcaption>
        <div className="text-sm font-medium text-foreground">{title}</div>
        <p className="text-xs text-muted-foreground">{note}</p>
      </figcaption>
      {/* The panel sizes itself to its window; the fixed box here stands in for
          one so every state fits on a single review page. */}
      <div className="h-[420px] w-[360px] overflow-hidden rounded-xl border border-border shadow-sm">
        {children}
      </div>
    </figure>
  );
}

export default function CopilotDesignRoute() {
  return (
    <main className="min-h-screen bg-background p-8">
      <header className="mb-6 space-y-1">
        <div className="text-xs uppercase tracking-widest text-muted-foreground">BACKLOG I1</div>
        <h1 className="text-2xl font-semibold text-foreground">Live copilot panel</h1>
        <p className="max-w-3xl text-[15px] text-muted-foreground">
          The shell only: a window, a global shortcut and an honest statement of what the operating
          system will do about screen sharing. No AI, and no capture of its own — the panel follows
          a recording the user started.
        </p>
      </header>

      <div className="flex flex-wrap gap-8">
        <Frame
          title="Windows · recording"
          note="The one platform that really excludes the panel from a capture."
        >
          <CopilotPanel status={WINDOWS_RECORDING} lines={LINES} onClose={() => {}} onPause={() => {}} onOpenMainWindow={() => {}} />
        </Frame>
        <Frame
          title="macOS · recording"
          note="macOS 15+ ignores the request for ScreenCaptureKit. The chip says so rather than promising invisibility."
        >
          <CopilotPanel status={MACOS_RECORDING} lines={LINES} paused onClose={() => {}} onResume={() => {}} onOpenMainWindow={() => {}} />
        </Frame>
        <Frame
          title="Windows · hiding switched off"
          note="The same computer as the first frame. The platform could exclude the panel — but the user said no, so the chip reports what is true now, not what the OS can do."
        >
          <CopilotPanel status={PROTECTION_OFF} lines={LINES} onClose={() => {}} onPause={() => {}} onOpenMainWindow={() => {}} />
        </Frame>
        <Frame
          title="Linux · no recording"
          note="No capture-exclusion API at all, and nothing to show until the user starts a recording."
        >
          <CopilotPanel status={LINUX_IDLE} lines={[]} onClose={() => {}} onOpenMainWindow={() => {}} />
        </Frame>
      </div>
    </main>
  );
}
