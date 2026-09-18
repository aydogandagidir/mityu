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
  // This app ships as a static export (next.config.ts `output: 'export'`), so the
  // prerendered HTML can only ever be the default section. Reading the URL while
  // rendering made the client's first render disagree with it, and React threw
  // "Hydration failed because the server rendered HTML didn't match the client" and
  // rebuilt the tree. The section still ended up correct, which is exactly why it
  // survived review — it was caught by reading the browser console, not by any test.
  //
  // A jsdom render cannot see this: it never performs the server render to disagree
  // with. So the guard is structural.
  //
  // WHY THESE TWO ASSERTIONS CHANGED SHAPE (they were not weakened): the page no
  // longer holds the active pane in `useState`. The redesign made each pane a URL —
  // `/settings?section=privacy` — so the value is derived from `useSearchParams()`
  // inside the page's Suspense boundary, which is how Next is meant to read a query
  // string in a static export. The old assertions grepped for
  // `useState<string>('general')` and for a non-empty list of `useState` initialisers;
  // against an implementation with no `useState` at all, both would pass or fail for
  // reasons unrelated to hydration. What the pair actually guarded is preserved below:
  // nothing reads the URL during render, and the default is the value the prerender
  // emits.
  const source = readFileSync(join(__dirname, 'page.tsx'), 'utf8');

  it('never reads the URL during render — only inside an effect', () => {
    // `?tab=` is translated to `?section=` for the tray's link, and that read is the
    // one direct `window.location` access in the file. It must sit inside an effect,
    // so it runs after hydration rather than during the first render.
    const firstEffect = source.indexOf('useEffect(');
    expect(firstEffect).toBeGreaterThan(-1);
    const beforeAnyEffect = source.slice(0, firstEffect);
    expect(beforeAnyEffect).not.toMatch(/window\./);

    const initialisers = source.match(/useState\([\s\S]*?\)\s*;/g) ?? [];
    for (const initialiser of initialisers) {
      expect(initialiser).not.toMatch(/window\./);
    }
  });

  it('starts at the section the prerender emits', () => {
    // The prerender has no query string, so the derivation must fall back to General
    // rather than to whatever the first entry of SECTIONS happens to be.
    expect(source).toMatch(/:\s*'general';/);
  });
});
