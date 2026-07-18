export interface RecordingStoppedMetadata {
  message: string;
  folder_path?: string | null;
  meeting_name?: string | null;
  completion_token?: string | null;
}

export type RecordingCompletionPublishResult = 'accepted' | 'duplicate' | 'invalid';

/**
 * FIFO mailbox for native recording completions.
 *
 * A claim returns an immutable snapshot, so a later recording event can never
 * replace the token/folder authority already bound to an in-flight save.
 */
export class RecordingCompletionMailbox {
  private readonly pending: Array<{
    metadata: Readonly<RecordingStoppedMetadata>;
    leased: boolean;
  }> = [];
  private readonly seenTokens = new Set<string>();
  private readonly retryHandlers = new Set<() => void>();
  private reconciliationStarted = false;

  publish(metadata: RecordingStoppedMetadata): RecordingCompletionPublishResult {
    const token = metadata.completion_token;
    if (!token) {
      return 'invalid';
    }
    if (this.seenTokens.has(token)) {
      return 'duplicate';
    }
    this.seenTokens.add(token);
    this.pending.push({
      metadata: Object.freeze({ ...metadata }),
      leased: false,
    });
    return 'accepted';
  }

  hasPending(): boolean {
    return this.pending.some(entry => !entry.leased);
  }

  hasToken(completionToken: string): boolean {
    return this.pending.some(
      entry => entry.metadata.completion_token === completionToken,
    );
  }

  claim(): Readonly<RecordingStoppedMetadata> | null {
    const entry = this.pending.find(candidate => !candidate.leased);
    if (!entry) {
      return null;
    }
    entry.leased = true;
    return entry.metadata;
  }

  release(completionToken: string): boolean {
    const entry = this.pending.find(
      candidate => candidate.metadata.completion_token === completionToken,
    );
    if (!entry) {
      return false;
    }
    entry.leased = false;
    return true;
  }

  complete(completionToken: string): boolean {
    const index = this.pending.findIndex(
      candidate => candidate.metadata.completion_token === completionToken,
    );
    if (index < 0) {
      return false;
    }
    this.pending.splice(index, 1);
    return true;
  }

  registerRetryHandler(handler: () => void): () => void {
    this.retryHandlers.add(handler);
    return () => this.retryHandlers.delete(handler);
  }

  requestRetry(): boolean {
    const handler = this.retryHandlers.values().next().value as (() => void) | undefined;
    if (!handler) {
      return false;
    }
    handler();
    return true;
  }

  beginRecoveryReconciliation(): boolean {
    if (this.reconciliationStarted) {
      return false;
    }
    this.reconciliationStarted = true;
    return true;
  }

  resetRecoveryReconciliation(): void {
    this.reconciliationStarted = false;
  }
}

// The root provider and home page currently both consume useRecordingStop.
// A window-wide mailbox makes their native event listeners idempotent and
// prevents either hook instance from retaining a stale token for a later stop.
export const recordingCompletionMailbox = new RecordingCompletionMailbox();
