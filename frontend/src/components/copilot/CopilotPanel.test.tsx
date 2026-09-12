// @vitest-environment jsdom

/**
 * What the copilot panel must never do, as tests rather than as comments
 * (BACKLOG I1, ADR-0038).
 *
 * Two of these assert an *absence*, which is the kind of guarantee that rots
 * quietly: nothing fails when a "Start recording" button is added to a panel
 * that is supposed to follow a session the user already consented to, and
 * nothing fails when a screen-sharing chip starts promising invisibility. Both
 * were confirmed load-bearing by mutation before being committed.
 */

import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import { CopilotPanel, appendLine, sideForSource, TAIL_LENGTH, PanelLine } from './CopilotPanel';
import type { CopilotStatus } from '@/types/copilot';
import type { TranscriptUpdate } from '@/types';

// The suite default is `environment: 'node'`; this file opts into jsdom above.
// Cleanup is explicit because `globals` is off, so RTL's automatic afterEach
// hook is never installed — without this, each render stacks on the last and
// every `getByText` finds two of everything.
afterEach(cleanup);

const BASE_STATUS: CopilotStatus = {
  config: {
    enabled: true,
    contentProtection: true,
    shortcutsEnabled: true,
    keybinds: {
      togglePanel: 'CommandOrControl+Shift+M',
      ask: 'CommandOrControl+Shift+A',
      captureScreen: 'CommandOrControl+Shift+S',
    },
  },
  protection: {
    level: 'bestEffort',
    headline: 'Best effort on macOS',
    detail: 'macOS 15 and newer ignore this for ScreenCaptureKit-based sharing.',
  },
  panelOpen: true,
  recording: true,
  shortcuts: [],
};

function update(overrides: Partial<TranscriptUpdate>): TranscriptUpdate {
  return {
    text: 'hello',
    timestamp: '14:00:00',
    source: 'system',
    sequence_id: 1,
    chunk_start_time: 0,
    is_partial: false,
    confidence: 0.9,
    audio_start_time: 0,
    audio_end_time: 1,
    duration: 1,
    ...overrides,
  };
}

describe('transcript tail', () => {
  it('attributes a line by capture device, never by voice', () => {
    // The microphone is the user; system audio is everyone else. Nothing here
    // infers who is speaking from how they sound (ADR-0034).
    expect(sideForSource('microphone')).toBe('me');
    expect(sideForSource('Microphone (Realtek)')).toBe('me');
    expect(sideForSource('system')).toBe('them');
    expect(sideForSource('')).toBe('them');
  });

  it('ignores partial segments', () => {
    // A partial is text about to be rewritten; letting it in makes lines change
    // under the reader.
    expect(appendLine([], update({ is_partial: true }))).toEqual([]);
  });

  it('ignores empty text', () => {
    expect(appendLine([], update({ text: '   ' }))).toEqual([]);
  });

  it('replaces a segment when it arrives again rather than duplicating it', () => {
    const first = appendLine([], update({ sequence_id: 7, text: 'draft' }));
    const second = appendLine(first, update({ sequence_id: 7, text: 'final' }));
    expect(second).toHaveLength(1);
    expect(second[0].text).toBe('final');
  });

  it('keeps the tail bounded', () => {
    let lines: PanelLine[] = [];
    for (let i = 0; i < TAIL_LENGTH + 5; i += 1) {
      lines = appendLine(lines, update({ sequence_id: i, text: `line ${i}` }));
    }
    expect(lines).toHaveLength(TAIL_LENGTH);
    expect(lines[lines.length - 1].text).toBe(`line ${TAIL_LENGTH + 4}`);
  });
});

describe('the panel', () => {
  it('offers no way to start a recording', () => {
    // ADR-0038 invariant 2: the copilot follows a session whose consent was
    // already taken in Rust. It must never be a second way to begin capture.
    render(<CopilotPanel status={{ ...BASE_STATUS, recording: false }} lines={[]} />);
    const startish = screen
      .queryAllByRole('button')
      .filter((button) => /start|record/i.test(button.textContent ?? ''));
    expect(startish).toEqual([]);
  });

  it('says the copilot does not record on its own when nothing is running', () => {
    render(<CopilotPanel status={{ ...BASE_STATUS, recording: false }} lines={[]} />);
    expect(screen.getByText(/never records on its own/i)).toBeTruthy();
  });

  it('shows the platform verdict verbatim and never claims invisibility', () => {
    render(<CopilotPanel status={BASE_STATUS} lines={[]} />);
    expect(screen.getByText('Best effort on macOS')).toBeTruthy();
    expect(document.body.textContent ?? '').not.toMatch(/undetectable|invisible/i);
  });

  it('labels who was speaking', () => {
    const lines: PanelLine[] = [
      { id: 1, side: 'them', text: 'their line', timestamp: '14:00:00' },
      { id: 2, side: 'me', text: 'my line', timestamp: '14:00:05' },
    ];
    render(<CopilotPanel status={BASE_STATUS} lines={lines} />);
    expect(screen.getByText('Them')).toBeTruthy();
    expect(screen.getByText('You')).toBeTruthy();
  });
});
