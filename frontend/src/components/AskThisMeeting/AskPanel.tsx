'use client';

/**
 * Ask This Meeting (Product Intelligence slice 3).
 *
 * A question box over one meeting's own transcript. What makes it different
 * from a chat window is what it refuses to do:
 *
 *  - Every claim shows the transcript segment that supports it, and clicking it
 *    jumps there. A claim with no source cannot exist — the Rust core drops it
 *    before this component ever sees it.
 *  - "No evidence" and "refused" are rendered as real answers, not as errors or
 *    as an empty box. An empty box reads as "the meeting does not contain this",
 *    which is a stronger statement than we can support.
 *  - Filtered claims are disclosed rather than hidden, because a silently
 *    shortened answer looks complete.
 *
 * The AI-generated marking is non-dismissable here for the same reason it is in
 * the summary draft (EU AI Act Art. 50, ADR-0032): this is model output and must
 * say so wherever it appears.
 */

import { useState } from 'react';
import { Search, AlertTriangle, Loader2, CornerDownLeft } from 'lucide-react';
import { Button } from '@/components/ui/button';
import {
  askMeeting,
  dropReasonLabel,
  type AskOutcome,
  type DroppedClaim,
} from '@/services/askService';

interface AskPanelProps {
  meetingId: string;
  modelProvider: string | null;
  modelName: string | null;
  /** Jump to (and highlight) the transcript segment backing a claim. */
  onJumpToSource?: (sourceChunkId: string) => void;
}

/**
 * Always-on marking: this panel renders model output, AND it is a conversational
 * surface, which is a second and separate obligation.
 *
 * EU AI Act Art. 50(1) requires a person to be informed that they are
 * *interacting with an AI system* — not merely that some content was
 * AI-generated. ADR-0032 verified the content marking and explicitly left this
 * one open: "the planned 'Ask This Meeting' conversational surface would engage
 * it and must be assessed before it ships."
 *
 * So the first sentence names the interaction ("You are asking an AI
 * assistant"), and the rest keeps the Art. 50(2) content marking that was
 * already here. Both are stated, because they are different requirements and a
 * reader who satisfies one has not necessarily satisfied the other.
 *
 * It renders unconditionally, above the input, so it is present before a person
 * types rather than after an answer arrives — being told afterwards is not
 * being informed that you are interacting with an AI.
 */
function AiGeneratedNote() {
  return (
    <div
      role="note"
      aria-label="You are interacting with an AI assistant; AI-generated content, human review required"
      className="mb-3 flex items-start gap-2 rounded-lg border border-amber-300 bg-amber-50 p-2.5 text-sm dark:border-amber-500/30 dark:bg-amber-500/10"
    >
      <AlertTriangle
        className="mt-0.5 h-4 w-4 flex-shrink-0 text-amber-600 dark:text-amber-400"
        aria-hidden="true"
      />
      <p className="text-amber-900 dark:text-amber-200">
        <span className="font-semibold">
          You are asking an AI assistant · answers are AI-generated and need your
          review.
        </span>{' '}
        Answers are drawn only from this meeting&apos;s transcript. Open the
        source of each line to check it.
      </p>
    </div>
  );
}

function DroppedNote({ dropped }: { dropped: DroppedClaim[] }) {
  if (dropped.length === 0) return null;
  return (
    <p className="mt-3 text-xs text-muted-foreground">
      {dropped.length === 1 ? '1 line was' : `${dropped.length} lines were`} left
      out because {dropReasonLabel(dropped[0].reason)}
      {dropped.length > 1 ? ' (and similar)' : ''}.
    </p>
  );
}

export function AskPanel({
  meetingId,
  modelProvider,
  modelName,
  onJumpToSource,
}: AskPanelProps) {
  const [question, setQuestion] = useState('');
  const [outcome, setOutcome] = useState<AskOutcome | null>(null);
  const [isAsking, setIsAsking] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const canAsk =
    question.trim().length > 0 && !isAsking && !!modelProvider && !!modelName;

  const ask = async () => {
    if (!canAsk || !modelProvider || !modelName) return;
    setIsAsking(true);
    setError(null);
    setOutcome(null);
    try {
      setOutcome(
        await askMeeting({
          meetingId,
          question: question.trim(),
          modelProvider,
          modelName,
        }),
      );
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setIsAsking(false);
    }
  };

  return (
    <section className="w-full" aria-label="Ask this meeting">
      <AiGeneratedNote />

      <div className="flex items-center gap-2">
        <div className="relative flex-1">
          <Search
            className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground"
            aria-hidden="true"
          />
          <input
            type="text"
            value={question}
            onChange={(e) => setQuestion(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter') void ask();
            }}
            placeholder="Ask about this meeting…"
            aria-label="Question about this meeting"
            className="w-full rounded-lg border border-border bg-background py-2 pl-9 pr-3 text-sm focus:outline-none focus:ring-2 focus:ring-primary"
          />
        </div>
        <Button onClick={() => void ask()} disabled={!canAsk}>
          {isAsking ? (
            <Loader2 className="h-4 w-4 animate-spin" aria-hidden="true" />
          ) : (
            <CornerDownLeft className="h-4 w-4" aria-hidden="true" />
          )}
          <span className="ml-2">Ask</span>
        </Button>
      </div>

      {!modelProvider || !modelName ? (
        <p className="mt-3 text-sm text-muted-foreground">
          Choose a model in Settings to ask questions about this meeting.
        </p>
      ) : null}

      {error ? (
        <p className="mt-3 text-sm text-red-600 dark:text-red-400" role="alert">
          Couldn&apos;t answer: {error}
        </p>
      ) : null}

      {outcome?.status === 'noEvidence' ? (
        <p className="mt-4 text-sm text-muted-foreground">
          Nothing in this meeting&apos;s transcript answers that. Rather than
          guess from outside the meeting, Mityu leaves it unanswered.
        </p>
      ) : null}

      {outcome?.status === 'refused' ? (
        <div className="mt-4">
          <p className="text-sm text-muted-foreground">
            An answer was drafted but none of it could be traced to this
            meeting, so it was discarded rather than shown.
          </p>
          <DroppedNote dropped={outcome.dropped} />
        </div>
      ) : null}

      {outcome?.status === 'answered' ? (
        <div className="mt-4">
          <ul className="space-y-2">
            {outcome.claims.map((claim, i) => (
              <li
                key={`${claim.sourceChunkId}-${i}`}
                className="rounded-lg border border-border bg-card p-3"
              >
                <p className="text-sm text-foreground">{claim.text}</p>
                <button
                  type="button"
                  onClick={() => onJumpToSource?.(claim.sourceChunkId)}
                  disabled={!onJumpToSource}
                  className="mt-1.5 text-xs text-primary hover:underline disabled:cursor-default disabled:no-underline disabled:opacity-60"
                  aria-label={`Open the transcript at ${claim.timestamp}`}
                >
                  Source · {claim.timestamp}
                </button>
              </li>
            ))}
          </ul>
          <DroppedNote dropped={outcome.dropped} />
        </div>
      ) : null}
    </section>
  );
}

export default AskPanel;
