'use client';

/**
 * A labelled switch — DESIGN_SYSTEM.md §6.5.
 *
 * The label is a real `<label>` bound to the control, so the whole row is the hit target
 * and a screen reader reads the setting rather than the word "switch". Where a setting
 * needs a consequence spelled out, `description` carries it — never a `title=` tooltip,
 * which no keyboard user ever sees.
 */

import React, { useId } from 'react';
import { Switch } from '@/components/ui/switch';
import { cn } from '@/lib/utils';

export interface SwitchRowProps {
  label: React.ReactNode;
  description?: React.ReactNode;
  checked: boolean;
  onCheckedChange: (checked: boolean) => void;
  disabled?: boolean;
  /** 🔒 Where a suite pins the switch's accessible name, pass it here. */
  ariaLabel?: string;
  className?: string;
}

export function SwitchRow({
  label,
  description,
  checked,
  onCheckedChange,
  disabled,
  ariaLabel,
  className,
}: SwitchRowProps) {
  const id = useId();
  const descriptionId = description ? `${id}-description` : undefined;

  return (
    <div className={cn('flex items-start justify-between gap-4 py-2', className)}>
      <div className="min-w-0">
        <label htmlFor={id} className="cursor-pointer text-label text-foreground">
          {label}
        </label>
        {description ? (
          <p id={descriptionId} className="mt-0.5 text-caption text-muted-foreground">
            {description}
          </p>
        ) : null}
      </div>
      <Switch
        id={id}
        checked={checked}
        onCheckedChange={onCheckedChange}
        disabled={disabled}
        aria-label={ariaLabel}
        aria-describedby={descriptionId}
        className="mt-0.5 shrink-0"
      />
    </div>
  );
}

export default SwitchRow;
