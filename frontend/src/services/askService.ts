/**
 * Ask This Meeting Service (Product Intelligence slice 3)
 *
 * Typed wrapper over `api_ask_meeting`. Follows the `summaryDraftService` /
 * `services/search.ts` pattern: this is the SINGLE typed definition of the wire
 * shape and the only place this `invoke()` lives.
 *
 * ## Wire contract (matches `src-tauri/src/ask/service.rs`)
 *
 * The outcome is a tagged union on `status`, and the union is the point: there
 * is deliberately **no "empty answer"** case. An answer with no claims is a
 * refusal, because rendering nothing reads as "the meeting does not contain
 * this", which is a different and unsupported statement.
 *
 * - `noEvidence` — retrieval found no passage, so **no model was called**. The
 *   UI must not present this as a model failure; it is the honest outcome of a
 *   transcript that does not cover the question.
 * - `answered` — at least one claim survived grounding. `dropped` may still be
 *   non-empty and must be surfaced, not hidden: a silently shortened answer
 *   looks complete.
 * - `refused` — the model produced claims and none were grounded.
 *
 * Every claim carries a `sourceChunkId` and timing taken from the retrieved
 * passage, never from the model, so a claim can always be opened in the
 * transcript.
 */

import { invoke } from '@tauri-apps/api/core';

/** Why a claim the model produced was not shown. */
export type DropReason = 'ungroundedCitation' | 'blankText' | 'duplicate';

/** A claim that survived grounding. Timing comes from the retrieved passage. */
export interface GroundedClaim {
  text: string;
  sourceChunkId: string;
  timestamp: string;
  audioStartTime: number | null;
}

/** A claim that was filtered out, kept so the UI can say filtering happened. */
export interface DroppedClaim {
  text: string;
  sourceChunkId: string;
  reason: DropReason;
}

export type AskOutcome =
  | { status: 'noEvidence' }
  | {
      status: 'answered';
      claims: GroundedClaim[];
      dropped: DroppedClaim[];
      passagesConsidered: number;
    }
  | { status: 'refused'; dropped: DroppedClaim[]; passagesConsidered: number };

/**
 * Ask a question about one meeting, answered only from that meeting's own
 * transcript.
 *
 * Read-only: nothing is persisted and the answer can never become approved
 * content. A thrown error is a real failure (no provider configured, retrieval
 * error, unparsable model reply) — `noEvidence` and `refused` are normal
 * outcomes, not errors.
 */
export async function askMeeting(args: {
  meetingId: string;
  question: string;
  modelProvider: string;
  modelName: string;
}): Promise<AskOutcome> {
  return invoke<AskOutcome>('api_ask_meeting', {
    meetingId: args.meetingId,
    question: args.question,
    modelProvider: args.modelProvider,
    modelName: args.modelName,
  });
}

/** Human-readable reason text for a dropped claim. */
export function dropReasonLabel(reason: DropReason): string {
  switch (reason) {
    case 'ungroundedCitation':
      return 'cited a passage that was not in this meeting';
    case 'blankText':
      return 'was empty';
    case 'duplicate':
      return 'repeated an earlier point';
    default:
      return 'was filtered out';
  }
}
