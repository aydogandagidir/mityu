import { Check, FileEdit, History, Pencil, X, type LucideIcon } from 'lucide-react';

import { Badge } from '@/components/ui/badge';
import { cn } from '@/lib/utils';

/**
 * StatusPill — DESIGN_SYSTEM.md §5.8, the review state of one block, one action item or one
 * meeting row.
 *
 * WHY THIS WRAPPER EXISTS AT ALL. `Badge` can render a tint with no word; this cannot. The
 * word and the icon come from the same table as the tone, so "draft vs approved" survives
 * greyscale, a colour-vision deficiency, a printed export and a screenshot pasted into a
 * dispute file — which is the only context where this product's output matters. §8: status
 * is NEVER conveyed by colour alone.
 *
 * `label` overrides the word for a progress variant ("5 of 7 approved" on a meeting row,
 * §5.15) — it never removes the icon.
 *
 * 🔒 The REJECTED state also renders the block's own text with `line-through` at the call
 * site (§5.8); the pill is one half of that pair.
 */
export type ReviewStatus = 'draft' | 'approved' | 'edited' | 'rejected' | 'legacy';

const STATUS: Record<
  ReviewStatus,
  { word: string; icon: LucideIcon; tone: 'ai' | 'verified' | 'edited' | 'destructive' | 'neutral' }
> = {
  draft: { word: 'Draft', icon: FileEdit, tone: 'ai' },
  approved: { word: 'Approved', icon: Check, tone: 'verified' },
  edited: { word: 'Edited', icon: Pencil, tone: 'edited' },
  rejected: { word: 'Rejected', icon: X, tone: 'destructive' },
  legacy: { word: 'Legacy', icon: History, tone: 'neutral' },
};

export function StatusPill({
  status,
  label,
  className,
}: {
  status: ReviewStatus;
  /** Replaces the word only — e.g. `5 of 7 approved`. The icon always stays. */
  label?: string;
  className?: string;
}) {
  const { word, icon: Icon, tone } = STATUS[status];
  return (
    <Badge tone={tone} className={cn(className)}>
      <Icon aria-hidden />
      {label ?? word}
    </Badge>
  );
}

export default StatusPill;
