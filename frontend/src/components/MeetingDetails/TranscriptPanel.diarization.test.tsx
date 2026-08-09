// @vitest-environment jsdom
/**
 * One question: when the speaker feature cannot even be asked about, does the
 * person see anything?
 *
 * Caught in review. The hook was right — it set `error` and left `state` null —
 * but the panel gated the whole block on `state`, so a database or command
 * failure made the feature silently vanish. A user cannot tell "this meeting
 * has no speakers" from "we could not find out", and the second is the one
 * where they can retry.
 *
 * `useDiarization` is tested on its own; this covers only the rendering gate,
 * which is where the defect was.
 */
import { cleanup, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...a: unknown[]) => invoke(...a) }));

// Everything below the speaker panel needs a virtualizer, an editor and a tour
// context to mount. None of that is under test here, so it is stubbed to keep
// the failure signal about the gate rather than about scaffolding.
vi.mock('@/components/VirtualizedTranscriptView', () => ({
  VirtualizedTranscriptView: () => null,
}));
vi.mock('@/components/TranscriptView', () => ({ TranscriptView: () => null }));
vi.mock('./TranscriptButtonGroup', () => ({ TranscriptButtonGroup: () => null }));

const { TranscriptPanel } = await import('./TranscriptPanel');

afterEach(() => {
  cleanup();
  invoke.mockReset();
});

const props = {
  transcripts: [],
  customPrompt: '',
  onPromptChange: () => {},
  onCopyTranscript: () => {},
  onOpenMeetingFolder: async () => {},
  isRecording: false,
  meetingId: 'm1',
};

describe('the speaker panel when things go wrong', () => {
  it('says the check failed instead of hiding the feature', async () => {
    invoke.mockRejectedValue('database is locked');
    render(<TranscriptPanel {...props} />);
    await waitFor(() =>
      expect(document.body.textContent).toContain('Speakers could not be checked')
    );
    expect(document.body.textContent).toContain('database is locked');
    // ...and a way out, since the failure is usually transient.
    expect(screen.getByRole('button', { name: /try again/i })).toBeTruthy();
  });

  it('shows the panel normally when the check succeeds', async () => {
    invoke.mockResolvedValue({ status: 'ready' });
    render(<TranscriptPanel {...props} />);
    await waitFor(() =>
      expect(document.body.textContent).toContain('have not been separated')
    );
    expect(document.body.textContent).not.toContain('could not be checked');
  });

  /**
   * Diarization is a post-hoc pass (ADR-0034); offering it mid-recording would
   * advertise something that cannot run, and nothing here may touch a recording
   * in progress (CLAUDE.md §4).
   */
  it('asks nothing at all while recording', async () => {
    render(<TranscriptPanel {...props} isRecording={true} />);
    await waitFor(() => expect(document.body.textContent).toContain('Transcript'));
    expect(invoke).not.toHaveBeenCalled();
    expect(document.body.textContent).not.toContain('Speakers');
  });
});
