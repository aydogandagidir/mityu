import { describe, expect, it } from 'vitest';
import { RecordingCompletionMailbox } from './recordingCompletionMailbox';

describe('RecordingCompletionMailbox', () => {
  it('keeps recording A bound to token A when recording B arrives later', () => {
    const mailbox = new RecordingCompletionMailbox();
    mailbox.publish({
      message: 'A stopped',
      folder_path: 'recording-a',
      completion_token: 'token-a',
    });

    const recordingA = mailbox.claim();
    mailbox.publish({
      message: 'B stopped',
      folder_path: 'recording-b',
      completion_token: 'token-b',
    });

    expect(recordingA).toMatchObject({
      folder_path: 'recording-a',
      completion_token: 'token-a',
    });
    expect(mailbox.claim()).toMatchObject({
      folder_path: 'recording-b',
      completion_token: 'token-b',
    });
  });

  it('rejects tokenless completion events instead of queueing ambiguous authority', () => {
    const mailbox = new RecordingCompletionMailbox();

    expect(mailbox.publish({ message: 'ambiguous stop' })).toBe('invalid');
    expect(mailbox.hasPending()).toBe(false);
    expect(mailbox.claim()).toBeNull();
  });

  it('deduplicates the same native event observed by multiple hook instances', () => {
    const mailbox = new RecordingCompletionMailbox();
    const metadata = {
      message: 'A stopped',
      folder_path: 'recording-a',
      completion_token: 'token-a',
    };

    expect(mailbox.publish(metadata)).toBe('accepted');
    expect(mailbox.publish(metadata)).toBe('duplicate');
    expect(mailbox.claim()?.completion_token).toBe('token-a');
    expect(mailbox.complete('token-a')).toBe(true);
    expect(mailbox.claim()).toBeNull();
  });

  it('releases a failed claim so the same token can be retried', () => {
    const mailbox = new RecordingCompletionMailbox();
    mailbox.publish({ message: 'A stopped', completion_token: 'token-a' });

    expect(mailbox.claim()?.completion_token).toBe('token-a');
    expect(mailbox.hasPending()).toBe(false);
    expect(mailbox.release('token-a')).toBe(true);
    expect(mailbox.hasPending()).toBe(true);
    expect(mailbox.claim()?.completion_token).toBe('token-a');
  });

  it('distinguishes an event owned by another hook from a missed event', () => {
    const mailbox = new RecordingCompletionMailbox();
    mailbox.publish({ message: 'A stopped', completion_token: 'token-a' });

    expect(mailbox.hasToken('token-a')).toBe(true);
    expect(mailbox.claim()?.completion_token).toBe('token-a');
    expect(mailbox.hasPending()).toBe(false);
    expect(mailbox.hasToken('token-a')).toBe(true);
    expect(mailbox.hasToken('token-b')).toBe(false);
  });

  it('keeps one mounted retry handler available across hook instances', () => {
    const mailbox = new RecordingCompletionMailbox();
    const calls: string[] = [];
    const unregisterProvider = mailbox.registerRetryHandler(() => calls.push('provider'));
    const unregisterPage = mailbox.registerRetryHandler(() => calls.push('page'));

    expect(mailbox.requestRetry()).toBe(true);
    expect(calls).toEqual(['provider']);

    unregisterProvider();
    expect(mailbox.requestRetry()).toBe(true);
    expect(calls).toEqual(['provider', 'page']);

    unregisterPage();
    expect(mailbox.requestRetry()).toBe(false);
  });

  it('runs native recovery reconciliation once across hook instances', () => {
    const mailbox = new RecordingCompletionMailbox();

    expect(mailbox.beginRecoveryReconciliation()).toBe(true);
    expect(mailbox.beginRecoveryReconciliation()).toBe(false);

    mailbox.resetRecoveryReconciliation();
    expect(mailbox.beginRecoveryReconciliation()).toBe(true);
  });
});
