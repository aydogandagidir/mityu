'use client';

/**
 * One AI-extracted action item, reviewable where it is shown — DESIGN_SYSTEM.md §5.15.
 *
 * WHY THIS COMPONENT EXISTS. `api_get_open_action_items` returns drafts: items a model
 * wrote and no person has approved. Home rendered them as a title with a 2px coloured
 * dot whose only explanation was a `title=` tooltip — no Art. 50 marking, no link to the
 * transcript segment the claim came from, and no way to approve or reject without
 * opening the meeting. The `source_chunk_id` was already on the wire and unused. That is
 * the single highest-severity finding in the audit, because it is the one place where
 * unreviewed model output was presented as a plain fact.
 *
 * 🔒 FROZEN, and asserted by the report's own suite for the twin controls inside
 * `DraftSummaryView`: the accessible names `Approve action item`, `Edit action item`,
 * `Reject action item` and `Reason for rejecting this action item (optional)`.
 *
 * OPTIMISTIC, WITH A REVERT. Every `summaryDraftService` mutation resolves a boolean and
 * `false` is a legitimate soft no-op (unknown id, illegal transition), not an exception.
 * So the row applies the new status immediately, and puts it back — with a toast — when
 * the call resolves false or throws. A queue that silently keeps a status the backend
 * refused is worse than one that is slow.
 */

import React, { useCallback, useId, useState } from 'react';
import { Check, Pencil, X } from 'lucide-react';
import { toast } from 'sonner';
import { AiLabel, SourceChip } from '@/components/report/primitives';
import { StatusPill, type ReviewStatus } from '@/components/report/StatusPill';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { cn } from '@/lib/utils';

export interface ActionItemRowItem {
  id: string;
  text: string;
  status: ReviewStatus;
  meeting_id: string;
  meeting_title: string;
  assignee: string | null;
  due: string | null;
  source_chunk_id: string;
}

export interface ActionItemRowProps {
  item: ActionItemRowItem;
  /** Opens the meeting at this item's source segment. */
  onJumpToSource: (item: ActionItemRowItem) => void;
  onApprove: (itemId: string) => Promise<boolean>;
  onReject: (itemId: string, reason?: string) => Promise<boolean>;
  onEdit: (itemId: string, text: string) => Promise<boolean>;
  /** Reports the status the row settled on, so a list can keep its own copy in step. */
  onStatusSettled?: (itemId: string, status: ReviewStatus) => void;
  className?: string;
}

type Mode = 'idle' | 'editing' | 'rejecting';

