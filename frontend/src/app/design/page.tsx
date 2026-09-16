'use client';

/**
 * /design — the token foundation, rendered.
 *
 * This is the verification surface for DESIGN_SYSTEM.md §4: every colour token, all eleven
 * type steps, and the radius / elevation / motion / z scales, in light and `.dark` side by
 * side, so a single screenshot proves both themes at once. It is Tauri-free on purpose —
 * the product routes `invoke()` on mount and never render in a browser, which is why the
 * `/design/*` fixtures are what `tools/ui/shoot.py` (and CI) actually check.
 *
 * Each row prints the CSS variable it renders, so a label can never drift from its colour.
 * Not linked from product navigation.
 */

type Row = { v: string; cls: string; note?: string };

const SURFACES: Row[] = [
  { v: '--background', cls: 'bg-background', note: 'app ground · rail · pane' },
  { v: '--card', cls: 'bg-card', note: 'content pane · cards · rows' },
  { v: '--popover', cls: 'bg-popover', note: 'dropdown · tooltip' },
  { v: '--surface-2', cls: 'bg-surface-2', note: 'sunken well · track' },
  { v: '--surface-3', cls: 'bg-surface-3', note: 'pressed · selected' },
  { v: '--muted', cls: 'bg-muted', note: 'row hover' },
  { v: '--accent', cls: 'bg-accent', note: 'active nav · selected row' },
];

const INKS: Row[] = [
  { v: '--foreground', cls: 'text-foreground', note: 'min 15.58 / 12.71' },
  { v: '--muted-foreground', cls: 'text-muted-foreground', note: 'min 5.06 / 5.88' },
  { v: '--subtle-foreground', cls: 'text-subtle-foreground', note: 'min 4.63 / 4.67' },
  { v: '--primary-ink', cls: 'text-primary-ink', note: 'min 4.82 / 4.67' },
];

/**
 * Filled controls: idle → hover → active. The ratios are the measured label contrast for
 * THIS theme (§4.4.4) — there is no global "hover = darker" rule, which is exactly how the
 * old dark --primary-hover ended up at 3.82 under `Approve summary`.
 */
const FILLS: {
  v: string;
  label: string;
  steps: string[];
  fg: string;
  light: string;
  dark: string;
}[] = [
  {
    v: '--foreground (default / ink)',
    label: 'Ink',
    steps: ['bg-foreground', 'bg-foreground-hover', 'bg-foreground-active'],
    fg: 'text-background',
    light: '17.64 → 14.15 → 11.16',
    dark: '16.20 → 13.13 → 10.74',
  },
  {
    v: '--verified (a human decided)',
    label: 'Approve',
    steps: ['bg-verified', 'bg-verified-hover', 'bg-verified-active'],
    fg: 'text-verified-foreground',
    light: '5.43 → 6.66 → 8.53',
    dark: '4.88 → 5.43 → 5.85',
  },
  {
    v: '--destructive',
    label: 'Reject',
    steps: ['bg-destructive', 'bg-destructive-hover', 'bg-destructive-active'],
    fg: 'text-destructive-foreground',
    light: '4.83 → 5.96 → 7.44',
    dark: '6.02 → 5.09 → 5.63',
  },
  {
    v: '--recording',
    label: 'Record',
    steps: ['bg-recording', 'bg-recording-hover', 'bg-recording-active'],
    fg: 'text-recording-foreground',
    light: '4.83 → 5.96 → 7.44',
    dark: '4.74 → 5.60 → 6.55',
  },
];

