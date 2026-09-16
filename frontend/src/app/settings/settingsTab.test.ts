import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { resolveTabFromSearch } from './tabs';

describe('resolveTabFromSearch', () => {
  it('returns the tab the tray asked for', () => {
    // The tray's "Live copilot" entry routes to /settings?tab=beta when the
    // copilot is off; landing on General would leave the user hunting for the
    // switch, which is the whole reason the entry exists.
    expect(resolveTabFromSearch('?tab=beta')).toBe('beta');
  });

  it('accepts every tab the page actually renders', () => {
    for (const tab of ['general', 'recording', 'Transcriptionmodels', 'summaryModels', 'beta', 'license']) {
      expect(resolveTabFromSearch(`?tab=${tab}`)).toBe(tab);
    }
  });

  it('stays put for a tab that does not exist, rather than emptying the panel', () => {
    expect(resolveTabFromSearch('?tab=copilot')).toBeNull();
    expect(resolveTabFromSearch('?tab=')).toBeNull();
  });

  it('stays put when no tab is named', () => {
    expect(resolveTabFromSearch('')).toBeNull();
    expect(resolveTabFromSearch('?other=1')).toBeNull();
  });
});

describe('the settings page hydrates identically on both sides', () => {
  // This app ships as a static export (next.config.ts `output: 'export'`), so
  // the prerendered HTML can only ever be the default tab. A `useState`
  // initialiser that read `window.location.search` made the client's first
  // render disagree with it, and React threw "Hydration failed because the
  // server rendered HTML didn't match the client" and rebuilt the tree. The tab
  // still ended up correct, which is exactly why it survived review — it was
  // caught by reading the browser console, not by any test.
  //
  // A jsdom render cannot see this: it never performs the server render to
  // disagree with. So the guard is structural — no state initialiser may read
  // the URL. Keep the read in an effect, which runs only after hydration.
  const source = readFileSync(join(__dirname, 'page.tsx'), 'utf8');

  it('never reads window inside a useState initialiser', () => {
    const initialisers = source.match(/useState\([\s\S]*?\)\s*;/g) ?? [];
    expect(initialisers.length).toBeGreaterThan(0);
    for (const initialiser of initialisers) {
      expect(initialiser).not.toMatch(/window\./);
    }
  });

  it('starts the tab at the value the prerender emits', () => {
    expect(source).toMatch(/useState<string>\('general'\)/);
  });
});
