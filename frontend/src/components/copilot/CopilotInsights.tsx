'use client';

/**
 * The copilot's insight region (BACKLOG **I3b**, ADR-0038/0040).
 *
 * Purely presentational: every decision — which actions a mode allows, whether
 * a claim is grounded, whether this workspace lets an insight leave the device
 * — was already made in Rust. This file renders what it is handed and invents
 * nothing, which is why each state below is a unit test rather than a comment.
 *
 * ## The two EU AI Act obligations, and why they are different components
 *
 * - **Art. 50(1) — disclosure.** The person must be told they are interacting
 *   with an AI system. [`AiDisclosure`] says so on first open and **can** be
 *   dismissed: it is an announcement, and an announcement that cannot be
 *   acknowledged is just clutter.
 * - **Art. 50(2) — marking.** AI-generated content must be marked as such.
 *   [`AiMarking`] has **no close control and no state that hides it**, and it
 *   renders in *every* phase of this region — including while loading and after
 *   a refusal. That is deliberate: a marking that disappears in some states is
 *   not a marking, and ADR-0032 proved the equivalent property for the summary
 *   banner by mutation (removing it from a single branch must turn exactly that
 *   case red).
 *
 * ## Refusals are rendered as themselves
 *
 * There is no "empty answer" rendering here, because an empty card reads as
 * "there is nothing in this conversation" — a claim the copilot has no basis
 * for. `noContext`, `refused` and each failure `kind` get their own sentence.
 */

import { AlertTriangle, Loader2, Pin, PinOff, Quote, ShieldOff, Sparkles, SquarePen, X } from 'lucide-react';
import { useCallback, useEffect, useState } from 'react';
import type {
  GroundedClaim,
  InsightFailure,
  LiveAction,
  LiveInsightOutcome,
} from '@/types/copilot';

/** Where the region is in its request cycle. */
export type InsightState =
  | { phase: 'idle' }
  | { phase: 'loading'; action: LiveAction }
  | { phase: 'done'; outcome: LiveInsightOutcome }
  | { phase: 'failed'; action: LiveAction; failure: InsightFailure };

/** Label for each action. Kept identical to Rust's `LiveAction::label()`. */
export const ACTION_LABELS: Record<LiveAction, string> = {
  suggest: 'Suggest',
  followUpQuestions: 'Follow-up questions',
  recap: 'Recap',
  define: 'Define',
};

/** The order the buttons appear in — the mode decides which of them exist. */
export const ACTION_ORDER: LiveAction[] = ['suggest', 'followUpQuestions', 'recap', 'define'];

/** `localStorage` key for the Art. 50(1) acknowledgement. */
export const DISCLOSURE_KEY = 'mityu.copilot.aiDisclosureAcknowledged';

/**
 * Art. 50(1): tell the person they are interacting with an AI system.
 *
 * Dismissable, and remembered per machine. `localStorage` can throw or come
 * back empty (a private window, cleared site data), so every access is guarded
 * and the failure mode is **showing the disclosure again** rather than
 * suppressing it — the safe direction for a transparency obligation.
 */
export function AiDisclosure() {
  const [acknowledged, setAcknowledged] = useState(true);

  // Read after mount so server and first client render agree.
  useEffect(() => {
    let seen = false;
    try {
      seen = window.localStorage.getItem(DISCLOSURE_KEY) === 'true';
    } catch {
      seen = false;
    }
    setAcknowledged(seen);
  }, []);

  const acknowledge = useCallback(() => {
    setAcknowledged(true);
    try {
      window.localStorage.setItem(DISCLOSURE_KEY, 'true');
    } catch {
      // Not being able to remember is not a reason to fail; it only means the
      // disclosure appears again next time.
    }
  }, []);

  if (acknowledged) return null;

  return (
    <div
      role="region"
      aria-label="AI disclosure"
      className="flex items-start gap-2 border-b border-border bg-muted/50 px-3 py-2"
    >
      <Sparkles className="mt-0.5 h-3 w-3 shrink-0 text-muted-foreground" aria-hidden />
      <p className="flex-1 text-caption leading-snug text-muted-foreground">
        You are interacting with an AI assistant. Everything it offers is a draft built from this
        conversation&apos;s transcript — check it before you rely on it.
      </p>
      <button
        type="button"
        onClick={acknowledge}
        aria-label="Acknowledge the AI disclosure"
        className="rounded p-0.5 text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
      >
        <X className="h-3 w-3" />
      </button>
    </div>
  );
}

/**
 * Art. 50(2): mark AI-generated content as AI-generated.
 *
 * **There is no way to hide this.** No close control, no collapsed state, no
 * prop that suppresses it — and it is rendered unconditionally by
 * [`CopilotInsights`], so it is present in every phase including loading and
 * refusal. Tests query it by role and accessible name; the wording, colour and
 * icon are deliberately not pinned, so it can be reworded without a test
 * pretending the marking was removed.
 */
export function AiMarking() {
  return (
    <p
      role="note"
      aria-label="AI-generated content notice"
      className="flex items-center gap-1 px-3 pb-1 pt-2 text-caption leading-snug text-muted-foreground"
    >
      <Sparkles className="h-3 w-3 shrink-0" aria-hidden />
      AI-generated · verify before you rely on it
    </p>
  );
}

