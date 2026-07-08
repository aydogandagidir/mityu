import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vitest/config';

/**
 * Vitest config for the `frontend/` unit tests.
 *
 * `environment: 'node'` — every suite matched by the `include` glob below is pure
 * logic. The heaviest ones (exportDocx/exportPdf) only need `Blob`, which Node
 * provides globally; no React component is rendered, so neither jsdom nor
 * `@vitejs/plugin-react` is needed.
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
  test: {
    environment: 'node',
    include: ['src/**/*.test.ts'],
  },
});
