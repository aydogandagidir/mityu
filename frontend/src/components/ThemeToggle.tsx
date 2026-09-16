'use client';

import { useEffect, useState } from 'react';
import { useTheme } from 'next-themes';
import { Monitor, Moon, Sun } from 'lucide-react';
import { focusRing } from '@/components/ui/focus-ring';
import { cn } from '@/lib/utils';

const OPTIONS = [
  { value: 'system', label: 'System', Icon: Monitor },
  { value: 'light', label: 'Light', Icon: Sun },
  { value: 'dark', label: 'Dark', Icon: Moon },
] as const;

/**
 * Segmented System / Light / Dark control backed by next-themes — DESIGN_SYSTEM.md §5.6.
 *
 * Guarded with a `mounted` flag: the resolved theme is only known on the client, so a
 * neutral skeleton renders on the server to avoid a hydration mismatch.
 *
 * The selected segment is marked by SURFACE AND WEIGHT, not by colour alone: `aria-checked`
 * carries it for a screen reader, and sighted users get a raised card plus the 500 weight,
 * which survives a monochrome display and greyscale printing (§4.8).
 *
 * `shadow-elev-1` rather than `shadow-sm`, `size-4` rather than `h-4 w-4`, `text-label`
 * rather than `text-sm`: the tokens are the contract, and this file predates them.
 */
export function ThemeToggle() {
  const { theme, setTheme } = useTheme();
  const [mounted, setMounted] = useState(false);
  useEffect(() => setMounted(true), []);

  const active = mounted ? (theme ?? 'system') : undefined;

  return (
    <div
      role="radiogroup"
      aria-label="Theme"
      className="inline-flex items-center gap-1 rounded-lg border border-border bg-muted p-1"
    >
      {OPTIONS.map(({ value, label, Icon }) => {
        const isActive = active === value;
        return (
          <button
            key={value}
            type="button"
            role="radio"
            aria-checked={isActive}
            onClick={() => setTheme(value)}
            className={cn(
              'inline-flex h-8 items-center gap-1.5 rounded-md px-3 text-label transition-colors',
              focusRing('muted'),
              isActive
                ? 'bg-background font-medium text-foreground shadow-elev-1'
                : 'text-muted-foreground hover:text-foreground'
            )}
          >
            <Icon className="size-4" aria-hidden />
            {label}
          </button>
        );
      })}
    </div>
  );
}
