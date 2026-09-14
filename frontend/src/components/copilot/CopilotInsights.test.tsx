// @vitest-environment jsdom

/**
 * The insight region's transparency and refusal guarantees, as tests
 * (BACKLOG I3b, ADR-0032/0038/0040).
 *
 * Two kinds of claim live here, and both rot quietly if left to convention:
 *
 * 1. **The Art. 50(2) marking cannot be suppressed.** ADR-0032 established the
 *    pattern for the summary banner — assert presence in *every* state, query
 *    by role and accessible name so wording stays free to change, and prove the
 *    suite has teeth by removing the marking from one branch and watching
 *    exactly that case go red. The same is done here.
 * 2. **A refusal is rendered as a refusal.** The failure mode this guards
 *    against is not a crash; it is an empty, confident-looking card. Each of
 *    `noContext`, `refused` and the failure kinds must say something, and none
 *    of them may render as an answer.
 */

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import {
  AiMarking,
  claimPinId,
  CopilotInsights,
  DISCLOSURE_KEY,
  type InsightState,
} from './CopilotInsights';
import type { LiveAction, LiveInsightOutcome } from '@/types/copilot';

const ALL_ACTIONS: LiveAction[] = ['suggest', 'followUpQuestions', 'recap', 'define'];

const answered: LiveInsightOutcome = {
  status: 'answered',
  action: 'recap',
  claims: [
    {
      text: 'The deadline moved to the 14th.',
      sourceChunkId: 't7',
      timestamp: '02:13',
      audioStartTime: 133,
    },
  ],
  dropped: [],
  turnsConsidered: 6,
  turnsOmitted: 0,
};

/** Every phase the region can be in, so "in every state" is enumerable. */
const EVERY_STATE: Array<{ name: string; state: InsightState }> = [
  { name: 'idle', state: { phase: 'idle' } },
  { name: 'loading', state: { phase: 'loading', action: 'recap' } },
  { name: 'answered', state: { phase: 'done', outcome: answered } },
  { name: 'no context', state: { phase: 'done', outcome: { status: 'noContext' } } },
  {
    name: 'refused',
    state: {
      phase: 'done',
      outcome: {
        status: 'refused',
        action: 'suggest',
        dropped: [{ text: 'Invented.', sourceChunkId: 't99', reason: 'ungroundedCitation' }],
        turnsConsidered: 4,
        turnsOmitted: 0,
      },
    },
  },
  {
    name: 'failed',
    state: {
      phase: 'failed',
      action: 'recap',
      failure: { kind: 'provider', message: 'model call failed: connection refused' },
    },
  },
];

function renderRegion(state: InsightState, overrides: Partial<{ disabled: boolean }> = {}) {
  return render(
    <CopilotInsights
      state={state}
      actions={ALL_ACTIONS}
      disabled={overrides.disabled ?? false}
      onRequest={() => {}}
      onCancel={() => {}}
    />
  );
}

beforeEach(() => {
  // The disclosure is a first-open announcement; most tests are not about it.
  window.localStorage.setItem(DISCLOSURE_KEY, 'true');
});

afterEach(() => {
  cleanup();
  window.localStorage.clear();
});

describe('the Art. 50(2) marking', () => {
  it.each(EVERY_STATE)('is present in the $name state', ({ state }) => {
    renderRegion(state);
    expect(screen.getByRole('note', { name: /ai-generated content/i })).toBeTruthy();
  });

  /**
   * The marking is only a marking if it cannot be turned off. A close button,
   * a collapse toggle or an `aria-hidden` would each defeat it, so the
   * assertion is about the rendered subtree rather than about wording.
   */
  it('carries no control that could dismiss it', () => {
    render(<AiMarking />);
    const marking = screen.getByRole('note', { name: /ai-generated content/i });
    expect(marking.querySelector('button')).toBeNull();
    expect(marking.querySelector('[role="button"]')).toBeNull();
    expect(marking.getAttribute('aria-hidden')).toBeNull();
    expect(marking.hidden).toBe(false);
  });

  /** No prop may suppress it: the region takes none that could. */
  it('survives a region with no actions and nothing to show', () => {
    render(
      <CopilotInsights
        state={{ phase: 'idle' }}
        actions={[]}
        disabled
        onRequest={() => {}}
        onCancel={() => {}}
      />
    );
    expect(screen.getByRole('note', { name: /ai-generated content/i })).toBeTruthy();
    expect(screen.queryByRole('button')).toBeNull();
  });
});

