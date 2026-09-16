'use client'

import './globals.css'
import { Inter } from 'next/font/google'
import { Toaster } from 'sonner'
import "sonner/dist/styles.css"
import { usePathname } from 'next/navigation'
import { ThemeProvider } from '@/components/theme-provider'
import { MotionConfig } from 'framer-motion'
import { AppShell } from '@/components/AppShell'

// Inter (DESIGN_SYSTEM.md §4.1, ADR-B). Drawn for 12-16px UI, with the x-height to
// hold up at the 13px label / 12px caption sizes this density needs, true tabular
// figures for every timestamp, duration and mm:ss, and latin-ext coverage for Turkish
// (s-cedilla, g-breve, dotted/dotless i). Self-hosted by next/font at BUILD time — no
// runtime network dependency, which the Tauri CSP would refuse anyway (local-first).
const sans = Inter({
  subsets: ['latin', 'latin-ext'],
  // 400/500/600 carry the §4.5 type scale. 700 is loaded for the 19 `font-bold`
  // sites still in pre-redesign files: without the real face the browser SYNTHESISES
  // bold by smearing the 600, which is heavier, wider and blurrier than Inter Bold and
  // reflows the line. Those sites migrate to 600 with their own work packages; the
  // weight is dropped again when the last one goes.
  weight: ['400', '500', '600', '700'],
  variable: '--font-sans',
  display: 'swap',
})

/**
 * Routes that render on their own, without the application shell.
 *
 * `/copilot` is the live-copilot panel (BACKLOG I1): a separate, frameless OS
 * window loading this same static export. It must not mount the sidebar,
 * onboarding check, update checker, drag-to-import listeners or trial banner —
 * see the note in `components/AppShell.tsx` for why this is a mount decision
 * rather than a CSS one.
 */
const BARE_ROUTES = ['/copilot']


export default function RootLayout({
  children,
}: {
  children: React.ReactNode
}) {
  const pathname = usePathname()
  const isBareRoute = BARE_ROUTES.some(
    (route) => pathname === route || pathname?.startsWith(`${route}/`)
  )

  return (
    <html lang="en" suppressHydrationWarning>
      <body className={`${sans.variable} font-sans antialiased`}>
        {/* Reduced motion, once, for every framer-motion animation in the tree.
            The global CSS block in globals.css collapses CSS animations and
            transitions, but framer-motion animates with JavaScript transforms, which
            no stylesheet can reach — so eleven files were animating regardless of the
            user's preference. `reducedMotion="user"` makes every `motion` component
            under it drop transform and layout animation while keeping opacity, which
            is the guidance for vestibular triggers: things may still appear, they must
            not fly. This is one provider instead of eleven `useReducedMotion()` calls
            that each have to be remembered. */}
        <MotionConfig reducedMotion="user">
        <ThemeProvider attribute="class" defaultTheme="system" enableSystem disableTransitionOnChange>
          {isBareRoute ? children : <AppShell>{children}</AppShell>}

          {/* 🔒 `position="bottom-center"` and `closeButton` are preserved byte-for-byte
              — the close control is the ONLY dismiss for an infinite download toast.
              `richColors` is deliberately dropped (ADR-0060): it paints sonner's own
              Tailwind palette, which no contrast row in the design system covers and
              which the palette guardrail forbids everywhere else in this app. Tone now
              comes from the design tokens, through the classes below. The offset reads
              the fixed-chrome token so a toast never lands under the record pill. */}
          <Toaster
            position="bottom-center"
            closeButton
            offset="calc(var(--bottom-chrome) + 16px)"
            toastOptions={{
              classNames: {
                toast:
                  'border border-border bg-card text-foreground shadow-elev-2 rounded-md',
                title: 'text-body text-foreground',
                description: 'text-caption text-muted-foreground',
                actionButton: 'bg-foreground text-background',
                cancelButton: 'bg-muted text-muted-foreground',
                closeButton: 'bg-card border-border text-muted-foreground',
                success: 'border-l-[3px] border-l-success',
                error: 'border-l-[3px] border-l-destructive',
                warning: 'border-l-[3px] border-l-warning',
                info: 'border-l-[3px] border-l-info',
              },
            }}
          />
        </ThemeProvider>
        </MotionConfig>
      </body>
    </html>
  )
}