/** The sentence for a failure, and whether it is the user&apos;s to act on. */
function failureCopy(failure: InsightFailure): { text: string; hint?: string } {
  switch (failure.kind) {
    case 'cloudNotAllowed':
      return {
        text: failure.message,
        hint: 'Switch to the built-in local model, or allow cloud insights in Settings → Copilot.',
      };
    case 'timeout':
      return { text: failure.message, hint: 'Ask again, or try a smaller window.' };
    case 'unparsable':
      return {
        text: 'The model did not answer in a form the copilot could check.',
        hint: 'Nothing was shown rather than showing something unverified.',
      };
    case 'noUsableSource':
      return {
        text: failure.message,
        hint: 'This mode reads sources that a later version adds.',
      };
    default:
      return { text: failure.message };
  }
}

function Refusal({ children }: { children: React.ReactNode }) {
  return (
    <p className="px-3 py-2 text-caption leading-snug text-muted-foreground">{children}</p>
  );
}

/**
 * The id a pin is keyed by (BACKLOG I3c).
 *
 * Derived from the claim's own content and citation rather than from an array
 * index: the same claim must get the same id across a re-render and a re-poll,
 * or the backend's pinned set and the panel's buttons would disagree about
 * which card is pinned. It is also what makes pinning idempotent — a
 * double-click stores one block, not two.
 */
export function claimPinId(claim: GroundedClaim): string {
  return `${claim.sourceChunkId}::${claim.text}`;
}

function Claims({
  outcome,
  pinnedIds,
  onPin,
  onUnpin,
}: {
  outcome: Extract<LiveInsightOutcome, { status: 'answered' }>;
  pinnedIds: string[];
  onPin: (claim: GroundedClaim, action: LiveAction) => void;
  onUnpin: (id: string) => void;
}) {
  return (
    <>
      <ol className="space-y-1.5 px-3 py-1">
        {outcome.claims.map((claim) => {
          const pinId = claimPinId(claim);
          const pinned = pinnedIds.includes(pinId);
          return (
          <li
            key={pinId}
            className="rounded-md border border-border px-2 py-1.5 text-[12px] leading-snug text-foreground"
          >
            {claim.text}
            <span className="mt-1 flex items-center gap-1 text-caption text-muted-foreground">
              <Quote className="h-2.5 w-2.5 shrink-0" aria-hidden />
              {/* The stamp comes from the cited segment, never from the model. */}
              said at {claim.timestamp}
              <button
                type="button"
                onClick={() => (pinned ? onUnpin(pinId) : onPin(claim, outcome.action))}
                aria-pressed={pinned}
                aria-label={pinned ? `Unpin: ${claim.text}` : `Pin to notes: ${claim.text}`}
                className="ml-auto inline-flex items-center gap-1 rounded border border-border px-1.5 py-0.5 text-caption transition-colors hover:bg-muted"
              >
                {pinned ? (
                  <>
                    <PinOff className="h-2.5 w-2.5 shrink-0" aria-hidden /> Pinned
                  </>
                ) : (
                  <>
                    <Pin className="h-2.5 w-2.5 shrink-0" aria-hidden /> Pin to notes
                  </>
                )}
              </button>
            </span>
          </li>
          );
        })}
      </ol>
      {/* Filtering is stated, not hidden: a shortened answer looks complete. */}
      {outcome.dropped.length > 0 && (
        <p className="px-3 pb-1 text-caption text-muted-foreground">
          {outcome.dropped.length} {outcome.dropped.length === 1 ? 'suggestion' : 'suggestions'} was
          dropped for citing something that was not said in this window.
        </p>
      )}
      {outcome.turnsOmitted > 0 && (
        <p className="px-3 pb-1 text-caption text-muted-foreground">
          Built from the most recent {outcome.turnsConsidered} segments; {outcome.turnsOmitted}{' '}
          older ones in the window were left out.
        </p>
      )}
    </>
  );
}

export interface CopilotInsightsProps {
  state: InsightState;
  /** The actions the active mode allows. Empty renders no buttons at all. */
  actions: LiveAction[];
  /**
   * The active mode's name, shown as a chip (I4b).
   *
   * Worth the pixels because the mode is the largest single influence on what
   * comes back — the same conversation answered as a "Client call" and as a
   * "Recruiting interview" gets different actions and a different voice. A user
   * who cannot see which is active cannot explain the answer they got.
   */
  modeName?: string;
  /** No recording means no window to answer from. */
  disabled?: boolean;
  onRequest: (action: LiveAction) => void;
  onCancel: () => void;
  /**
   * The claim ids the BACKEND says are pinned (I3c), and how many pins wait.
   *
   * Both come from status rather than from local state, so reopening the panel
   * shows what is actually kept instead of what this component remembers
   * clicking.
   */
  pinnedIds?: string[];
  pendingPins?: number;
  onPin?: (claim: GroundedClaim, action: LiveAction) => void;
  onUnpin?: (id: string) => void;
  /** A pin the backend refused, rendered as its own sentence. */
  pinError?: string | null;
}