/** Tint registers: x-surface + x-ink + x-border. Violet AI is 220° from amber warning. */
const REGISTERS: { v: string; word: string; cls: string }[] = [
  { v: '--verified-surface', word: 'Approved', cls: 'bg-verified-surface text-verified-ink border-verified-border' },
  { v: '--ai-surface', word: 'Draft', cls: 'bg-ai-surface text-ai-ink border-ai-border' },
  { v: '--success-surface', word: 'Saved', cls: 'bg-success-surface text-success-ink border-success-border' },
  { v: '--warning-surface', word: 'Check', cls: 'bg-warning-surface text-warning-ink border-warning-border' },
  { v: '--destructive-surface', word: 'Failed', cls: 'bg-destructive-surface text-destructive-ink border-destructive-border' },
  { v: '--info-surface', word: 'Note', cls: 'bg-info-surface text-info-ink border-info-border' },
  { v: '--recording-surface', word: 'Recording', cls: 'bg-recording-surface text-recording-ink border-destructive-border' },
];

const TYPE: { cls: string; name: string; spec: string }[] = [
  { cls: 'text-display', name: 'text-display', spec: '24/32 · 600' },
  { cls: 'text-title-lg', name: 'text-title-lg', spec: '20/28 · 600' },
  { cls: 'text-title', name: 'text-title', spec: '16/24 · 600' },
  { cls: 'text-title-sm', name: 'text-title-sm', spec: '14/20 · 600' },
  { cls: 'text-read', name: 'text-read', spec: '15/24 · 400' },
  { cls: 'text-body', name: 'text-body', spec: '14/22 · 400' },
  { cls: 'text-label', name: 'text-label', spec: '13/18 · 500' },
  { cls: 'text-caption', name: 'text-caption', spec: '12/16 · 400 — compliance floor' },
  { cls: 'text-micro', name: 'text-micro', spec: '11/16 · 500 — counts, mm:ss' },
  { cls: 'text-eyebrow uppercase', name: 'text-eyebrow', spec: '11/16 · 600 · 0.08em' },
  { cls: 'font-mono text-caption tabular-nums', name: 'font-mono', spec: '12/16 · tnum 00:42:07' },
];

const RADII = [
  { cls: 'rounded-xs', name: 'xs · 4' },
  { cls: 'rounded-sm', name: 'sm · 6' },
  { cls: 'rounded-md', name: 'md · 8' },
  { cls: 'rounded-lg', name: 'lg · 10' },
  { cls: 'rounded-xl', name: 'xl · 12' },
  { cls: 'rounded-2xl', name: '2xl · 16' },
];

const ELEV = [
  { cls: 'shadow-elev-0', name: '--elev-0', use: 'cards, rows — the default' },
  { cls: 'shadow-elev-1', name: '--elev-1', use: 'dropdown, tooltip' },
  { cls: 'shadow-elev-2', name: '--elev-2', use: 'dialog, sheet, toast' },
  { cls: 'shadow-elev-3', name: '--elev-3', use: 'copilot, coach-mark' },
];

const MOTION = [
  { v: '--dur-instant', d: '80ms', e: '--ease-out', use: 'hover / press colour' },
  { v: '--dur-fast', d: '140ms', e: '--ease-out', use: 'chips, switches, HITL reveal' },
  { v: '--dur-base', d: '200ms', e: '--ease-out', use: 'popover, dialog, tabs' },
  { v: '--dur-slow', d: '320ms', e: '--ease-emphasis', use: 'drawer, pane collapse' },
];

const ZSCALE = [
  ['10', 'sticky header · review bar'],
  ['30', 'rail + Meetings pane'],
  ['40', 'session dock'],
  ['50', 'popover · dropdown · tooltip'],
  ['60 / 70', 'dialog overlay / content'],
  ['80', 'toasts'],
  ['95 / 96', 'tour spotlight / popover'],
  ['100', 'onboarding shell'],
];

/**
 * The offset surfaces the shared focus treatment compiles against (§4.4.6). `ring` carries
 * the whole treatment, offset included, so the swatch can never claim an offset it does not
 * draw — the same reason WP2 exports it as one constant instead of hand-writing it per call.
 */
