'use client';

/**
 * /design — a Tauri-free living style guide + design-system verification surface.
 *
 * It renders the semantic design tokens (colors, typography, primitives) so the
 * "world-class UI" foundation can be reviewed and screenshotted in a plain browser
 * (the recording views need the native shell; this route deliberately does not).
 * Both themes are shown side by side: the right panel is wrapped in `.dark`, so a
 * single view proves light + dark token coherence at once.
 *
 * Not linked from product navigation; it exists for design review.
 */

const SWATCHES: { name: string; className: string; fg?: string }[] = [
  { name: 'background', className: 'bg-background', fg: 'text-foreground' },
  { name: 'card', className: 'bg-card', fg: 'text-card-foreground' },
  { name: 'primary', className: 'bg-primary', fg: 'text-primary-foreground' },
  { name: 'secondary', className: 'bg-secondary', fg: 'text-secondary-foreground' },
  { name: 'muted', className: 'bg-muted', fg: 'text-muted-foreground' },
  { name: 'accent', className: 'bg-accent', fg: 'text-accent-foreground' },
  { name: 'destructive', className: 'bg-destructive', fg: 'text-destructive-foreground' },
];

function Panel({ theme }: { theme: 'light' | 'dark' }) {
  return (
    <div className={theme === 'dark' ? 'dark' : ''}>
      <div className="bg-background text-foreground p-8 min-h-screen space-y-10">
        <header className="space-y-1">
          <div className="text-xs uppercase tracking-widest text-muted-foreground">
            {theme} theme
          </div>
          <h1 className="text-2xl font-semibold">Mityu design system</h1>
          <p className="text-[15px] text-muted-foreground">
            Brand-anchored tokens · bluedev #1E56FF · verify light + dark coherence.
          </p>
        </header>

        {/* Color tokens */}
        <section className="space-y-3">
          <h2 className="text-lg font-medium">Color tokens</h2>
          <div className="grid grid-cols-2 sm:grid-cols-4 gap-3">
            {SWATCHES.map((s) => (
              <div
                key={s.name}
                className={`${s.className} ${s.fg ?? ''} rounded-lg border border-border p-4 h-24 flex flex-col justify-between`}
              >
                <span className="text-sm font-medium">{s.name}</span>
                <span className="text-xs opacity-70">token</span>
              </div>
            ))}
          </div>
        </section>

        {/* Typography scale. Tier names are descriptive labels, NOT class names:
            the scale is Tailwind's own utilities. Each row prints the utilities it
            uses, so the label cannot drift from what the row renders. */}
        <section className="space-y-3">
          <h2 className="text-lg font-medium">Typography</h2>
          <div className="space-y-2">
            <p className="text-2xl font-semibold">Display — text-2xl font-semibold — 24 / 600</p>
            <p className="text-xl font-semibold">Heading 1 — text-xl font-semibold — 20 / 600</p>
            <p className="text-lg font-medium">Heading 2 — text-lg font-medium — 18 / 500</p>
            <p className="text-[15px]">Body — text-[15px] — 15 / 400. The quick brown fox jumps over the lazy dog.</p>
            <p className="text-sm text-muted-foreground">Small — text-sm — 14 / 400, muted.</p>
            <p className="text-xs text-muted-foreground">Caption — text-xs — 12 / 400, muted.</p>
          </div>
        </section>

        {/* Buttons */}
        <section className="space-y-3">
          <h2 className="text-lg font-medium">Buttons</h2>
          <div className="flex flex-wrap items-center gap-3">
            <button className="bg-primary text-primary-foreground rounded-lg px-4 py-2 text-sm font-medium shadow-sm hover:opacity-90 transition">
              Primary
            </button>
            <button className="bg-secondary text-secondary-foreground rounded-lg px-4 py-2 text-sm font-medium hover:bg-muted transition">
              Secondary
            </button>
            <button className="border border-border bg-background text-foreground rounded-lg px-4 py-2 text-sm font-medium hover:bg-muted transition">
              Outline
            </button>
            <button className="text-primary rounded-lg px-4 py-2 text-sm font-medium hover:bg-accent transition">
              Ghost
            </button>
            <button className="bg-destructive text-destructive-foreground rounded-lg px-4 py-2 text-sm font-medium hover:opacity-90 transition">
              Destructive
            </button>
          </div>
        </section>

        {/* Card + input + badges */}
        <section className="space-y-3">
          <h2 className="text-lg font-medium">Surfaces</h2>
          <div className="grid gap-4 sm:grid-cols-2">
            <div className="bg-card text-card-foreground rounded-xl border border-border p-5 shadow-sm space-y-3">
              <div className="flex items-center gap-2">
                <span className="inline-flex h-2.5 w-2.5 rounded-full bg-primary" />
                <h3 className="text-lg font-medium">Meeting summary</h3>
              </div>
              <p className="text-sm text-muted-foreground">
                Source-linked draft, awaiting your approval.
              </p>
              <div className="flex gap-2">
                <span className="text-xs rounded-full bg-accent text-accent-foreground px-2 py-0.5">
                  AI-generated
                </span>
                <span className="text-xs rounded-full border border-border px-2 py-0.5">
                  review required
                </span>
              </div>
            </div>
            <div className="bg-card text-card-foreground rounded-xl border border-border p-5 shadow-sm space-y-3">
              <label className="text-sm font-medium">Meeting title</label>
              <input
                className="w-full rounded-lg border border-input bg-background px-3 py-2 text-sm outline-none focus:ring-2 focus:ring-ring"
                placeholder="Q3 planning with Acme…"
              />
              <div className="flex items-center gap-2 text-xs text-muted-foreground">
                <span className="inline-flex h-2 w-2 rounded-full bg-destructive animate-pulse" />
                Recording · 02:44
              </div>
            </div>
          </div>
        </section>
      </div>
    </div>
  );
}

export default function DesignSystemPage() {
  return (
    <div className="grid grid-cols-1 lg:grid-cols-2 w-full">
      <Panel theme="light" />
      <Panel theme="dark" />
    </div>
  );
}
