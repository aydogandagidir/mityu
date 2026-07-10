'use client';

import { motion } from 'framer-motion';
import { Clock, ListChecks, MessageSquareQuote, Sparkles } from 'lucide-react';
import { Button } from '@/components/ui/button';
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from '@/components/ui/tooltip';

interface EmptyStateSummaryProps {
  onGenerate: () => void;
  hasModel: boolean;
  isGenerating?: boolean;
}

export function EmptyStateSummary({ onGenerate, hasModel, isGenerating = false }: EmptyStateSummaryProps) {
  return (
    <motion.div
      initial={{ opacity: 0, scale: 0.95 }}
      animate={{ opacity: 1, scale: 1 }}
      transition={{ duration: 0.3, ease: 'easeOut' }}
      className="flex flex-col items-center justify-center h-full p-8 text-center"
    >
      {/* Brand empty-state mark: a primary tile flanked by the report's section icons. */}
      <div className="mb-5 flex items-center gap-2.5" aria-hidden>
        <span className="grid h-10 w-10 place-items-center rounded-xl bg-accent text-accent-foreground">
          <MessageSquareQuote className="h-5 w-5" />
        </span>
        <span className="grid h-14 w-14 place-items-center rounded-2xl bg-primary text-primary-foreground shadow-md">
          <Sparkles className="h-7 w-7" />
        </span>
        <span className="grid h-10 w-10 place-items-center rounded-xl bg-accent text-accent-foreground">
          <ListChecks className="h-5 w-5" />
        </span>
      </div>
      <h3 className="text-lg font-semibold tracking-tight text-foreground mb-1.5">
        No summary yet
      </h3>
      <p className="text-sm text-muted-foreground mb-2 max-w-sm leading-relaxed">
        Turn this transcript into a structured report — summary, key points and action
        items, each linked to its source segment.
      </p>
      <p className="mb-6 inline-flex items-center gap-1.5 text-xs text-muted-foreground/80">
        <Clock className="h-3 w-3" aria-hidden />
        Drafts stay on this device until you review and approve them.
      </p>

      <TooltipProvider>
        <Tooltip>
          <TooltipTrigger asChild>
            <div>
              <Button
                onClick={onGenerate}
                disabled={!hasModel || isGenerating}
                className="gap-2"
              >
                <Sparkles className="w-4 h-4" />
                {isGenerating ? 'Generating...' : 'Generate Summary'}
              </Button>
            </div>
          </TooltipTrigger>
          {!hasModel && (
            <TooltipContent>
              <p>Please select a model in Settings first</p>
            </TooltipContent>
          )}
        </Tooltip>
      </TooltipProvider>

      {!hasModel && (
        <p className="text-xs text-amber-600 dark:text-amber-400 mt-3">
          Please select a model in Settings first
        </p>
      )}
    </motion.div>
  );
}
