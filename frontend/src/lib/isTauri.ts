/**
 * True when running inside the Tauri desktop shell (native APIs available).
 *
 * The desktop app is the product; this helper lets a handful of mount-time paths
 * degrade gracefully when the same Next.js bundle is rendered in a plain browser
 * (dev preview, design/style-guide route, Storybook-style isolation) where
 * `window.__TAURI_INTERNALS__` is absent and `invoke`/`listen` would throw. It is
 * NOT a feature gate for product behaviour — the app still assumes Tauri at runtime.
 */
export function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}
