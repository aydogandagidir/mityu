'use client';

import { ThemeProvider as NextThemesProvider } from 'next-themes';
import type { ComponentProps } from 'react';

/**
 * App theme provider. Toggles the `.dark` class on <html> so the semantic design
 * tokens in globals.css switch between the light and (bluedev-ink) dark palettes.
 * Defaults to following the OS setting; the user can override in Settings.
 */
export function ThemeProvider({ children, ...props }: ComponentProps<typeof NextThemesProvider>) {
  return <NextThemesProvider {...props}>{children}</NextThemesProvider>;
}
