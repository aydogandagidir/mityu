"use client";

/**
 * DraftSummaryView (BACKLOG C1.6) — the HITL review surface for source-linked
 * structured summary drafts.
 *
 * Renders a `MeetingNotesDraft` (sections -> blocks) plus its extracted action
 * items as an EDITABLE DRAFT. Every block and action item carries a status chip
 * and per-item Approve / Reject / Edit / Restore actions wired to the C1.5 HITL
 * Tauri commands (via `summaryDraftService`, never a raw invoke). Each item has
 * a source affordance that jumps to the backing transcript segment.
 *
 * Product-critical, non-negotiable affordances (CLAUDE.md §0.5, BACKLOG C5,
 * EU AI Act Art. 50):
 *  - a non-dismissable "AI-generated · review required" banner that is ALWAYS
 *    visible while a draft is shown;
 *  - an explicit "Approve summary" action gated on human review — it is disabled
 *    (with an explanatory tooltip) until every non-rejected block is approved;
 *  - a visible link from every AI-generated item back to its source transcript
 *    segment.
 *
 * Mutations are optimistic and revert on a thrown error OR a `false` return
 * (a soft "couldn't apply" — an illegal transition / not-found / stale
 * evidence), mirroring the AnalyticsConsentSwitch revert-on-failure pattern.
 */

import { useCallback, useEffect, useMemo, useState } from 'react';
import { toast } from 'sonner';
import {
  AlertTriangle,
  Check,
  X,
  Pencil,
  RotateCcw,
  Clock,
  ChevronDown,
  ChevronRight,
  Loader2,
  Download,
  FileText,
  FileType,
  FileDown,
  Sparkles,
  ListChecks,
  MessageSquareQuote,
  Hash,
  HelpCircle,
  CheckCircle2,
} from 'lucide-react';
import { SectionCard } from '@/components/report/primitives';
import { Button } from '@/components/ui/button';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from '@/components/ui/tooltip';
import {
  summaryDraftService,
  type ActionItemDraft,
  type BlockStatus,
  type DraftBlock,
  type MeetingNotesDraft,
  type SummaryDraftResponse,
} from '@/services/summaryDraftService';
import { useExportOperations } from '@/hooks/meeting-details/useExportOperations';
import { TOUR_ANCHORS } from '@/lib/tour';
import { areBlockReviewControlsLocked } from './draftSummaryReviewState';

interface DraftSummaryViewProps {
  /** The meeting whose draft is under review. */
  meetingId: string;
  /** The initial draft payload (from `api_get_summary_draft`). */
  draftResponse: SummaryDraftResponse | null;
  /** Whether the draft is still being fetched (parent load flow). */
  isLoading?: boolean;
  /** A fetch/parse error to surface (user-friendly; never a raw panic). */
  error?: string | null;
  /** Meeting title, used for the export header + filename (C4.1). */
  meetingTitle?: string;
  /** Human-readable meeting date for the export header (C4.1). */
  meetingDate?: string;
  /**
   * Jump to the transcript segment identified by `sourceChunkId` (scroll +
   * highlight). Optional: when absent, the source affordance is hidden.
   */
  onJumpToSource?: (sourceChunkId: string) => void;
  /**
   * Notified when the whole summary transitions to `approved`, so the parent
   * can refresh any dependent view.
   */
  onSummaryApproved?: () => void;
}

/**
 * Icon for a report section, chosen from its title. The draft's section titles come
 * from the model/template, so this is a best-effort match with a neutral fallback.
 */
function sectionIcon(title: string) {
  const t = title.toLowerCase();
  if (t.includes('summary') || t.includes('overview') || t.includes('özet')) return Sparkles;
  if (t.includes('action') || t.includes('next step') || t.includes('todo')) return ListChecks;
  if (t.includes('decision')) return CheckCircle2;
  if (t.includes('question')) return HelpCircle;
  if (t.includes('topic') || t.includes('chapter')) return Hash;
  return MessageSquareQuote;
}

/** Visual styling for each review status chip. */
const STATUS_CHIP: Record<BlockStatus, { label: string; className: string }> = {
  draft: { label: 'Draft', className: 'bg-muted text-muted-foreground border-border' },
  approved: { label: 'Approved', className: 'bg-green-100 dark:bg-green-500/15 text-green-800 dark:text-green-300 border-green-300 dark:border-green-500/30' },
  edited: { label: 'Edited', className: 'bg-amber-100 dark:bg-amber-500/15 text-amber-800 dark:text-amber-200 border-amber-300 dark:border-amber-500/30' },
  rejected: { label: 'Rejected', className: 'bg-red-100 dark:bg-red-500/15 text-red-700 dark:text-red-300 border-red-300 dark:border-red-500/30' },
};

function StatusChip({ status }: { status: BlockStatus }) {
  const chip = STATUS_CHIP[status];
  return (
    <span
      className={`px-2 py-0.5 text-xs font-medium rounded-full border ${chip.className}`}
    >
      {chip.label}
    </span>
  );
}

/**
 * The always-on transparency banner. It is intentionally not dismissable: there
 * is no close button and no state that can hide it while a draft is shown.
 */