export function CopilotInsights({
  state,
  actions,
  modeName,
  disabled = false,
  onRequest,
  onCancel,
  pinnedIds = [],
  pendingPins = 0,
  onPin,
  onUnpin,
  pinError = null,
}: CopilotInsightsProps) {
  const busy = state.phase === 'loading';
  const ordered = ACTION_ORDER.filter((a) => actions.includes(a));

  return (
    <section aria-label="Copilot insights" className="shrink-0 border-t border-border">
      <AiDisclosure />

      {modeName && (
        <p className="flex items-center gap-1 px-3 pt-2 text-caption text-muted-foreground">
          <SquarePen className="h-2.5 w-2.5 shrink-0" aria-hidden />
          Answering as <span className="font-medium text-foreground">{modeName}</span>
        </p>
      )}

      <div className="flex flex-wrap gap-1 px-3 pb-1 pt-2">
        {ordered.map((action) => (
          <button
            key={action}
            type="button"
            onClick={() => onRequest(action)}
            disabled={disabled || busy}
            className="inline-flex items-center gap-1 rounded-md border border-border px-2 py-1 text-caption transition-colors hover:bg-muted disabled:cursor-not-allowed disabled:opacity-50"
          >
            {ACTION_LABELS[action]}
          </button>
        ))}
      </div>

      {/* Rendered in EVERY phase — see the note on `AiMarking`. */}
      <AiMarking />

      {/* A pin is NOT saved yet, and the panel says so rather than letting the
          word "Pinned" imply it is. The meeting does not exist in the database
          until the user saves it, which is the whole reason I3c was deferred
          out of I3b (ADR-0041); telling the user their note is safe before
          that would be the same false promise in the UI. */}
      {pendingPins > 0 && (
        <p className="flex items-center gap-1 px-3 pb-1 text-caption text-muted-foreground">
          <Pin className="h-2.5 w-2.5 shrink-0" aria-hidden />
          {pendingPins} {pendingPins === 1 ? 'note' : 'notes'} pinned. They are written into the
          meeting&apos;s summary when you save it, each as a draft you review.
        </p>
      )}
      {pinError && (
        <p className="flex items-start gap-1 px-3 pb-1 text-caption text-muted-foreground">
          <AlertTriangle className="mt-0.5 h-2.5 w-2.5 shrink-0" aria-hidden />
          {pinError}
        </p>
      )}

      <div aria-live="polite" className="max-h-48 overflow-y-auto">
        {state.phase === 'idle' && (
          <Refusal>
            {disabled
              ? 'The copilot answers from a recording you start in Mityu.'
              : 'Ask for one of the above and the answer appears here, each point tied to something that was said.'}
          </Refusal>
        )}

        {state.phase === 'loading' && (
          <div className="flex items-center gap-2 px-3 py-2">
            <Loader2 className="h-3 w-3 shrink-0 animate-spin text-muted-foreground" aria-hidden />
            <span className="flex-1 text-caption text-muted-foreground">
              Reading the last few minutes for “{ACTION_LABELS[state.action]}”…
            </span>
            <button
              type="button"
              onClick={onCancel}
              className="rounded-md border border-border px-2 py-0.5 text-caption transition-colors hover:bg-muted"
            >
              Cancel
            </button>
          </div>
        )}

        {state.phase === 'done' && state.outcome.status === 'answered' && (
          <Claims
            outcome={state.outcome}
            pinnedIds={pinnedIds}
            onPin={onPin ?? (() => {})}
            onUnpin={onUnpin ?? (() => {})}
          />
        )}

        {state.phase === 'done' && state.outcome.status === 'noContext' && (
          <Refusal>
            There is nothing recent enough to answer from yet. The copilot will not answer from
            anything but this conversation.
          </Refusal>
        )}

        {state.phase === 'done' && state.outcome.status === 'refused' && (
          <Refusal>
            Everything the model offered cited something that was not said in this window, so none
            of it is shown. Nothing was repaired to make it look sourced.
          </Refusal>
        )}

        {state.phase === 'failed' && state.failure.kind !== 'cancelled' && (
          <div className="flex items-start gap-2 px-3 py-2" role="alert">
            {state.failure.kind === 'cloudNotAllowed' ? (
              <ShieldOff className="mt-0.5 h-3 w-3 shrink-0 text-muted-foreground" aria-hidden />
            ) : (
              <AlertTriangle className="mt-0.5 h-3 w-3 shrink-0 text-muted-foreground" aria-hidden />
            )}
            <div className="flex-1">
              <p className="text-caption leading-snug text-foreground">
                {failureCopy(state.failure).text}
              </p>
              {failureCopy(state.failure).hint && (
                <p className="mt-0.5 text-caption leading-snug text-muted-foreground">
                  {failureCopy(state.failure).hint}
                </p>
              )}
            </div>
          </div>
        )}

        {state.phase === 'failed' && state.failure.kind === 'cancelled' && (
          <Refusal>Cancelled.</Refusal>
        )}
      </div>
    </section>
  );
}