export function ActionItemRow({
  item,
  onJumpToSource,
  onApprove,
  onReject,
  onEdit,
  onStatusSettled,
  className,
}: ActionItemRowProps) {
  const [status, setStatus] = useState<ReviewStatus>(item.status);
  const [text, setText] = useState(item.text);
  const [mode, setMode] = useState<Mode>('idle');
  const [draftText, setDraftText] = useState(item.text);
  const [reason, setReason] = useState('');
  const [pending, setPending] = useState(false);
  const reasonId = useId();

  const settle = useCallback(
    (next: ReviewStatus) => {
      setStatus(next);
      onStatusSettled?.(item.id, next);
    },
    [item.id, onStatusSettled]
  );

  const run = useCallback(
    async (optimistic: ReviewStatus, call: () => Promise<boolean>, failure: string) => {
      const previous = status;
      setPending(true);
      settle(optimistic);
      try {
        const ok = await call();
        if (!ok) {
          settle(previous);
          toast.error(failure, {
            description: 'The item may have changed since this list was loaded.',
          });
        }
        return ok;
      } catch (error) {
        settle(previous);
        toast.error(failure, {
          description: error instanceof Error ? error.message : String(error),
        });
        return false;
      } finally {
        setPending(false);
      }
    },
    [settle, status]
  );

  const approve = () =>
    void run('approved', () => onApprove(item.id), 'Could not approve this action item');

  const confirmReject = async () => {
    const ok = await run(
      'rejected',
      () => onReject(item.id, reason.trim() || undefined),
      'Could not reject this action item'
    );
    if (ok) {
      setMode('idle');
      setReason('');
    }
  };

  const saveEdit = async () => {
    const next = draftText.trim();
    if (!next || next === text) {
      setMode('idle');
      setDraftText(text);
      return;
    }
    const previousText = text;
    setPending(true);
    setText(next);
    try {
      const ok = await onEdit(item.id, next);
      if (!ok) {
        setText(previousText);
        toast.error('Could not save this edit');
      } else {
        settle('edited');
        setMode('idle');
      }
    } catch (error) {
      setText(previousText);
      toast.error('Could not save this edit', {
        description: error instanceof Error ? error.message : String(error),
      });
    } finally {
      setPending(false);
    }
  };

  const reviewed = status === 'approved' || status === 'rejected';

  return (
    <li
      className={cn(
        'flex flex-col gap-2 px-4 py-3',
        status === 'rejected' && 'opacity-70',
        className
      )}
    >
      <div className="flex items-start gap-3">
        <div className="min-w-0 flex-1 space-y-1.5">
          {mode === 'editing' ? (
            <Input
              value={draftText}
              onChange={(e) => setDraftText(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter') {
                  e.preventDefault();
                  void saveEdit();
                } else if (e.key === 'Escape') {
                  e.preventDefault();
                  setMode('idle');
                  setDraftText(text);
                }
              }}
              aria-label="Action item text"
              autoFocus
            />
          ) : (
            <p
              className={cn(
                'text-body text-foreground',
                status === 'rejected' && 'line-through'
              )}
            >
              {text}
            </p>
          )}

          <div className="flex flex-wrap items-center gap-x-2 gap-y-1">
            <StatusPill status={status} />
            <SourceChip
              onClick={() => onJumpToSource(item)}
              title={`Open ${item.meeting_title} at this item's source`}
            />
            {/* The Art. 50 marking travels with the item. It is not a property of the
                screen that happens to be showing it. */}
            <AiLabel />
          </div>

          <p className="truncate text-caption text-subtle-foreground">
            {item.meeting_title}
            {item.assignee ? ` · ${item.assignee}` : ''}
            {item.due ? ` · due ${item.due}` : ''}
          </p>
        </div>

        <div className="flex shrink-0 items-center gap-1">
          {mode === 'editing' ? (
            <>
              <Button
                variant="outline"
                size="icon"
                onClick={() => void saveEdit()}
                disabled={pending}
                aria-label="Save edit"
              >
                <Check className="size-4" aria-hidden="true" />
              </Button>
              <Button
                variant="outline"
                size="icon"
                onClick={() => {
                  setMode('idle');
                  setDraftText(text);
                }}
                disabled={pending}
                aria-label="Cancel edit"
              >
                <X className="size-4" aria-hidden="true" />
              </Button>
            </>
          ) : mode === 'rejecting' ? (
            <>
              <Button
                variant="destructive"
                size="icon"
                onClick={() => void confirmReject()}
                disabled={pending}
                aria-label="Confirm reject"
              >
                <Check className="size-4" aria-hidden="true" />
              </Button>
              <Button
                variant="outline"
                size="icon"
                onClick={() => {
                  setMode('idle');
                  setReason('');
                }}
                disabled={pending}
                aria-label="Cancel reject"
              >
                <X className="size-4" aria-hidden="true" />
              </Button>
            </>
          ) : (
            <>
              {/* Approve keeps its label: it is the decision this screen exists for. */}
              <Button
                variant="outline"
                size="row"
                onClick={approve}
                disabled={pending || status === 'approved'}
                aria-label="Approve action item"
              >
                <Check className="size-4" aria-hidden="true" />
                Approve
              </Button>
              <Button
                variant="ghost"
                size="icon"
                onClick={() => {
                  setDraftText(text);
                  setMode('editing');
                }}
                disabled={pending || status === 'rejected'}
                aria-label="Edit action item"
              >
                <Pencil className="size-4" aria-hidden="true" />
              </Button>
              <Button
                variant="ghost"
                size="icon"
                onClick={() => setMode('rejecting')}
                disabled={pending || status === 'rejected'}
                aria-label="Reject action item"
              >
                <X className="size-4" aria-hidden="true" />
              </Button>
            </>
          )}
        </div>
      </div>

      {mode === 'rejecting' && (
        <div className="space-y-1">
          <Input
            id={reasonId}
            value={reason}
            onChange={(e) => setReason(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter') {
                e.preventDefault();
                void confirmReject();
              } else if (e.key === 'Escape') {
                e.preventDefault();
                setMode('idle');
                setReason('');
              }
            }}
            placeholder="Why is this wrong? (optional — press Enter to reject)"
            aria-label="Reason for rejecting this action item (optional)"
            autoFocus
          />
          {/* 🔒 The reason is never a gate: "this was wrong" is not a signal, but an
              empty box must not stop a person rejecting something that is wrong. */}
          <p className="text-caption text-subtle-foreground">
            A reason is optional and is kept on this device.
          </p>
        </div>
      )}

      {reviewed && mode === 'idle' && (
        <p className="sr-only" role="status">
          {status === 'approved' ? 'Action item approved' : 'Action item rejected'}
        </p>
      )}
    </li>
  );
}

export default ActionItemRow;