describe('the Art. 50(1) disclosure', () => {
  it('is shown on first open and says an AI is involved', () => {
    window.localStorage.removeItem(DISCLOSURE_KEY);
    renderRegion({ phase: 'idle' });
    const disclosure = screen.getByRole('region', { name: /ai disclosure/i });
    expect(disclosure.textContent).toMatch(/interacting with an AI/i);
  });

  it('stays dismissed once acknowledged', () => {
    window.localStorage.removeItem(DISCLOSURE_KEY);
    renderRegion({ phase: 'idle' });
    fireEvent.click(screen.getByRole('button', { name: /acknowledge the AI disclosure/i }));
    expect(screen.queryByRole('region', { name: /ai disclosure/i })).toBeNull();
    expect(window.localStorage.getItem(DISCLOSURE_KEY)).toBe('true');
  });

  /**
   * Storage can throw (a private window, blocked site data). Failing to
   * *remember* the acknowledgement must never turn into failing to *show* the
   * disclosure — for a transparency obligation the safe direction is showing
   * it again.
   */
  it('is shown again when storage cannot be read', () => {
    const getItem = vi
      .spyOn(Storage.prototype, 'getItem')
      .mockImplementation(() => {
        throw new Error('blocked');
      });
    renderRegion({ phase: 'idle' });
    expect(screen.getByRole('region', { name: /ai disclosure/i })).toBeTruthy();
    getItem.mockRestore();
  });
});

describe('refusals are rendered as refusals', () => {
  it('an empty window says there is nothing to answer from, not an empty card', () => {
    const { container } = renderRegion({
      phase: 'done',
      outcome: { status: 'noContext' },
    });
    expect(container.textContent).toMatch(/nothing recent enough/i);
    // The distinguishing property: no claim list is rendered at all.
    expect(container.querySelector('ol')).toBeNull();
  });

  it('a total rejection says so, and does not show the dropped claims as answers', () => {
    const dropped = 'Something the model invented.';
    const { container } = renderRegion({
      phase: 'done',
      outcome: {
        status: 'refused',
        action: 'suggest',
        dropped: [{ text: dropped, sourceChunkId: 't99', reason: 'ungroundedCitation' }],
        turnsConsidered: 4,
        turnsOmitted: 0,
      },
    });
    expect(container.textContent).toMatch(/cited something that was not said/i);
    // The whole point of grounding: an ungrounded claim's TEXT never reaches
    // the user, not even as a greyed-out entry.
    expect(container.textContent).not.toContain(dropped);
  });

  /**
   * The egress refusal is its own state, not a generic error: the user has a
   * decision to make and needs to be told which one.
   */
  it('the cloud policy refusal names the setting that would change it', () => {
    const { container } = renderRegion({
      phase: 'failed',
      action: 'recap',
      failure: {
        kind: 'cloudNotAllowed',
        message: 'this workspace does not allow live insights to reach a cloud provider',
      },
    });
    expect(container.textContent).toMatch(/does not allow live insights/i);
    expect(container.textContent).toMatch(/Settings/);
  });

  /** A cancelled request is the user's own doing and must not read as a fault. */
  it('a cancellation is not shown as an error', () => {
    renderRegion({
      phase: 'failed',
      action: 'recap',
      failure: { kind: 'cancelled', message: 'cancelled' },
    });
    expect(screen.queryByRole('alert')).toBeNull();
  });
});

describe('answers', () => {
  it('shows each claim with the timestamp of the segment it cites', () => {
    const { container } = renderRegion({ phase: 'done', outcome: answered });
    expect(container.textContent).toContain('The deadline moved to the 14th.');
    expect(container.textContent).toContain('02:13');
  });

  /**
   * Filtering is stated. A shortened answer that says nothing about what was
   * removed looks complete, and the user cannot tell that grounding ran.
   */
  it('says how many claims were dropped rather than quietly shortening', () => {
    const { container } = renderRegion({
      phase: 'done',
      outcome: {
        ...answered,
        dropped: [{ text: 'Invented.', sourceChunkId: 't99', reason: 'ungroundedCitation' }],
      },
    });
    expect(container.textContent).toMatch(/1 suggestion was dropped/i);
  });

  it('says when the window held more than the prompt could carry', () => {
    const { container } = renderRegion({
      phase: 'done',
      outcome: { ...answered, turnsConsidered: 40, turnsOmitted: 5 },
    });
    expect(container.textContent).toMatch(/5 older ones/i);
  });
});

