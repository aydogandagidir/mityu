'use client';

import { Mic, Sparkles } from 'lucide-react';
import { Dialog, DialogContent, DialogTitle, DialogDescription } from '@/components/ui/dialog';
import { Button } from '@/components/ui/button';
import { WELCOME_COPY } from '@/lib/tour';

interface WelcomeOverlayProps {
  open: boolean;
  /** Primary — begins the coach-mark tour. */
  onTakeTour: () => void;
  /** Secondary — dismiss and hand off to the user's first recording. */
  onSkip: () => void;
  /** Light dismiss (close button / Esc / outside click). */
  onDismiss: () => void;
}

/**
 * The one-time welcome card shown on top of the sample meeting. Reuses the repo
 * Dialog styling; calm, lots of whitespace, one clear brand-blue primary action.
 * It only explains and offers a tour — it never approves any AI output.
 */
export function WelcomeOverlay({ open, onTakeTour, onSkip, onDismiss }: WelcomeOverlayProps) {
  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        if (!next) onDismiss();
      }}
    >
      <DialogContent className="sm:max-w-lg sm:rounded-2xl p-8">
        <div className="flex flex-col items-center text-center">
          <div className="flex h-12 w-12 items-center justify-center rounded-full bg-primary/10 text-primary">
            <Sparkles className="h-6 w-6" />
          </div>

          <DialogTitle className="mt-5 text-2xl font-semibold tracking-tight text-foreground">
            {WELCOME_COPY.title}
          </DialogTitle>

          <DialogDescription className="mt-3 text-base leading-relaxed text-muted-foreground">
            {WELCOME_COPY.body}
          </DialogDescription>

          <div className="mt-7 flex w-full flex-col gap-2.5">
            <Button size="lg" className="w-full" onClick={onTakeTour}>
              {WELCOME_COPY.primary}
            </Button>
            <Button
              variant="ghost"
              size="lg"
              className="w-full text-muted-foreground hover:text-foreground"
              onClick={onSkip}
            >
              <Mic className="h-4 w-4" />
              {WELCOME_COPY.secondary}
            </Button>
          </div>

          <p className="mt-5 text-xs text-muted-foreground/80">{WELCOME_COPY.footer}</p>
        </div>
      </DialogContent>
    </Dialog>
  );
}

export default WelcomeOverlay;