const FOCUS: { on: string; surface: string; ring: string; ratio: string }[] = [
  {
    on: 'ring-offset-card',
    surface: 'bg-card',
    ring: 'ring-2 ring-ring ring-offset-2 ring-offset-card',
    ratio: '6.61 / 6.41',
  },
  {
    on: 'ring-offset-surface-3',
    surface: 'bg-surface-3',
    ring: 'ring-2 ring-ring ring-offset-2 ring-offset-surface-3',
    ratio: '5.44 / 5.32',
  },
  {
    on: 'ring-offset-ai-surface',
    surface: 'bg-ai-surface',
    ring: 'ring-2 ring-ring ring-offset-2 ring-offset-ai-surface',
    ratio: '5.94 / 6.10',
  },
];

const CHARTS = ['bg-chart-1', 'bg-chart-2', 'bg-chart-3', 'bg-chart-4', 'bg-chart-5', 'bg-chart-6'];

function Section({ title, hint, children }: { title: string; hint?: string; children: React.ReactNode }) {
  return (
    <section className="space-y-3">
      <div className="flex items-baseline justify-between gap-3 border-b border-border pb-1.5">
        <h2 className="text-title">{title}</h2>
        {hint ? <span className="font-mono text-micro text-subtle-foreground">{hint}</span> : null}
      </div>
      {children}
    </section>
  );
}

