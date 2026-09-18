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
import { Checkbox } from '@/components/ui/checkbox';

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
            <Mic className="size-5 text-recording" aria-hidden="true" />
            <DialogTitle>Before you record</DialogTitle>
          </div>
          <DialogDescription className="pt-1">
            Mityu captures your microphone and system audio and transcribes it
            locally on this device. Nothing is uploaded to start a recording.
            The raw audio remains on this device until you delete the meeting.
          </DialogDescription>
        </DialogHeader>

        {/* 🔒 The heading and the paragraph are frozen; the palette is not. These were
            three hardcoded amber values with no dark variant, so in dark mode the box
            painted near-black text on near-white inside a near-black dialog. */}
        <div className="flex items-start gap-3 rounded-md border border-warning-border bg-warning-surface p-3 text-warning-ink">
          <Users className="mt-0.5 size-5 shrink-0" aria-hidden="true" />
          <div className="text-body">
            <p className="font-semibold">You are responsible for participant consent</p>
            <p className="mt-1">
              Recording laws vary by jurisdiction and many require that every
              participant is informed or gives consent before recording. By
              continuing, you confirm that all participants in this
              meeting/conversation are aware they are being recorded and that you
              have any consent required where you are.
            </p>
          </div>
        </div>

        <label className="mt-1 flex cursor-pointer select-none items-center gap-2 text-body text-foreground">
          <Checkbox
            checked={dontShowAgain}
            onCheckedChange={(checked) => setDontShowAgain(checked === true)}
          />
          Don&apos;t show this again on this device
        </label>

        <DialogFooter className="mt-2 gap-2">
          <Button variant="outline" onClick={onCancel}>
            Cancel
          </Button>
          {/* 🔒 The string is frozen. The COLOUR is not, and red was wrong here: red is
              the indicator that a recording is running, not the semantics of a person
              confirming something. This is the one filled blue action in the dialog —
              "a human decided" — and its hover/active come from the same family, whose
              white label clears AA under exactly this sentence. */}
          <Button variant="verified" onClick={() => onConfirm(dontShowAgain)}>
            I confirm participants are informed — start recording
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

export default RecordingConsentDialog;