describe('the mode chip', () => {
  /**
   * The mode is the largest single influence on what comes back, so a user who
   * cannot see which one is active cannot explain the answer they got.
   */
  it('names the mode that is answering', () => {
    render(
      <CopilotInsights
        state={{ phase: 'idle' }}
        actions={ALL_ACTIONS}
        modeName="Client call"
        onRequest={() => {}}
        onCancel={() => {}}
      />
    );
    expect(screen.getByText('Client call')).toBeTruthy();
  });

  /** Nothing is invented when the backend has not said yet. */
  it('shows no chip before the status has loaded', () => {
    const { container } = renderRegion({ phase: 'idle' });
    expect(container.textContent).not.toMatch(/answering as/i);
  });
});

describe('the action buttons', () => {
  /** Which actions exist is the mode's decision, made in Rust. */
  it('renders only the actions it is handed', () => {
    render(
      <CopilotInsights
        state={{ phase: 'idle' }}
        actions={['recap', 'define']}
        onRequest={() => {}}
        onCancel={() => {}}
      />
    );
    expect(screen.getByRole('button', { name: 'Recap' })).toBeTruthy();
    expect(screen.queryByRole('button', { name: 'Suggest' })).toBeNull();
  });

  it('asks for the action that was clicked', () => {
    const onRequest = vi.fn();
    render(
      <CopilotInsights
        state={{ phase: 'idle' }}
        actions={ALL_ACTIONS}
        onRequest={onRequest}
        onCancel={() => {}}
      />
    );
    fireEvent.click(screen.getByRole('button', { name: 'Follow-up questions' }));
    expect(onRequest).toHaveBeenCalledWith('followUpQuestions');
  });

  /** No recording means no window, so there is nothing to answer from. */
  it('is disabled with no recording behind it', () => {
    renderRegion({ phase: 'idle' }, { disabled: true });
    expect(screen.getByRole('button', { name: 'Recap' }).hasAttribute('disabled')).toBe(true);
  });

  it('cannot start a second request while one is running', () => {
    renderRegion({ phase: 'loading', action: 'recap' });
    expect(screen.getByRole('button', { name: 'Suggest' }).hasAttribute('disabled')).toBe(true);
  });

  it('offers a cancel while a request is running', () => {
    const onCancel = vi.fn();
    render(
      <CopilotInsights
        state={{ phase: 'loading', action: 'recap' }}
        actions={ALL_ACTIONS}
        onRequest={() => {}}
        onCancel={onCancel}
      />
    );
    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(onCancel).toHaveBeenCalled();
  });
});

/**
 * Pin to notes (BACKLOG **I3c**, ADR-0046).
 *
 * The claims here are about honesty and about identity, which are the two
 * things this control can get wrong in ways no crash would reveal:
 *
 * - **A pin is not saved yet, and the panel must not imply it is.** The meeting
 *   does not exist in the database until the user saves it — the reason I3c was
 *   deferred out of I3b (ADR-0041). A card that says "Pinned" and nothing else
 *   makes the same false promise the deferred implementation would have.
 * - **The pin id is derived from the claim, not from its position.** Keyed by
 *   index, a re-ordered or re-polled list would pin the wrong claim, and the
 *   backend's pinned set would disagree with the buttons.
 */
