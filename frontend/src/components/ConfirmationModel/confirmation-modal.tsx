'use client';

/**
 * The destructive confirmation — DESIGN_SYSTEM.md §6.8.
 *
 * It was a hand-rolled fixed overlay: raw `bg-black bg-opacity-50`, a red button from the
 * palette rather than the token, no focus trap, no Escape handling, and a heading hard-coded
 * to "Confirm Delete" — so it recited ninety words about SSD wear-levelling and
 * copy-on-write filesystems without ever saying WHICH meeting was about to be destroyed.
 *
 * Now it is the `Dialog` primitive, which supplies the focus trap, Escape and the
 * 🔒 `role="dialog"` + `aria-modal` pair the register freezes, and the content is ordered
 * the way a person reads it: the name of the thing, one sentence about what happens, and
 * the full disclosure one disclosure away under "What exactly is removed".
 *
 * 🔒 Preserved exactly: `aria-labelledby="delete-confirmation-title"`, `aria-busy` while
 * the delete runs, the Cancel and Delete labels, Delete becoming "Deleting…", and the
 * ADR-0026 disclosure text passed in by the caller — verbatim, never summarised.
 */

import React from 'react';
import { TriangleAlert } from 'lucide-react';
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogTitle,
} from '@/components/ui/dialog';
import { Button } from '@/components/ui/button';

interface ConfirmationModalProps {
  onConfirm: () => void | Promise<void>;
  onCancel: () => void;
  /** 🔒 The full disclosure, verbatim. Rendered under the details summary. */
  text: string;
  isOpen: boolean;
  isBusy?: boolean;
  /**
   * Names the thing being destroyed. Optional and defaulted, so an older call site is
   * unchanged — but a confirmation that recites a disclosure without saying WHAT it is
   * about to delete is asking the user to approve a blank cheque.
   */
  title?: string;
  /** One sentence of consequence, above the fold. */
  lead?: string;
  /** The confirm button's label. */
  confirmLabel?: string;
  busyLabel?: string;
}

export function ConfirmationModal({
  onConfirm,
  onCancel,
  text,
  isOpen,
  isBusy = false,
  title = 'Confirm Delete',
  lead = 'This cannot be undone.',
  confirmLabel = 'Delete',
  busyLabel = 'Deleting…',
}: ConfirmationModalProps) {
  return (
    <Dialog
      open={isOpen}
      onOpenChange={(open) => {
        // A dismissal must never be read as consent to destroy something.
        if (!open && !isBusy) onCancel();
      }}
    >
      <DialogContent
        aria-labelledby="delete-confirmation-title"
        aria-busy={isBusy}
        className="sm:max-w-[520px]"
      >
        <div className="flex items-start gap-3">
          <span className="grid size-9 shrink-0 place-items-center rounded-md bg-destructive-surface text-destructive-ink">
            <TriangleAlert className="size-5" aria-hidden="true" />
          </span>
          <div className="min-w-0 flex-1 space-y-2">
            <DialogTitle id="delete-confirmation-title" className="text-title">
              {title}
            </DialogTitle>
            <p className="text-body text-muted-foreground">{lead}</p>
            <details className="group rounded-md border border-border bg-surface-2 px-3 py-2">
              <summary className="cursor-pointer text-label text-foreground marker:text-subtle-foreground">
                What exactly is removed
              </summary>
              <p className="mt-2 text-caption text-muted-foreground">{text}</p>
            </details>
          </div>
        </div>

        <DialogFooter className="gap-2">
          {/* Focus lands on Cancel, not on the destructive action: Enter on an
              unread dialog must not delete a meeting. */}
          <Button variant="outline" onClick={onCancel} disabled={isBusy} autoFocus>
            Cancel
          </Button>
          <Button variant="destructive" onClick={() => void onConfirm()} disabled={isBusy}>
            {isBusy ? busyLabel : confirmLabel}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