function ReviewRequiredBanner() {
  return (
    <div
      role="note"
      aria-label="AI-generated content, human review required"
      className="flex items-start gap-3 p-3 mb-4 bg-amber-50 dark:bg-amber-500/10 border border-amber-300 dark:border-amber-500/30 rounded-lg"
    >
      <AlertTriangle className="h-5 w-5 text-amber-600 dark:text-amber-400 flex-shrink-0 mt-0.5" />
      <div className="text-sm text-amber-900 dark:text-amber-200">
        <p className="font-semibold">AI-generated · review required</p>
        <p className="mt-0.5 text-amber-800 dark:text-amber-200">
          This summary and its action items were generated by AI. Each item is
          linked to its source transcript segment and must be reviewed and
          approved by a person before it is finalized.
        </p>
      </div>
    </div>
  );
}

/** A small link/clock affordance that jumps to the backing transcript segment. */
function SourceLink({
  sourceChunkId,
  onJumpToSource,
}: {
  sourceChunkId: string;
  onJumpToSource?: (sourceChunkId: string) => void;
}) {
  if (!onJumpToSource) return null;
  return (
    <TooltipProvider>
      <Tooltip>
        <TooltipTrigger asChild>
          <button
            type="button"
            onClick={() => onJumpToSource(sourceChunkId)}
            className="inline-flex shrink-0 items-center gap-1 rounded-full border border-border bg-muted/60 px-2 py-0.5 text-xs text-muted-foreground transition-colors hover:border-primary/40 hover:text-foreground"
            aria-label="Jump to source transcript segment"
          >
            <Clock className="h-3 w-3" />
            <span>Source</span>
          </button>
        </TooltipTrigger>
        <TooltipContent>Jump to the source transcript segment</TooltipContent>
      </Tooltip>
    </TooltipProvider>
  );
}

/**
 * A pending-aware wrapper around a HITL mutation. Applies an optimistic update,
 * awaits the command, and reverts on a `false` return (soft no-op) or a thrown
 * error, surfacing a user-friendly note in each case.
 */
function useHitlAction() {
  const [pendingId, setPendingId] = useState<string | null>(null);

  const run = useCallback(
    async (opts: {
      /** Stable key for the row so its buttons can show a spinner. */
      key: string;
      /** Optimistic state mutation (applied before the await). */
      optimistic: () => void;
      /** Revert if the command reports failure. */
      revert: () => void;
      /** The command; resolves `false` for a soft no-op. */
      command: () => Promise<boolean>;
      /** Message shown when the command returns `false`. */
      softFailMessage: string;
    }): Promise<boolean> => {
      const { key, optimistic, revert, command, softFailMessage } = opts;
      setPendingId(key);
      optimistic();
      try {
        const ok = await command();
        if (!ok) {
          revert();
          toast.warning(softFailMessage);
        }
        return ok;
      } catch (err) {
        console.error('[DraftSummaryView] HITL action failed:', err);
        revert();
        toast.error("Couldn't apply your change", {
          description: 'Please try again in a moment.',
        });
        return false;
      } finally {
        setPendingId((current) => (current === key ? null : current));
      }
    },
    [],
  );

  return { pendingId, run };
}

// ---------------------------------------------------------------------------
// Block row
// ---------------------------------------------------------------------------

interface BlockRowProps {
  block: DraftBlock;
  isPending: boolean;
  /** Whole-review lock: approved/in-flight summaries cannot be mutated. */
  reviewLocked: boolean;
  onApprove: () => void;
  /**
   * `reason` is the user's optional rationale. It may be empty or absent — the
   * backend normalizes blank away and rejects either way; a refusal to explain
   * must never cost the user their verdict.
   */
  onReject: (reason?: string) => void;
  onRestore: () => void;
  onEdit: (content: string) => Promise<boolean>;
  onJumpToSource?: (sourceChunkId: string) => void;
  /** First rendered block: carries the product-tour anchor (step 2). */
  isTourAnchor?: boolean;
}

function blockTextClass(type: DraftBlock['type']): string {
  switch (type) {
    case 'heading1':
      return 'text-lg font-semibold text-foreground';
    case 'heading2':
      return 'text-base font-semibold text-foreground';
    default:
      return 'text-sm text-foreground';
  }
}

