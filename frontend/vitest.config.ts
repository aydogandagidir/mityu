import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vitest/config';

/**
 * Vitest config for the `frontend/` unit tests.
 *
 * `environment: 'node'` stays the DEFAULT: almost every suite is pure logic and
 * the heaviest ones (exportDocx/exportPdf) only need `Blob`, which Node provides
 * globally. A suite that genuinely renders React opts in per file with a
 * `@vitest-environment jsdom` docblock, so one component test does not slow the
 * whole suite down or pull a DOM into pure-logic tests.
 *
 * `oxc.jsx.runtime: 'automatic'` is required because `tsconfig.json` sets
 * `"jsx": "preserve"` for Next's own compiler, so the transformer has to be told
 * to emit the React 17+ automatic runtime instead. This Vitest runs on
 * Vite 8 / Rolldown, whose transformer is **oxc** — setting `esbuild.jsx` here is
 * silently ignored (Vite logs "oxc options will be used").
 *
 * The `@/*` alias mirrors `tsconfig.json`'s `compilerOptions.paths` so tests can
 * import product code the same way the app does.
 */
export default defineConfig({
  resolve: {
    alias: {
      '@': fileURLToPath(new URL('./src', import.meta.url)),
    },
  },
  oxc: {
    jsx: {
      runtime: 'automatic',
    },
  },
  test: {
    environment: 'node',
    include: ['src/**/*.test.ts', 'src/**/*.test.tsx'],
  },
});