function Panel({ theme }: { theme: 'light' | 'dark' }) {
  return (
    <div className={theme === 'dark' ? 'dark' : ''}>
      <div className="min-h-screen space-y-8 bg-background p-6 text-foreground">
        <header className="space-y-1">
          <div className="text-eyebrow uppercase text-subtle-foreground">{theme} theme</div>
          <h1 className="text-title-lg">Mityu design tokens</h1>
          <p className="text-body text-muted-foreground">
            §4 of the design system, rendered. Every row prints the CSS variable it draws.
          </p>
        </header>

        <Section title="Surfaces" hint="hsl(var(--x))">
          <div className="grid grid-cols-2 gap-2">
            {SURFACES.map((s) => (
              <div key={s.v} className={`${s.cls} rounded-md border border-border p-2.5`}>
                <div className="font-mono text-micro text-foreground">{s.v}</div>
                <div className="text-caption text-subtle-foreground">{s.note}</div>
              </div>
            ))}
          </div>
        </Section>

        <Section title="Text on surfaces" hint="threshold 4.5">
          <div className="rounded-lg border border-border bg-card p-3">
            {INKS.map((t) => (
              <div key={t.v} className="flex items-baseline justify-between gap-3 py-1">
                <span className={`${t.cls} text-body`}>The quick brown fox · {t.v}</span>
                <span className="font-mono text-micro text-subtle-foreground">{t.note}</span>
              </div>
            ))}
          </div>
        </Section>

        <Section title="Filled controls" hint="idle → hover → active">
          <div className="space-y-2">
            {FILLS.map((f) => (
              <div key={f.v} className="flex items-center gap-2">
                <div className="flex gap-1">
                  {f.steps.map((step) => (
                    <span
                      key={step}
                      className={`${step} ${f.fg} inline-flex h-8 min-w-[64px] items-center justify-center rounded-md px-2.5 text-label`}
                    >
                      {f.label}
                    </span>
                  ))}
                </div>
                <div className="min-w-0">
                  <div className="truncate font-mono text-micro text-foreground">{f.v}</div>
                  <div className="font-mono text-micro text-subtle-foreground">
                    {theme === 'dark' ? f.dark : f.light}
                  </div>
                </div>
              </div>
            ))}
          </div>
        </Section>

        <Section title="Registers" hint="AI violet ≠ warning amber">
          <div className="flex flex-wrap gap-1.5">
            {REGISTERS.map((r) => (
              <span
                key={r.v}
                className={`${r.cls} inline-flex items-center gap-1.5 rounded-full border px-2.5 py-1`}
              >
                <span className="text-label">{r.word}</span>
                <span className="font-mono text-micro opacity-80">{r.v}</span>
              </span>
            ))}
          </div>
        </Section>

        <Section title="Type scale" hint="Inter · 400 / 500 / 600">
          <div className="space-y-1.5 rounded-lg border border-border bg-card p-3">
            {TYPE.map((t) => (
              <div key={t.name} className="flex items-baseline justify-between gap-3">
                <span className={t.cls}>{t.name === 'font-mono' ? '00:42:07 — 1,284' : 'Evidence, not vibes'}</span>
                <span className="shrink-0 font-mono text-micro text-subtle-foreground">
                  {t.name} · {t.spec}
                </span>
              </div>
            ))}
          </div>
        </Section>

        <Section title="Focus ring" hint="--ring decoupled from --primary">
          <div className="space-y-2">
            <div className="flex flex-wrap gap-3">
              {FOCUS.map((f) => (
                <div key={f.on} className={`${f.surface} rounded-md p-1.5`}>
                  <span
                    className={`${f.ring} inline-flex h-9 items-center rounded-md bg-verified px-3 text-label text-verified-foreground`}
                  >
                    Approve
                  </span>
                  <div className="pt-1 font-mono text-micro text-subtle-foreground">
                    {f.on} · {f.ratio}
                  </div>
                </div>
              ))}
            </div>
            <p className="text-caption text-muted-foreground">
              The ring against the fill it surrounds is 1.22:1. What carries SC 2.4.11 is the 2px
              offset in the colour of the surface underneath — worst case 5.32:1.
            </p>
          </div>
        </Section>

        <Section title="Radius · elevation" hint="--radius 8px">
          <div className="flex flex-wrap gap-2">
            {RADII.map((r) => (
              <div
                key={r.cls}
                className={`${r.cls} flex h-14 w-20 flex-col items-center justify-center border border-border bg-card`}
              >
                <span className="font-mono text-micro text-foreground">{r.name}</span>
              </div>
            ))}
          </div>
          <div className="grid grid-cols-2 gap-2">
            {ELEV.map((e) => (
              <div key={e.cls} className={`${e.cls} rounded-lg border border-border bg-card p-2.5`}>
                <div className="font-mono text-micro text-foreground">{e.name}</div>
                <div className="text-caption text-subtle-foreground">{e.use}</div>
              </div>
            ))}
          </div>
        </Section>

        <Section title="Motion · z-index" hint="prefers-reduced-motion honoured">
          <div className="grid grid-cols-2 gap-3">
            <div className="rounded-lg border border-border bg-card p-3">
              {MOTION.map((m) => (
                <div key={m.v} className="flex items-baseline justify-between gap-2 py-0.5">
                  <span className="font-mono text-micro text-foreground">{m.v}</span>
                  <span className="font-mono text-micro text-subtle-foreground">{m.d}</span>
                </div>
              ))}
              <p className="pt-1 text-caption text-subtle-foreground">
                Easing: --ease-out · --ease-emphasis
              </p>
            </div>
            <div className="rounded-lg border border-border bg-card p-3">
              {ZSCALE.map(([z, use]) => (
                <div key={z} className="flex items-baseline justify-between gap-2 py-0.5">
                  <span className="font-mono text-micro text-foreground">z-{z}</span>
                  <span className="truncate text-caption text-subtle-foreground">{use}</span>
                </div>
              ))}
            </div>
          </div>
        </Section>

        <Section title="Descriptive series" hint="identity, never ranking">
          <div className="flex gap-1.5">
            {CHARTS.map((c, i) => (
              <div key={c} className="flex-1 space-y-1">
                <div className={`${c} h-8 rounded-sm`} />
                <div className="text-center font-mono text-micro text-subtle-foreground">{i + 1}</div>
              </div>
            ))}
          </div>
          <p className="text-caption text-muted-foreground">
            Fills and strokes only — no chart token ever carries small text.
          </p>
        </Section>
      </div>
    </div>
  );
}

export default function DesignSystemPage() {
  return (
    <div className="grid w-full grid-cols-1 lg:grid-cols-2">
      <Panel theme="light" />
      <Panel theme="dark" />
    </div>
  );
}