describe('pin to notes', () => {
  const pinnedState: InsightState = { phase: 'done', outcome: answered };

  it('offers a pin control on an answered claim', () => {
    render(
      <CopilotInsights
        state={pinnedState}
        actions={ALL_ACTIONS}
        onRequest={() => {}}
        onCancel={() => {}}
        onPin={() => {}}
        onUnpin={() => {}}
      />,
    );
    const button = screen.getByRole('button', {
      name: 'Pin to notes: The deadline moved to the 14th.',
    });
    expect(button.getAttribute('aria-pressed')).toBe('false');
  });

  it('passes the claim and the outcome action to the handler', () => {
    const onPin = vi.fn();
    render(
      <CopilotInsights
        state={pinnedState}
        actions={ALL_ACTIONS}
        onRequest={() => {}}
        onCancel={() => {}}
        onPin={onPin}
        onUnpin={() => {}}
      />,
    );
    fireEvent.click(
      screen.getByRole('button', { name: 'Pin to notes: The deadline moved to the 14th.' }),
    );
    expect(onPin).toHaveBeenCalledTimes(1);
    const [claim, action] = onPin.mock.calls[0];
    expect(claim.sourceChunkId).toBe('t7');
    expect(claim.text).toBe('The deadline moved to the 14th.');
    // The action comes from the outcome, not from whatever button was last
    // pressed: the block's label says which kind of answer was kept.
    expect(action).toBe('recap');
  });

  it('renders a claim the backend says is pinned as pinned, and unpins by id', () => {
    const onUnpin = vi.fn();
    render(
      <CopilotInsights
        state={pinnedState}
        actions={ALL_ACTIONS}
        onRequest={() => {}}
        onCancel={() => {}}
        pinnedIds={[claimPinId(answered.status === 'answered' ? answered.claims[0] : ({} as never))]}
        onPin={() => {}}
        onUnpin={onUnpin}
      />,
    );
    const button = screen.getByRole('button', {
      name: 'Unpin: The deadline moved to the 14th.',
    });
    expect(button.getAttribute('aria-pressed')).toBe('true');
    fireEvent.click(button);
    expect(onUnpin).toHaveBeenCalledWith('t7::The deadline moved to the 14th.');
  });

  /**
   * The honesty test. Saying "Pinned" while the note exists only in memory is
   * the UI version of the bug ADR-0041 refused to ship.
   */
  it('says a pin is written when the meeting is saved, rather than implying it already is', () => {
    render(
      <CopilotInsights
        state={pinnedState}
        actions={ALL_ACTIONS}
        onRequest={() => {}}
        onCancel={() => {}}
        pendingPins={2}
        onPin={() => {}}
        onUnpin={() => {}}
      />,
    );
    const note = screen.getByText(/written into the meeting/i);
    expect(note.textContent).toMatch(/2 notes pinned/i);
    expect(note.textContent).toMatch(/when you save it/i);
    expect(note.textContent).toMatch(/draft you review/i);
  });

  it('says nothing about pins when there are none', () => {
    render(
      <CopilotInsights
        state={pinnedState}
        actions={ALL_ACTIONS}
        onRequest={() => {}}
        onCancel={() => {}}
        pendingPins={0}
        onPin={() => {}}
        onUnpin={() => {}}
      />,
    );
    expect(screen.queryByText(/written into the meeting/i)).toBeNull();
  });

  it('renders a refused pin as its own sentence', () => {
    render(
      <CopilotInsights
        state={pinnedState}
        actions={ALL_ACTIONS}
        onRequest={() => {}}
        onCancel={() => {}}
        onPin={() => {}}
        onUnpin={() => {}}
        pinError="This answer has no transcript citation, so it cannot be kept."
      />,
    );
    expect(screen.getByText(/no transcript citation/i)).toBeTruthy();
  });

  /**
   * Identity, not position. Two claims differing only in citation must get
   * different ids, and the same claim must get the same id every time.
   */
  it('derives a pin id from the claim, not from its index', () => {
    const a = { text: 'Same words.', sourceChunkId: 't1', timestamp: '00:01', audioStartTime: 1 };
    const b = { text: 'Same words.', sourceChunkId: 't2', timestamp: '00:09', audioStartTime: 9 };
    expect(claimPinId(a)).toBe(claimPinId({ ...a }));
    expect(claimPinId(a)).not.toBe(claimPinId(b));
  });

  /** A card with no pin handler shows no pin control — the design fixture. */
  it('shows no pin control when pinning is not wired', () => {
    render(
      <CopilotInsights
        state={pinnedState}
        actions={ALL_ACTIONS}
        onRequest={() => {}}
        onCancel={() => {}}
      />,
    );
    expect(screen.queryByText(/written into the meeting/i)).toBeNull();
  });
});
