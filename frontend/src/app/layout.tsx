'use client'

import './globals.css'
import { DM_Sans } from 'next/font/google'
import { Toaster } from 'sonner'
import "sonner/dist/styles.css"
import { usePathname } from 'next/navigation'
import { ThemeProvider } from '@/components/theme-provider'
import { AppShell } from '@/components/AppShell'

// Same face as the landing page (closest open-license match to the reference
// site's Google Sans, which is proprietary). Self-hosted by next/font at build
// time — no runtime network dependency (local-first).
const dmSans = DM_Sans({
  subsets: ['latin'],
  weight: ['400', '500', '600', '700'],
  variable: '--font-dm-sans',
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
      <body className={`${dmSans.variable} font-sans antialiased`}>
        <ThemeProvider attribute="class" defaultTheme="system" enableSystem disableTransitionOnChange>
          {isBareRoute ? children : <AppShell>{children}</AppShell>}

          <Toaster position="bottom-center" richColors closeButton />
        </ThemeProvider>
      </body>
    </html>
  )
}
