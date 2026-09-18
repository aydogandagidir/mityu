'use client';

/**
 * One settings card — DESIGN_SYSTEM.md §6.5.
 *
 * Settings were nine unrelated cards stacked in one "General" tab, each declaring its own
 * `bg-card rounded-lg border p-6 shadow-sm`, so a change to how a settings card looks
 * meant finding every copy. This is the copy.
 */

import React from 'react';
import { Card } from '@/components/ui/card';
import { cn } from '@/lib/utils';

export interface SettingsCardProps {
  title?: React.ReactNode;
  description?: React.ReactNode;
  /** Sits opposite the title — one control, for a card that is a single setting. */
  action?: React.ReactNode;
  children?: React.ReactNode;
  className?: string;
  id?: string;
}

export function SettingsCard({
  title,
  description,
  action,
  children,
  className,
  id,
}: SettingsCardProps) {
  return (
    <Card id={id} className={cn('p-5', className)}>
      {(title || action) && (
        <div className="flex flex-wrap items-start justify-between gap-3">
          <div className="min-w-0">
            {title ? <h3 className="text-title text-foreground">{title}</h3> : null}
            {description ? (
              <p className="mt-1 text-body text-muted-foreground">{description}</p>
            ) : null}
          </div>
          {action ? <div className="shrink-0">{action}</div> : null}
        </div>
      )}
      {children ? <div className={cn(title || action ? 'mt-4' : '')}>{children}</div> : null}
    </Card>
  );
}

export default SettingsCard;
