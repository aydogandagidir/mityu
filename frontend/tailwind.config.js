/** @type {import('tailwindcss').Config} */

// Design tokens: DESIGN_SYSTEM.md §4 (ADR-A, ADR-B, ADR-L).
//
// Every colour here resolves `hsl(var(--token))`, and every token is declared on BOTH
// `:root` and `.dark` in src/app/globals.css. NEW VALUES LAND UNDER THE OLD TOKEN NAMES,
// so the ~871 existing `bg-card` / `text-muted-foreground` call sites improve rather than
// break while the component packages land one at a time.
module.exports = {
  darkMode: ['class'],
  // `./src/lib/**`, `./src/hooks/**` and `./src/contexts/**` are scanned because they render
  // JSX too — `lib/recordingNotification.tsx` emitted NO CSS for its classes while tsc, lint
  // and vitest all passed (§11.4 rule 5, the ADR-0037 failure class).
  content: [
    './src/pages/**/*.{js,ts,jsx,tsx,mdx}',
    './src/components/**/*.{js,ts,jsx,tsx,mdx}',
    './src/app/**/*.{js,ts,jsx,tsx,mdx}',
    './src/lib/**/*.{js,ts,jsx,tsx,mdx}',
    './src/hooks/**/*.{js,ts,jsx,tsx,mdx}',
    './src/contexts/**/*.{js,ts,jsx,tsx,mdx}',
  ],
  theme: {
    extend: {
      // §4.1 — Inter, self-hosted by next/font at build time (no runtime network).
      // The fallback stack is declared so a font failure degrades to the platform UI face
      // instead of to Times; `mono` is declared so `font-mono` stops falling through.
      fontFamily: {
        sans: [
          'var(--font-sans)', 'ui-sans-serif', 'system-ui', '-apple-system',
          'Segoe UI', 'Roboto', 'Helvetica Neue', 'Arial', 'sans-serif',
        ],
        mono: [
          'ui-monospace', 'SFMono-Regular', 'SF Mono', 'Menlo', 'Consolas', 'monospace',
        ],
      },

      // §4.5 — the type scale as REAL utilities, so an undeclared step does not compile to a
      // class (the ADR-0037 phantom-class failure) and no component needs `text-[15px]`.
      // `text-eyebrow` is always paired with `uppercase`.
      fontSize: {
        display: ['1.5rem', { lineHeight: '2rem', fontWeight: '600', letterSpacing: '-0.02em' }],
        'title-lg': ['1.25rem', { lineHeight: '1.75rem', fontWeight: '600', letterSpacing: '-0.015em' }],
        title: ['1rem', { lineHeight: '1.5rem', fontWeight: '600', letterSpacing: '-0.01em' }],
        'title-sm': ['0.875rem', { lineHeight: '1.25rem', fontWeight: '600', letterSpacing: '-0.005em' }],
        read: ['0.9375rem', { lineHeight: '1.5rem', fontWeight: '400' }],
        body: ['0.875rem', { lineHeight: '1.375rem', fontWeight: '400' }],
        label: ['0.8125rem', { lineHeight: '1.125rem', fontWeight: '500' }],
        // Same 13px as `label`, looser leading. Kept because main's components
        // (NoMicrophoneNotice, PreferenceSettings) and setting-card.test.tsx use
        // `text-meta`; without the step those classes compile to nothing.
        meta: ['0.8125rem', { lineHeight: '1.25rem' }],
        // 12px is the floor for anything a person must read — every compliance string.
        caption: ['0.75rem', { lineHeight: '1rem', fontWeight: '400', letterSpacing: '0.005em' }],
        // 11px: an uppercase eyebrow, a count or an mm:ss. Never a sentence outside the
        // 380px copilot window, and never a compliance string.
        micro: ['0.6875rem', { lineHeight: '1rem', fontWeight: '500', letterSpacing: '0.01em' }],
        eyebrow: ['0.6875rem', { lineHeight: '1rem', fontWeight: '600', letterSpacing: '0.08em' }],
      },

      colors: {
        background: 'hsl(var(--background))',
        foreground: {
          DEFAULT: 'hsl(var(--foreground))',
          hover: 'hsl(var(--foreground-hover))',
          active: 'hsl(var(--foreground-active))',
        },
        'subtle-foreground': 'hsl(var(--subtle-foreground))',
        'surface-2': 'hsl(var(--surface-2))',
        'surface-3': 'hsl(var(--surface-3))',
        border: {
          DEFAULT: 'hsl(var(--border))',
          // DECORATIVE ONLY — never a control's sole boundary (§4.4.5).
          strong: 'hsl(var(--border-strong))',
        },
        input: {
          DEFAULT: 'hsl(var(--input))',
          hover: 'hsl(var(--input-hover))',
        },
        ring: 'hsl(var(--ring))',
        overlay: 'hsl(var(--overlay))',
        primary: {
          DEFAULT: 'hsl(var(--primary))',
          foreground: 'hsl(var(--primary-foreground))',
          ink: 'hsl(var(--primary-ink))',
          hover: 'hsl(var(--primary-hover))',
          active: 'hsl(var(--primary-active))',
        },
        secondary: {
          DEFAULT: 'hsl(var(--secondary))',
          foreground: 'hsl(var(--secondary-foreground))',
        },
        card: {
          DEFAULT: 'hsl(var(--card))',
          foreground: 'hsl(var(--card-foreground))',
        },
        popover: {
          DEFAULT: 'hsl(var(--popover))',
          foreground: 'hsl(var(--popover-foreground))',
        },
        muted: {
          DEFAULT: 'hsl(var(--muted))',
          foreground: 'hsl(var(--muted-foreground))',
        },
        accent: {
          DEFAULT: 'hsl(var(--accent))',
          foreground: 'hsl(var(--accent-foreground))',
        },
        // "A human decided" (§0.2 P1) — the only filled blue action in a viewport.
        verified: {
          DEFAULT: 'hsl(var(--verified))',
          foreground: 'hsl(var(--verified-foreground))',
          hover: 'hsl(var(--verified-hover))',
          active: 'hsl(var(--verified-active))',
          surface: 'hsl(var(--verified-surface))',
          ink: 'hsl(var(--verified-ink))',
          border: 'hsl(var(--verified-border))',
        },
        success: {
          DEFAULT: 'hsl(var(--success))',
          foreground: 'hsl(var(--success-foreground))',
          surface: 'hsl(var(--success-surface))',
          ink: 'hsl(var(--success-ink))',
          border: 'hsl(var(--success-border))',
        },
        warning: {
          DEFAULT: 'hsl(var(--warning))',
          foreground: 'hsl(var(--warning-foreground))',
          surface: 'hsl(var(--warning-surface))',
          ink: 'hsl(var(--warning-ink))',
          border: 'hsl(var(--warning-border))',
        },
        destructive: {
          DEFAULT: 'hsl(var(--destructive))',
          foreground: 'hsl(var(--destructive-foreground))',
          hover: 'hsl(var(--destructive-hover))',
          active: 'hsl(var(--destructive-active))',
          surface: 'hsl(var(--destructive-surface))',
          ink: 'hsl(var(--destructive-ink))',
          border: 'hsl(var(--destructive-border))',
        },
        info: {
          DEFAULT: 'hsl(var(--info))',
          foreground: 'hsl(var(--info-foreground))',
          surface: 'hsl(var(--info-surface))',
          ink: 'hsl(var(--info-ink))',
          border: 'hsl(var(--info-border))',
        },
        // The machine-draft register. Violet (256°), 220° away from warning amber — "this is
        // a machine draft" can never be read as "something is wrong".
        ai: {
          DEFAULT: 'hsl(var(--ai))',
          foreground: 'hsl(var(--ai-foreground))',
          surface: 'hsl(var(--ai-surface))',
          ink: 'hsl(var(--ai-ink))',
          border: 'hsl(var(--ai-border))',
        },
        recording: {
          DEFAULT: 'hsl(var(--recording))',
          // The record/stop label. NEVER --primary-foreground: white on the dark
          // --recording #EA473E is 3.84 and fails AA.
          foreground: 'hsl(var(--recording-foreground))',
          hover: 'hsl(var(--recording-hover))',
          active: 'hsl(var(--recording-active))',
          surface: 'hsl(var(--recording-surface))',
          ink: 'hsl(var(--recording-ink))',
        },
        meter: {
          DEFAULT: 'hsl(var(--meter))',
          hot: 'hsl(var(--meter-hot))',
          clip: 'hsl(var(--meter-clip))',
          track: 'hsl(var(--meter-track))',
        },
        sidebar: {
          DEFAULT: 'hsl(var(--sidebar))',
          foreground: 'hsl(var(--sidebar-foreground))',
          border: 'hsl(var(--sidebar-border))',
          accent: 'hsl(var(--sidebar-accent))',
          'accent-foreground': 'hsl(var(--sidebar-accent-foreground))',
          ring: 'hsl(var(--sidebar-ring))',
        },
        // Descriptive series — fills and strokes only, never small text (§4.4.7).
        chart: {
          1: 'hsl(var(--chart-1))',
          2: 'hsl(var(--chart-2))',
          3: 'hsl(var(--chart-3))',
          4: 'hsl(var(--chart-4))',
          5: 'hsl(var(--chart-5))',
          6: 'hsl(var(--chart-6))',
        },
      },

      // §4.7 — mapped all the way up, so a --radius change moves every corner. sm/md/lg
      // still compute to 6/8/10px: ~290 existing `rounded-*` usages move zero pixels.
      borderRadius: {
        xs: 'calc(var(--radius) - 4px)', //  4  chips, checkbox, tiny badges
        sm: 'calc(var(--radius) - 2px)', //  6  inputs, small buttons, row hover
        md: 'var(--radius)', //  8  buttons, cards, rows, toolbars
        lg: 'calc(var(--radius) + 2px)', // 10  section cards, stat tiles
        xl: 'calc(var(--radius) + 4px)', // 12  panels, dialogs, popovers, copilot
        '2xl': 'calc(var(--radius) + 8px)', // 16  onboarding hero, empty-state art only
      },

      // §4.7 — what the shared focus treatment actually needs. Tailwind draws the offset as a
      // box-shadow OUTSIDE the border box, so it must be the colour of whatever the control
      // sits ON, never the control's own fill (a fill-coloured offset paints a halo, not a
      // gap). Fifteen entries → twelve distinct surface values, each certified against --ring
      // at ≥5.32:1 in §4.4.6. This is what carries SC 2.4.11; the ESLint rule only stops the
      // most common way of forgetting it.
      ringOffsetColor: {
        background: 'hsl(var(--background))',
        card: 'hsl(var(--card))',
        popover: 'hsl(var(--popover))',
        sidebar: 'hsl(var(--sidebar))',
        muted: 'hsl(var(--muted))',
        'surface-2': 'hsl(var(--surface-2))',
        'surface-3': 'hsl(var(--surface-3))',
        accent: 'hsl(var(--accent))',
        'verified-surface': 'hsl(var(--verified-surface))',
        'success-surface': 'hsl(var(--success-surface))',
        'warning-surface': 'hsl(var(--warning-surface))',
        'destructive-surface': 'hsl(var(--destructive-surface))',
        'info-surface': 'hsl(var(--info-surface))',
        'ai-surface': 'hsl(var(--ai-surface))',
        'recording-surface': 'hsl(var(--recording-surface))',
      },

      // §4.8 — hairline-first. No component may use a raw Tailwind `shadow-*`.
      boxShadow: {
        'elev-0': 'var(--elev-0)',
        'elev-1': 'var(--elev-1)',
        'elev-2': 'var(--elev-2)',
        'elev-3': 'var(--elev-3)',
      },

      // §4.9
      transitionDuration: {
        instant: 'var(--dur-instant)',
        fast: 'var(--dur-fast)',
        base: 'var(--dur-base)',
        slow: 'var(--dur-slow)',
      },
      // `out` and `in-out` deliberately REDEFINE Tailwind's built-in `ease-out` /
      // `ease-in-out`, so the 31 `ease-out` and 4 `ease-in-out` sites already in the
      // tree adopt the design curve without being touched — one motion system rather
      // than two. `--ease-in-out` is byte-identical to Tailwind's default; `--ease-out`
      // is cubic-bezier(.2,.8,.2,1) against the default's (0,0,.2,1), i.e. it leaves
      // faster and settles longer. A call site that genuinely needs the CSS keyword
      // writes `ease-[cubic-bezier(0,0,.2,1)]` and says why.
      transitionTimingFunction: {
        out: 'var(--ease-out)',
        'in-out': 'var(--ease-in-out)',
        emphasis: 'var(--ease-emphasis)',
      },

      // §4.10 — so no component hand-writes `z-[95]` again.
      zIndex: {
        60: '60', //  dialog + sheet overlay
        70: '70', //  dialog + sheet content, evidence drawer
        80: '80', //  toasts (sonner)
        90: '90', //  full-screen drag-drop import overlay
        95: '95', //  first-run tour coach-mark + spotlight
        96: '96', //  tour popover
        100: '100', // onboarding full-screen shell (gates everything)
      },

      // §4.11 — the fixed-chrome contract, so a page body can end with
      // `pb-[calc(var(--bottom-chrome)+16px)]` through a named utility instead.
      spacing: {
        'bottom-chrome': 'var(--bottom-chrome)',
        rail: 'var(--rail-w)',
        pane: 'var(--pane-w)',
        header: 'var(--header-h)',
        dock: 'var(--dock-h)',
        reviewbar: 'var(--reviewbar-h)',
        gutter: 'var(--gutter)',
      },
      maxWidth: {
        measure: 'var(--measure)',
      },

      keyframes: {
        // §5.16. The indeterminate bar is a half-width fill, so a sweep has to travel from
        // -100% (fully off the left edge) to 200% (fully off the right) to cross the WHOLE
        // track: `slide-in-from-left-full` only moves it by its OWN width, i.e. half the
        // track, and left it parked mid-bar. Declared as a real keyframe rather than
        // composed from tailwindcss-animate utilities because an ARBITRARY `duration-`
        // value is ambiguous there — both core `transitionDuration` and the plugin's
        // `animationDuration` claim that namespace, so Tailwind emitted nothing for it and
        // the sweep silently ran at `.animate-in`'s built-in 150ms.
        'progress-indeterminate': {
          from: { transform: 'translateX(-100%)' },
          to: { transform: 'translateX(200%)' },
        },
        'accordion-down': {
          from: { height: '0' },
          to: { height: 'var(--radix-accordion-content-height)' },
        },
        'accordion-up': {
          from: { height: 'var(--radix-accordion-content-height)' },
          to: { height: '0' },
        },
      },
      animation: {
        'progress-indeterminate':
          'progress-indeterminate 1.4s var(--ease-in-out) infinite',
        'accordion-down': 'accordion-down 0.2s ease-out',
        'accordion-up': 'accordion-up 0.2s ease-out',
      },
    },
  },
  plugins: [require('tailwindcss-animate')],
};
