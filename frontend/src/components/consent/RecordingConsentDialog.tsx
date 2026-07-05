'use client';

import React, { useEffect, useState } from 'react';
import { Mic, Users } from 'lucide-react';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from '@/components/ui/dialog';
import { Button } from '@/components/ui/button';

/**
 * RecordingConsentDialog (BACKLOG C5).
 *
 * A pre-recording, multi-party consent affordance shown BEFORE the first
 * recording starts (KVKK/GDPR lawful-basis + EU AI Act Art. 50 transparency).
 * It explains that Mityu captures microphone + system audio LOCALLY and that the
 * user is responsible for ensuring all participants are informed / consent —
 * recording laws vary by jurisdiction.
 *
 * Unlike the permanent "AI-generated · review required" banner, this dialog IS
 * dismissable (consent is an acknowledgment, not a persistent status), but it
 * BLOCKS the recording start until the user either confirms or cancels:
 *  - Confirm  -> onConfirm(dontShowAgain) : proceed to actually start recording.
 *  - Cancel / X / ESC / outside-click -> onCancel() : abort the start.
 *
 * The "Don't show this again on this device" checkbox lets the caller persist a
 * one-time acknowledgment so the gate does not repeat by default (re-armable
 * from Settings).
 */
interface RecordingConsentDialogProps {
  open: boolean;
  /** Confirm and start recording. `dontShowAgain` requests persisting the ack. */
  onConfirm: (dontShowAgain: boolean) => void;
  /** Abort the start (cancel button, close X, ESC, or outside-click). */
  onCancel: () => void;
}

export function RecordingConsentDialog({ open, onConfirm, onCancel }: RecordingConsentDialogProps) {
  const [dontShowAgain, setDontShowAgain] = useState(false);

  // Reset the checkbox each time the dialog is (re)opened so a prior session's
  // choice never silently carries over.
  useEffect(() => {
    if (open) {
      setDontShowAgain(false);
    }
  }, [open]);

  // Radix fires onOpenChange(false) for the X button, ESC, and outside-click.
  // Every one of those paths must ABORT the start, not silently proceed.
  const handleOpenChange = (next: boolean) => {
    if (!next) {
      onCancel();
    }
  };

  return (
    <Dialog open={open} onOpenChange={handleOpenChange}>
      <DialogContent className="max-w-lg">
        <DialogHeader>
          <div className="flex items-center gap-2">
            <Mic className="h-5 w-5 text-red-500" aria-hidden="true" />
            <DialogTitle>Before you record</DialogTitle>
          </div>
          <DialogDescription className="pt-1">
            Mityu captures your microphone and system audio and transcribes it
            locally on this device. Nothing is uploaded to start a recording.
          </DialogDescription>
        </DialogHeader>

        <div className="flex items-start gap-3 rounded-lg border border-amber-300 bg-amber-50 p-3">
          <Users className="mt-0.5 h-5 w-5 flex-shrink-0 text-amber-600" aria-hidden="true" />
          <div className="text-sm text-amber-900">
            <p className="font-semibold">You are responsible for participant consent</p>
            <p className="mt-1 text-amber-800">
              Recording laws vary by jurisdiction and many require that every
              participant is informed or gives consent before recording. By
              continuing, you confirm that all participants in this
              meeting/conversation are aware they are being recorded and that you
              have any consent required where you are.
            </p>
          </div>
        </div>

        <label className="mt-1 flex cursor-pointer items-center gap-2 text-sm text-gray-700 select-none">
          <input
            type="checkbox"
            checked={dontShowAgain}
            onChange={(e) => setDontShowAgain(e.target.checked)}
            className="h-4 w-4 rounded border-gray-300 text-red-600 focus:ring-red-500"
          />
          Don&apos;t show this again on this device
        </label>

        <DialogFooter className="mt-2 gap-2">
          <Button variant="outline" onClick={onCancel}>
            Cancel
          </Button>
          <Button
            className="bg-red-600 text-white hover:bg-red-700"
            onClick={() => onConfirm(dontShowAgain)}
          >
            I confirm participants are informed — start recording
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

export default RecordingConsentDialog;
