'use client'

import './globals.css'
import { Inter } from 'next/font/google'
import { Toaster } from 'sonner'
import "sonner/dist/styles.css"
import { usePathname } from 'next/navigation'
import { ThemeProvider } from '@/components/theme-provider'
import { AppShell } from '@/components/AppShell'

// Inter (DESIGN_SYSTEM.md §4.1, ADR-B). Drawn for 12-16px UI, with the x-height to
// hold up at the 13px label / 12px caption sizes this density needs, true tabular
// figures for every timestamp, duration and mm:ss, and latin-ext coverage for Turkish
// (s-cedilla, g-breve, dotted/dotless i). Self-hosted by next/font at BUILD time — no
// runtime network dependency, which the Tauri CSP would refuse anyway (local-first).
const sans = Inter({
  subsets: ['latin', 'latin-ext'],
  weight: ['400', '500', '600'],
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

// export { metadata } from './metadata'

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
        <ThemeProvider attribute="class" defaultTheme="system" enableSystem disableTransitionOnChange>
          {isBareRoute ? children : <AppShell>{children}</AppShell>}

          <Toaster position="bottom-center" richColors closeButton />
        </ThemeProvider>
      </body>
    </html>
  )
}
