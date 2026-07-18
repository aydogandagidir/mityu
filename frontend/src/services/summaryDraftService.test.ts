/**
 * Unit tests for the reject-reason wire contract (ADR-0024 §3).
 *
 * Run with `pnpm test` (Vitest, `environment: 'node'` — see `vitest.config.ts`);
 * `@tauri-apps/api/core` is mocked, so this pins the PAYLOAD, not the IPC.
 *
 * Why a unit test rather than the `/design/hitl` browser surface: that surface
 * proves the interaction (does the field appear, does Enter submit) but cannot
 * see across the IPC boundary without stubbing `__TAURI_INTERNALS__`, which
 * makes `isTauri()` true and wakes every Tauri-gated provider in the layout.
 * The argument names are a contract with Tauri's own camelCase→snake_case
 * mapping, and a mocked invoke reads them exactly.
 */

import { beforeEach, describe, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { summaryDraftService } from './summaryDraftService';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

const mockedInvoke = vi.mocked(invoke);

beforeEach(() => {
  mockedInvoke.mockReset();
  mockedInvoke.mockResolvedValue(true);
});

describe('rejectBlock', () => {
  it('passes the reason through to the command', async () => {
    await summaryDraftService.rejectBlock('m1', 'b1', 'this is a chat, not a decision');

    expect(mockedInvoke).toHaveBeenCalledWith('api_reject_summary_block', {
      meetingId: 'm1',
      blockId: 'b1',
      reason: 'this is a chat, not a decision',
    });
  });

  it('sends reason: undefined when the user gave none', async () => {
    await summaryDraftService.rejectBlock('m1', 'b1');

    expect(mockedInvoke).toHaveBeenCalledWith('api_reject_summary_block', {
      meetingId: 'm1',
      blockId: 'b1',
      reason: undefined,
    });
  });

  /**
   * The UI submits the field untouched, so blank arrives here routinely. It must
   * still reach the backend — `normalize_reason` (commands.rs) is the single
   * place that decides blank means "no reason", and duplicating that judgement
   * on this side would let the two drift.
   */
  it('forwards a blank reason rather than second-guessing it', async () => {
    await summaryDraftService.rejectBlock('m1', 'b1', '   ');

    expect(mockedInvoke).toHaveBeenCalledWith('api_reject_summary_block', {
      meetingId: 'm1',
      blockId: 'b1',
      reason: '   ',
    });
  });

  it('resolves whatever the command returns, including the soft no-op', async () => {
    mockedInvoke.mockResolvedValue(false);
    await expect(summaryDraftService.rejectBlock('m1', 'b1', 'x')).resolves.toBe(false);
  });
});

describe('rejectActionItem', () => {
  it('passes the reason through to the command', async () => {
    await summaryDraftService.rejectActionItem('a1', 'not an action');

    expect(mockedInvoke).toHaveBeenCalledWith('api_reject_action_item', {
      itemId: 'a1',
      reason: 'not an action',
    });
  });

  it('sends reason: undefined when the user gave none', async () => {
    await summaryDraftService.rejectActionItem('a1');

    expect(mockedInvoke).toHaveBeenCalledWith('api_reject_action_item', {
      itemId: 'a1',
      reason: undefined,
    });
  });
});

/**
 * The reason is scoped to rejects on purpose: an edit's delta IS its rationale,
 * and an approve has nothing to explain. If a future change starts sending
 * `reason` on these, that is a decision to make deliberately, not by accident.
 */
describe('the other verdicts carry no reason', () => {
  it('approveBlock', async () => {
    await summaryDraftService.approveBlock('m1', 'b1');
    expect(mockedInvoke).toHaveBeenCalledWith('api_approve_summary_block', {
      meetingId: 'm1',
      blockId: 'b1',
    });
  });

  it('restoreBlock', async () => {
    await summaryDraftService.restoreBlock('m1', 'b1');
    expect(mockedInvoke).toHaveBeenCalledWith('api_restore_summary_block', {
      meetingId: 'm1',
      blockId: 'b1',
    });
  });
});