function BlockRow({
  block,
  isPending,
  reviewLocked,
  onApprove,
  onReject,
  onRestore,
  onEdit,
  onJumpToSource,
  isTourAnchor = false,
}: BlockRowProps) {
  const [isEditing, setIsEditing] = useState(false);
  const [draftContent, setDraftContent] = useState(block.content);
  const [isSaving, setIsSaving] = useState(false);
  const [showOriginal, setShowOriginal] = useState(false);
  const [isRejecting, setIsRejecting] = useState(false);
  const [rejectReason, setRejectReason] = useState('');

  const isRejected = block.status === 'rejected';
  const isEdited = block.status === 'edited';

  const startEdit = () => {
    setDraftContent(block.content);
    setIsEditing(true);
  };

  const cancelEdit = () => {
    setIsEditing(false);
    setDraftContent(block.content);
  };

  const startReject = () => {
    setRejectReason('');
    setIsRejecting(true);
  };

  const cancelReject = () => {
    setIsRejecting(false);
    setRejectReason('');
  };

  /**
   * Rejects with whatever is in the field, blank included. The reason is asked
   * for because "this was wrong" teaches nothing while "wrong because X" does —
   * but it is never a gate, so Enter on an empty field is a complete, valid
   * reject and costs one keystroke.
   */
  const confirmReject = () => {
    setIsRejecting(false);
    onReject(rejectReason);
    setRejectReason('');
  };

  const saveEdit = async () => {
    setIsSaving(true);
    try {
      const ok = await onEdit(draftContent);
      if (ok) setIsEditing(false);
    } finally {
      setIsSaving(false);
    }
  };

  return (
    <div
      {...(isTourAnchor ? { 'data-tour': TOUR_ANCHORS.summaryApproveBlock } : {})}
      className="group -mx-2 rounded-lg px-2 py-3 transition-colors first:pt-0 last:pb-0 hover:bg-muted/40"
    >
      <div className="flex items-start justify-between gap-3">
        <div className="flex-1 min-w-0">
          {isEditing ? (
            <textarea
              value={draftContent}
              onChange={(e) => setDraftContent(e.target.value)}
              disabled={reviewLocked}
              rows={Math.max(2, Math.ceil(draftContent.length / 60))}
              className="w-full px-3 py-2 border border-blue-300 rounded-md text-sm focus:outline-none focus:ring-1 focus:ring-blue-500 focus:border-blue-500 resize-y"
              autoFocus
            />
          ) : (
            <p
              className={`${blockTextClass(block.type)} whitespace-pre-wrap break-words ${
                isRejected ? 'line-through text-muted-foreground' : ''
              }`}
            >
              {block.type === 'bullet' ? `• ${block.content}` : block.content}
            </p>
          )}

          <div className="flex items-center flex-wrap gap-x-3 gap-y-1 mt-2">
            <StatusChip status={block.status} />
            {isEdited && block.original_content !== undefined && (
              <button
                type="button"
                onClick={() => setShowOriginal((v) => !v)}
                className="inline-flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground"
                aria-label="Show originally generated text"
              >
                {showOriginal ? (
                  <ChevronDown className="h-3.5 w-3.5" />
                ) : (
                  <ChevronRight className="h-3.5 w-3.5" />
                )}
                <span>Original</span>
              </button>
            )}
            <SourceLink
              sourceChunkId={block.source_chunk_id}
              onJumpToSource={onJumpToSource}
            />
          </div>

          {isEdited && showOriginal && block.original_content !== undefined && (
            <div className="mt-2 p-2 bg-muted border border-border rounded text-xs text-muted-foreground">
              <span className="font-medium text-muted-foreground">
                Originally generated:{' '}
              </span>
              <span className="whitespace-pre-wrap break-words">
                {block.original_content}
              </span>
            </div>
          )}

          {isRejecting && (
            <input
              type="text"
              value={rejectReason}
              onChange={(e) => setRejectReason(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter') {
                  e.preventDefault();
                  confirmReject();
                } else if (e.key === 'Escape') {
                  e.preventDefault();
                  cancelReject();
                }
              }}
              placeholder="Why is this wrong? (optional — press Enter to reject)"
              aria-label="Reason for rejecting this block (optional)"
              className="mt-2 w-full px-3 py-2 border border-red-300 dark:border-red-500/40 rounded-md text-sm focus:outline-none focus:ring-1 focus:ring-red-500 focus:border-red-500"
              autoFocus
            />
          )}
        </div>

        {/* Per-block actions */}
        <div className="flex-shrink-0 flex items-center gap-1">
          {isEditing ? (
            <>
              <Button
                variant="green"
                size="sm"
                onClick={saveEdit}
                disabled={isSaving || reviewLocked}
                title="Save edit"
              >
                {isSaving ? (
                  <Loader2 className="h-4 w-4 animate-spin" />
                ) : (
                  <Check className="h-4 w-4" />
                )}
                <span className="hidden lg:inline">Save</span>
              </Button>
              <Button
                variant="outline"
                size="sm"
                onClick={cancelEdit}
                disabled={isSaving}
                title="Cancel edit"
              >
                <X className="h-4 w-4" />
              </Button>
            </>
          ) : isRejecting ? (
            <>
              <Button
                variant="red"
                size="sm"
                onClick={confirmReject}
                disabled={isPending}
                title="Reject block"
                aria-label="Confirm reject"
              >
                {isPending ? (
                  <Loader2 className="h-4 w-4 animate-spin" />
                ) : (
                  <X className="h-4 w-4" />
                )}
                <span className="hidden lg:inline">Reject</span>
              </Button>
              <Button
                variant="ghost"
                size="sm"
                onClick={cancelReject}
                disabled={isPending}
                title="Cancel"
                aria-label="Cancel reject"
              >
                Cancel
              </Button>
            </>
          ) : isRejected ? (
            <Button
              variant="outline"
              size="sm"
              onClick={onRestore}
              disabled={isPending || reviewLocked}
              title="Restore to draft"
            >
              {isPending ? (
                <Loader2 className="h-4 w-4 animate-spin" />
              ) : (
                <RotateCcw className="h-4 w-4" />
              )}
              <span className="hidden lg:inline">Restore</span>
            </Button>
          ) : (
            <>
              <Button
                variant={block.status === 'approved' ? 'green' : 'outline'}
                size="sm"
                onClick={onApprove}
                disabled={isPending || reviewLocked || block.status === 'approved'}
                title="Approve block"
                aria-label="Approve block"
              >
                {isPending ? (
                  <Loader2 className="h-4 w-4 animate-spin" />
                ) : (
                  <Check className="h-4 w-4" />
                )}
              </Button>
              <Button
                variant="outline"
                size="sm"
                onClick={startEdit}
                disabled={isPending || reviewLocked}
                title="Edit block"
                aria-label="Edit block"
              >
                <Pencil className="h-4 w-4" />
              </Button>
              <Button
                variant="outline"
                size="sm"
                onClick={startReject}
                disabled={isPending || reviewLocked}
                title="Reject block"
                aria-label="Reject block"
              >
                <X className="h-4 w-4 text-red-600 dark:text-red-400" />
              </Button>
            </>
          )}
        </div>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Action item row
// ---------------------------------------------------------------------------

interface ActionItemRowProps {
  item: ActionItemDraft;
  isPending: boolean;
  onApprove: () => void;
  /** `reason` is optional and never gates the reject — see {@link BlockRowProps}. */
  onReject: (reason?: string) => void;
  onRestore: () => void;
  onEditText: (text: string) => Promise<boolean>;
  onJumpToSource?: (sourceChunkId: string) => void;
}

function ActionItemRow({
  item,
  isPending,
  onApprove,
  onReject,
  onRestore,
  onEditText,
  onJumpToSource,
}: ActionItemRowProps) {
  const [isEditing, setIsEditing] = useState(false);
  const [draftText, setDraftText] = useState(item.text);
  const [isSaving, setIsSaving] = useState(false);
  const [isRejecting, setIsRejecting] = useState(false);
  const [rejectReason, setRejectReason] = useState('');

  const isRejected = item.status === 'rejected';

  const startEdit = () => {
    setDraftText(item.text);
    setIsEditing(true);
  };

  const cancelEdit = () => {
    setIsEditing(false);
    setDraftText(item.text);
  };

  const startReject = () => {
    setRejectReason('');
    setIsRejecting(true);
  };

  const cancelReject = () => {
    setIsRejecting(false);
    setRejectReason('');
  };

  /** Rejects with whatever is in the field, blank included — see `BlockRow`. */
  const confirmReject = () => {
    setIsRejecting(false);
    onReject(rejectReason);
    setRejectReason('');
  };

  const saveEdit = async () => {
    setIsSaving(true);
    try {
      const ok = await onEditText(draftText);
      if (ok) setIsEditing(false);
    } finally {
      setIsSaving(false);
    }
  };

  return (
    <div className="group -mx-2 rounded-lg px-2 py-3 transition-colors first:pt-0 last:pb-0 hover:bg-muted/40">
      <div className="flex items-start justify-between gap-3">
        <div className="flex-1 min-w-0">
          {isEditing ? (
            <textarea
              value={draftText}
              onChange={(e) => setDraftText(e.target.value)}
              rows={2}
              className="w-full px-3 py-2 border border-blue-300 rounded-md text-sm focus:outline-none focus:ring-1 focus:ring-blue-500 focus:border-blue-500 resize-y"
              autoFocus
            />
          ) : (
            <p
              className={`text-sm text-foreground whitespace-pre-wrap break-words ${
                isRejected ? 'line-through text-muted-foreground' : ''
              }`}
            >
              {item.text}
            </p>
          )}

          {(item.assignee || item.due) && !isEditing && (
            <div className="flex items-center flex-wrap gap-x-3 gap-y-1 mt-1 text-xs text-muted-foreground">
              {item.assignee && (
                <span>
                  <span className="font-medium">Assignee:</span> {item.assignee}
                </span>
              )}
              {item.due && (
                <span>
                  <span className="font-medium">Due:</span> {item.due}
                </span>
              )}
            </div>
          )}

          <div className="flex items-center flex-wrap gap-x-3 gap-y-1 mt-2">
            <StatusChip status={item.status} />
            <SourceLink
              sourceChunkId={item.source_chunk_id}
              onJumpToSource={onJumpToSource}
            />
          </div>

          {isRejecting && (
            <input
              type="text"
              value={rejectReason}
              onChange={(e) => setRejectReason(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter') {
                  e.preventDefault();
                  confirmReject();
                } else if (e.key === 'Escape') {
                  e.preventDefault();
                  cancelReject();
                }
              }}
              placeholder="Why is this wrong? (optional — press Enter to reject)"
              aria-label="Reason for rejecting this action item (optional)"
              className="mt-2 w-full px-3 py-2 border border-red-300 dark:border-red-500/40 rounded-md text-sm focus:outline-none focus:ring-1 focus:ring-red-500 focus:border-red-500"
              autoFocus
            />
          )}
        </div>

        <div className="flex-shrink-0 flex items-center gap-1">
          {isEditing ? (
            <>
              <Button
                variant="green"
                size="sm"
                onClick={saveEdit}
                disabled={isSaving}
                title="Save edit"
              >
                {isSaving ? (
                  <Loader2 className="h-4 w-4 animate-spin" />
                ) : (
                  <Check className="h-4 w-4" />
                )}
                <span className="hidden lg:inline">Save</span>
              </Button>
              <Button
                variant="outline"
                size="sm"
                onClick={cancelEdit}
                disabled={isSaving}
                title="Cancel edit"
              >
                <X className="h-4 w-4" />
              </Button>
            </>
          ) : isRejecting ? (
            <>
              <Button
                variant="red"
                size="sm"
                onClick={confirmReject}
                disabled={isPending}
                title="Reject action item"
                aria-label="Confirm reject"
              >
                {isPending ? (
                  <Loader2 className="h-4 w-4 animate-spin" />
                ) : (
                  <X className="h-4 w-4" />
                )}
                <span className="hidden lg:inline">Reject</span>
              </Button>
              <Button
                variant="ghost"
                size="sm"
                onClick={cancelReject}
                disabled={isPending}
                title="Cancel"
                aria-label="Cancel reject"
              >
                Cancel
              </Button>
            </>
          ) : isRejected ? (
            <Button
              variant="outline"
              size="sm"
              onClick={onRestore}
              disabled={isPending}
              title="Restore to draft"
            >
              {isPending ? (
                <Loader2 className="h-4 w-4 animate-spin" />
              ) : (
                <RotateCcw className="h-4 w-4" />
              )}
              <span className="hidden lg:inline">Restore</span>
            </Button>
          ) : (
            <>
              <Button
                variant={item.status === 'approved' ? 'green' : 'outline'}
                size="sm"
                onClick={onApprove}
                disabled={isPending || item.status === 'approved'}
                title="Approve action item"
                aria-label="Approve action item"
              >
                {isPending ? (
                  <Loader2 className="h-4 w-4 animate-spin" />
                ) : (
                  <Check className="h-4 w-4" />
                )}
              </Button>
              <Button
                variant="outline"
                size="sm"
                onClick={startEdit}
                disabled={isPending}
                title="Edit action item"
                aria-label="Edit action item"
              >
                <Pencil className="h-4 w-4" />
              </Button>
              <Button
                variant="outline"
                size="sm"
                onClick={startReject}
                disabled={isPending}
                title="Reject action item"
                aria-label="Reject action item"
              >
                <X className="h-4 w-4 text-red-600 dark:text-red-400" />
              </Button>
            </>
          )}
        </div>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Main view
// ---------------------------------------------------------------------------

export function DraftSummaryView({
  meetingId,
  draftResponse,
  isLoading = false,
  error = null,
  meetingTitle,
  meetingDate,
  onJumpToSource,
  onSummaryApproved,
}: DraftSummaryViewProps) {
  const [draft, setDraft] = useState<MeetingNotesDraft | null>(
    draftResponse?.draft ?? null,
  );
  const [actionItems, setActionItems] = useState<ActionItemDraft[]>(
    draftResponse?.action_items ?? [],
  );
  const [summaryStatus, setSummaryStatus] = useState(
    draftResponse?.status ?? 'draft',
  );
  const [isApprovingSummary, setIsApprovingSummary] = useState(false);

  // Sync internal review state whenever a fresh draft payload arrives (the async
  // fetch resolves after mount, meeting change, or a post-approve refetch). The
  // parent only swaps this reference for authoritative server truth, so it is
  // safe to adopt it wholesale.
  useEffect(() => {
    setDraft(draftResponse?.draft ?? null);
    setActionItems(draftResponse?.action_items ?? []);
    setSummaryStatus(draftResponse?.status ?? 'draft');
  }, [draftResponse]);

  const blockAction = useHitlAction();
  const itemAction = useHitlAction();

  // Immutably replace one block inside the nested draft structure.
  const patchBlock = useCallback(
    (blockId: string, patch: (b: DraftBlock) => DraftBlock) => {
      setDraft((prev) => {
        if (!prev) return prev;
        return {
          ...prev,
          sections: prev.sections.map((section) => ({
            ...section,
            blocks: section.blocks.map((b) =>
              b.id === blockId ? patch(b) : b,
            ),
          })),
        };
      });
    },
    [],
  );

  const patchItem = useCallback(
    (itemId: string, patch: (i: ActionItemDraft) => ActionItemDraft) => {
      setActionItems((prev) =>
        prev.map((i) => (i.id === itemId ? patch(i) : i)),
      );
    },
    [],
  );

  const allBlocks = useMemo(
    () => draft?.sections.flatMap((s) => s.blocks) ?? [],
    [draft],
  );

  // The first rendered block gets the product-tour anchor (step 2 points at a
  // real source-linked block + its Approve control).
  const firstBlockId = allBlocks[0]?.id;

  // The §4 approval gate, evaluated client-side to drive the button's enabled
  // state + tooltip. The backend re-enforces it authoritatively at approve time
  // (including live source re-resolution), so `false` from the command is still
  // handled as a soft no-op.
  const nonRejected = allBlocks.filter((b) => b.status !== 'rejected');
  const allNonRejectedApproved =
    nonRejected.length > 0 && nonRejected.every((b) => b.status === 'approved');
  const isSummaryApproved = summaryStatus === 'approved';
  const isBlockMutationPending = blockAction.pendingId !== null;
  const blockReviewLocked = areBlockReviewControlsLocked(
    summaryStatus,
    isApprovingSummary,
    isBlockMutationPending,
  );

  const approveGateReason = useMemo(() => {
    if (isSummaryApproved) return 'This summary has already been approved.';
    if (isBlockMutationPending)
      return 'Wait for the current block review change to finish.';
    if (allBlocks.length === 0) return 'There are no blocks to approve.';
    if (nonRejected.length === 0)
      return 'Every block is rejected. Restore and approve at least one block first.';
    if (!allNonRejectedApproved)
      return 'Approve (or reject) every block before approving the whole summary.';
    return null;
  }, [
    isSummaryApproved,
    isBlockMutationPending,
    allBlocks.length,
    nonRejected.length,
    allNonRejectedApproved,
  ]);

  // --- Export (C4.1) -------------------------------------------------------
  // The export reads the LIVE in-component review state (blocks/items the user
  // just approved locally), not the stale prop, so it works the instant the
  // summary flips to approved. Provenance fields (model/approved_*) come from the
  // last fetched payload; the renderer degrades gracefully when they are absent.
  const liveDraftResponse = useMemo<SummaryDraftResponse | null>(() => {
    if (!draftResponse) return null;
    return {
      ...draftResponse,
      draft,
      status: summaryStatus,
      action_items: actionItems,
    };
  }, [draftResponse, draft, summaryStatus, actionItems]);

  const { exportMarkdown, exportDocx, exportPdf, isExporting } =
    useExportOperations({
      meetingId,
      draftResponse: liveDraftResponse,
      meetingTitle,
      meetingDate,
    });

  // Approved-only disclosure: how many action items are NOT approved (they are
  // excluded from the export). Mirrors buildExportDoc's excluded count.
  const excludedActionItemCount = useMemo(
    () => actionItems.filter((i) => i.status !== 'approved').length,
    [actionItems],
  );

  // --- Block handlers ------------------------------------------------------
  const approveBlock = (block: DraftBlock) => {
    const prevStatus = block.status;
    void blockAction.run({
      key: block.id,
      optimistic: () => patchBlock(block.id, (b) => ({ ...b, status: 'approved' })),
      revert: () => patchBlock(block.id, (b) => ({ ...b, status: prevStatus })),
      command: () => summaryDraftService.approveBlock(meetingId, block.id),
      softFailMessage:
        "Couldn't approve this block. Its source segment may no longer exist.",
    });
  };

  const rejectBlock = (block: DraftBlock, reason?: string) => {
    const prevStatus = block.status;
    void blockAction.run({
      key: block.id,
      optimistic: () => patchBlock(block.id, (b) => ({ ...b, status: 'rejected' })),
      revert: () => patchBlock(block.id, (b) => ({ ...b, status: prevStatus })),
      command: () => summaryDraftService.rejectBlock(meetingId, block.id, reason),
      softFailMessage: "Couldn't reject this block.",
    });
  };

  const restoreBlock = (block: DraftBlock) => {
    void blockAction.run({
      key: block.id,
      optimistic: () => patchBlock(block.id, (b) => ({ ...b, status: 'draft' })),
      revert: () => patchBlock(block.id, (b) => ({ ...b, status: 'rejected' })),
      command: () => summaryDraftService.restoreBlock(meetingId, block.id),
      softFailMessage: "Couldn't restore this block.",
    });
  };

  const editBlock = async (block: DraftBlock, content: string): Promise<boolean> => {
    const prev = { content: block.content, status: block.status, original: block.original_content };
    return blockAction.run({
      key: block.id,
      optimistic: () =>
        patchBlock(block.id, (b) => ({
          ...b,
          content,
          status: 'edited',
          // Preserve the first-edit original locally to mirror the backend.
          original_content: b.original_content ?? b.content,
        })),
      revert: () =>
        patchBlock(block.id, (b) => ({
          ...b,
          content: prev.content,
          status: prev.status,
          original_content: prev.original,
        })),
      command: () => summaryDraftService.editBlock(meetingId, block.id, content),
      softFailMessage:
        "Couldn't save this edit. A rejected block must be restored first.",
    });
  };

  // --- Action item handlers ------------------------------------------------
  const approveItem = (item: ActionItemDraft) => {
    const prevStatus = item.status;
    void itemAction.run({
      key: item.id,
      optimistic: () => patchItem(item.id, (i) => ({ ...i, status: 'approved' })),
      revert: () => patchItem(item.id, (i) => ({ ...i, status: prevStatus })),
      command: () => summaryDraftService.approveActionItem(item.id),
      softFailMessage:
        "Couldn't approve this action item. Its source segment may no longer exist.",
    });
  };

  const rejectItem = (item: ActionItemDraft, reason?: string) => {
    const prevStatus = item.status;
    void itemAction.run({
      key: item.id,
      optimistic: () => patchItem(item.id, (i) => ({ ...i, status: 'rejected' })),
      revert: () => patchItem(item.id, (i) => ({ ...i, status: prevStatus })),
      command: () => summaryDraftService.rejectActionItem(item.id, reason),
      softFailMessage: "Couldn't reject this action item.",
    });
  };

  const restoreItem = (item: ActionItemDraft) => {
    void itemAction.run({
      key: item.id,
      optimistic: () => patchItem(item.id, (i) => ({ ...i, status: 'draft' })),
      revert: () => patchItem(item.id, (i) => ({ ...i, status: 'rejected' })),
      command: () => summaryDraftService.restoreActionItem(item.id),
      softFailMessage: "Couldn't restore this action item.",
    });
  };

  const editItemText = async (item: ActionItemDraft, text: string): Promise<boolean> => {
    const prev = { text: item.text, status: item.status };
    return itemAction.run({
      key: item.id,
      optimistic: () =>
        patchItem(item.id, (i) => ({ ...i, text, status: 'edited' })),
      revert: () =>
        patchItem(item.id, (i) => ({ ...i, text: prev.text, status: prev.status })),
      command: () => summaryDraftService.editActionItem(item.id, { text }),
      softFailMessage:
        "Couldn't save this edit. A rejected action item must be restored first.",
    });
  };

  // --- Whole-summary approval ---------------------------------------------
  const approveWholeSummary = async () => {
    setIsApprovingSummary(true);
    try {
      const ok = await summaryDraftService.approveSummary(meetingId);
      if (ok) {
        // Keep both lifecycle copies aligned with the authoritative successful
        // command; export independently verifies both before emitting a file.
        setDraft((prev) => (prev ? { ...prev, status: 'approved' } : prev));
        setSummaryStatus('approved');
        toast.success('Summary approved');
        onSummaryApproved?.();
      } else {
        toast.warning("Couldn't approve the summary", {
          description:
            'Every block must be approved and its source segment must still exist.',
        });
      }
    } catch (err) {
      console.error('[DraftSummaryView] approveSummary failed:', err);
      toast.error("Couldn't approve the summary", {
        description: 'Please try again in a moment.',
      });
    } finally {
      setIsApprovingSummary(false);
    }
  };

  // --- Render states -------------------------------------------------------
  if (isLoading) {
    return (
      <div className="w-full">
        <ReviewRequiredBanner />
        <div className="flex items-center gap-3 p-4 text-muted-foreground">
          <Loader2 className="h-5 w-5 animate-spin" />
          <span className="text-sm">Loading summary draft…</span>
        </div>
      </div>
    );
  }

  if (error) {
    return (
      <div className="w-full">
        <ReviewRequiredBanner />
        <div className="p-4 bg-red-50 dark:bg-red-500/10 border border-red-200 dark:border-red-500/25 rounded-lg">
          <p className="text-sm text-red-700 dark:text-red-300 font-medium">
            Couldn&apos;t load the summary draft
          </p>
          <p className="text-sm text-red-600 dark:text-red-400 mt-1">{error}</p>
        </div>
      </div>
    );
  }

  const hasDraftBody = !!draft && draft.sections.some((s) => s.blocks.length > 0);
  const hasActionItems = actionItems.length > 0;

  if (!hasDraftBody && !hasActionItems) {
    return (
      <div className="w-full">
        {/* Banner stays visible even when empty, so the AI-labeling is never lost. */}
        <ReviewRequiredBanner />
        <div className="p-4 bg-muted border border-border rounded-lg text-center">
          <p className="text-sm text-muted-foreground">No summary draft to review yet.</p>
          <p className="text-xs text-muted-foreground mt-1">
            Generate a summary to see source-linked blocks and action items here.
          </p>
        </div>
      </div>
    );
  }

  const approveButton = (
    <Button
      variant="green"
      size="sm"
      onClick={approveWholeSummary}
      disabled={
        isApprovingSummary ||
        isBlockMutationPending ||
        isSummaryApproved ||
        approveGateReason !== null
      }
    >
      {isApprovingSummary ? (
        <Loader2 className="h-4 w-4 animate-spin" />
      ) : (
        <Check className="h-4 w-4" />
      )}
      {isSummaryApproved ? 'Summary approved' : 'Approve summary'}
    </Button>
  );

  // Export is gated on an APPROVED summary (ADR-0019 decision 1). The control is
  // an "Export ▾" menu offering Markdown / Word / PDF; every format flows through
  // the same approved-only pipeline (`useExportOperations`), differing only in the
  // renderer. When the summary is NOT approved the trigger is disabled and wrapped
  // in an explanatory tooltip below (mirroring the pre-C4.2 single-button gate).
  const exportTrigger = (
    <Button
      variant="outline"
      size="sm"
      disabled={!isSummaryApproved || isExporting}
      aria-label="Export approved summary"
    >
      {isExporting ? (
        <Loader2 className="h-4 w-4 animate-spin" />
      ) : (
        <Download className="h-4 w-4" />
      )}
      Export
      <ChevronDown className="h-4 w-4" />
    </Button>
  );

  const exportMenu = (
    <DropdownMenu>
      <DropdownMenuTrigger asChild disabled={!isSummaryApproved || isExporting}>
        {exportTrigger}
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end">
        <DropdownMenuItem
          onClick={() => void exportMarkdown()}
          className="gap-2"
        >
          <FileText className="h-4 w-4" />
          <span>Markdown (.md)</span>
        </DropdownMenuItem>
        <DropdownMenuItem onClick={() => void exportDocx()} className="gap-2">
          <FileType className="h-4 w-4" />
          <span>Word (.docx)</span>
        </DropdownMenuItem>
        <DropdownMenuItem onClick={() => void exportPdf()} className="gap-2">
          <FileDown className="h-4 w-4" />
          <span>PDF (.pdf)</span>
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  );

  return (
    <div className="w-full">
      <ReviewRequiredBanner />

      {/* Approve-summary bar */}
      <div className="flex items-center justify-between gap-3 mb-2">
        <div className="text-sm text-muted-foreground">
          {isSummaryApproved ? (
            <span className="inline-flex items-center gap-1 text-green-700 dark:text-green-400 font-medium">
              <Check className="h-4 w-4" /> Approved
            </span>
          ) : (
            <span>
              {nonRejected.filter((b) => b.status === 'approved').length} of{' '}
              {nonRejected.length} block(s) approved
            </span>
          )}
        </div>
        <div className="flex items-center gap-2">
          {/* Export control: the "Export ▾" menu is enabled only for an approved
              summary; otherwise the disabled trigger is wrapped in a tooltip. */}
          {isSummaryApproved ? (
            exportMenu
          ) : (
            <TooltipProvider>
              <Tooltip>
                {/* Wrap the disabled trigger so the tooltip still fires. */}
                <TooltipTrigger asChild>
                  <span tabIndex={0}>{exportTrigger}</span>
                </TooltipTrigger>
                <TooltipContent>Approve the summary to export</TooltipContent>
              </Tooltip>
            </TooltipProvider>
          )}
          {approveGateReason ? (
            <TooltipProvider>
              <Tooltip>
                {/* Wrap the disabled button so the tooltip still fires. */}
                <TooltipTrigger asChild>
                  <span tabIndex={0}>{approveButton}</span>
                </TooltipTrigger>
                <TooltipContent>{approveGateReason}</TooltipContent>
              </Tooltip>
            </TooltipProvider>
          ) : (
            approveButton
          )}
        </div>
      </div>

      {/* Approved-only disclosure near the Export control. */}
      {isSummaryApproved && excludedActionItemCount > 0 && (
        <p className="text-xs text-amber-700 dark:text-amber-300 mb-4">
          {excludedActionItemCount} action item
          {excludedActionItemCount === 1 ? '' : 's'} not yet approved — not
          included in the export.
        </p>
      )}

      {/* Sections -> read.ai-style report cards (docs/DESIGN_READAI.md). Each card
          keeps the HITL affordances: per-block status chip, approve/reject/edit and
          the source-link chip back to the transcript segment. */}
      <div className="space-y-4">
        {draft?.sections.map((section, sIdx) => (
          <SectionCard
            key={`${section.title}-${sIdx}`}
            icon={sectionIcon(section.title)}
            title={section.title}
            count={section.blocks.length}
            accent={sIdx === 0}
            aiLabel
          >
            <div className="divide-y divide-border">
              {section.blocks.map((block) => (
                <BlockRow
                  key={block.id}
                  block={block}
                  isPending={blockAction.pendingId === block.id}
                  reviewLocked={blockReviewLocked}
                  onApprove={() => approveBlock(block)}
                  onReject={(reason) => rejectBlock(block, reason)}
                  onRestore={() => restoreBlock(block)}
                  onEdit={(content) => editBlock(block, content)}
                  onJumpToSource={onJumpToSource}
                  isTourAnchor={block.id === firstBlockId}
                />
              ))}
            </div>
          </SectionCard>
        ))}

        {/* Action items */}
        {hasActionItems && (
          <SectionCard icon={ListChecks} title="Action items" count={actionItems.length} aiLabel>
            <div className="divide-y divide-border">
              {actionItems.map((item) => (
                <ActionItemRow
                  key={item.id}
                  item={item}
                  isPending={itemAction.pendingId === item.id}
                  onApprove={() => approveItem(item)}
                  onReject={(reason) => rejectItem(item, reason)}
                  onRestore={() => restoreItem(item)}
                  onEditText={(text) => editItemText(item, text)}
                  onJumpToSource={onJumpToSource}
                />
              ))}
            </div>
          </SectionCard>
        )}
      </div>
    </div>
  );
}

export default DraftSummaryView;
