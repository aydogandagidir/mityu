/**
 * @vitest-environment jsdom
 *
 * What "Ask This Meeting" must never do.
 *
 * The Rust core guarantees a claim cannot exist without a resolvable source
 * (`ask::grounding`). These tests pin the other half — that the UI does not
 * undo that guarantee by rendering a refusal as an empty box, hiding filtered
 * claims, or dropping the AI marking.
 *
 * The distinction between "no evidence" and "an error" is load-bearing: a
 * transcript that does not cover a question is an answer, and showing it as a
 * failure would push a user toward asking a general-purpose model instead.
 */

import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { AskPanel } from './AskPanel';
import type { AskOutcome } from '@/services/askService';

const askMeeting = vi.fn<() => Promise<AskOutcome>>();

vi.mock('@/services/askService', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@/services/askService')>();
  return { ...actual, askMeeting: (...args: unknown[]) => askMeeting(...(args as [])) };
});

const MARKING = { role: 'note', name: /AI-generated content, human review required/i };

function renderPanel() {
  return render(
    <AskPanel meetingId="m1" modelProvider="ollama" modelName="qwen" onJumpToSource={() => {}} />,
  );
}

/** Render the panel already showing an outcome, without driving the input. */
async function renderWithOutcome(outcome: AskOutcome) {
  askMeeting.mockResolvedValue(outcome);
  const utils = renderPanel();
  const input = screen.getByLabelText('Question about this meeting');
  fireEvent.change(input, { target: { value: 'what was decided?' } });
  fireEvent.keyDown(input, { key: 'Enter' });
  return utils;
}

// Block body on purpose: `beforeEach(() => askMeeting.mockReset())` returns the
// mock, and Vitest calls a returned function as the test's teardown -- which
// invokes the mock after the test and surfaces its throw as an unhandled error.
beforeEach(() => {
  askMeeting.mockReset();
});
afterEach(cleanup);

describe('Ask This Meeting', () => {
  it('always carries the AI-generated marking', () => {
    renderPanel();
    expect(screen.getByRole(MARKING.role, { name: MARKING.name })).toBeDefined();
  });

  it('renders "no evidence" as an answer, not as a failure', async () => {
    await renderWithOutcome({ status: 'noEvidence' });

    expect(await screen.findByText(/leaves it unanswered/i)).toBeDefined();
    // It must not be reported as an error.
    expect(screen.queryByRole('alert')).toBeNull();
    expect(screen.queryByText(/Couldn't answer/i)).toBeNull();
  });

  it('renders a refusal instead of an empty answer', async () => {
    await renderWithOutcome({
      status: 'refused',
      passagesConsidered: 3,
      dropped: [{ text: 'Invented.', sourceChunkId: 'ghost', reason: 'ungroundedCitation' }],
    });

    expect(await screen.findByText(/discarded rather than shown/i)).toBeDefined();
    // The unsupported claim's text must never be shown.
    expect(screen.queryByText('Invented.')).toBeNull();
  });

  /// Every rendered claim is openable at its source -- that is the whole promise.
  it('gives every claim a source control', async () => {
    await renderWithOutcome({
      status: 'answered',
      passagesConsidered: 4,
      dropped: [],
      claims: [
        { text: 'Budget approved.', sourceChunkId: 'c1', timestamp: '00:12', audioStartTime: 12 },
        { text: 'Ship on Friday.', sourceChunkId: 'c2', timestamp: '01:30', audioStartTime: 90 },
      ],
    });

    expect(await screen.findByText('Budget approved.')).toBeDefined();
    expect(screen.getByText('Ship on Friday.')).toBeDefined();
    const sources = screen.getAllByRole('button', { name: /Open the transcript at/i });
    expect(sources).toHaveLength(2);
  });

  /// A silently shortened answer looks complete, so filtering must be visible.
  it('discloses that claims were filtered out', async () => {
    await renderWithOutcome({
      status: 'answered',
      passagesConsidered: 4,
      dropped: [{ text: 'Invented.', sourceChunkId: 'ghost', reason: 'ungroundedCitation' }],
      claims: [
        { text: 'Budget approved.', sourceChunkId: 'c1', timestamp: '00:12', audioStartTime: 12 },
      ],
    });

    expect(await screen.findByText(/left out because/i)).toBeDefined();
    expect(screen.queryByText('Invented.')).toBeNull();
  });

  it('reports a real failure as an error', async () => {
    // Throw at call time rather than mockRejectedValue, which builds the
    // rejected promise during setup and surfaces as an unhandled rejection.
    askMeeting.mockImplementation(async () => {
      throw new Error('no provider configured');
    });
    renderPanel();
    const input = screen.getByLabelText('Question about this meeting');
    fireEvent.change(input, { target: { value: 'anything' } });
    fireEvent.keyDown(input, { key: 'Enter' });

    await waitFor(() => expect(screen.getByRole('alert')).toBeDefined());
    expect(screen.getByRole('alert').textContent).toContain('no provider configured');
  });

  it('cannot be asked without a model configured', () => {
    render(<AskPanel meetingId="m1" modelProvider={null} modelName={null} />);
    expect(screen.getByRole('button', { name: /Ask/i })).toHaveProperty('disabled', true);
    expect(screen.getByText(/Choose a model in Settings/i)).toBeDefined();
  });
});
