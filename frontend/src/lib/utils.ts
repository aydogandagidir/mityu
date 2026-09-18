import { clsx, type ClassValue } from "clsx"
import { extendTailwindMerge } from "tailwind-merge"

/**
 * `cn()` has to be TAUGHT the tokens WP1 declared, or it silently deletes them.
 *
 * tailwind-merge resolves `text-*` by validator: a t-shirt size (`text-sm`) is a font-size,
 * and ANYTHING else is a colour. The eleven §4.5 steps are neither, so out of the box
 * `twMerge('text-label', 'text-muted-foreground')` returned exactly
 * `"text-muted-foreground"` — the type step was dropped, with no error, in every `cn()`
 * composition where a component sets a step and the call site sets a colour. That is the
 * failure mode ADR-0049 predicted ("a mis-grouped merge does not throw: it silently drops
 * or keeps the wrong class"), and the shared primitives are where `cn()` is densest.
 *
 * Registering the literal keys makes them exact matches, so they beat the colour group's
 * catch-all validator and stay mutually exclusive with `text-sm` and friends. The same
 * applies to the elevation, motion and measure tokens: `shadow-elev-2 shadow-none` and
 * `duration-instant duration-base` both survived as pairs, leaving stylesheet order — not
 * the call site — to decide which one won.
 */
/**
 * §4.11's `theme.extend.spacing` keys, shared by every spacing class group below. ADR-0053's
 * rule is that this registration is extended whenever `tailwind.config.js` gains a custom key
 * in an existing Tailwind class group — these are WP1's, and they were the ones left out.
 */
const CHROME_SPACING = [
  "bottom-chrome",
  "rail",
  "pane",
  "header",
  "dock",
  "reviewbar",
  "gutter",
] as const

const twMerge = extendTailwindMerge({
  extend: {
    classGroups: {
      // §4.5 type scale
      "font-size": [
        {
          text: [
            "display",
            "title-lg",
            "title",
            "title-sm",
            "read",
            "body",
            "label",
            "caption",
            "micro",
            "eyebrow",
          ],
        },
      ],
      // §4.8 elevation
      shadow: [{ shadow: ["elev-0", "elev-1", "elev-2", "elev-3"] }],
      // §4.9 motion
      duration: [{ duration: ["instant", "fast", "base", "slow"] }],
      // §4.11 reading measure
      "max-w": [{ "max-w": ["measure"] }],
      // §4.9 easing. tailwind-merge's `ease` group knows only `linear|in|out|in-out`, so
      // `cn('ease-emphasis', 'ease-out')` kept BOTH and left stylesheet order to decide.
      ease: [{ ease: ["emphasis"] }],
      // §4.11 the fixed-chrome spacing aliases. tailwind-merge's h/w/p*/m*/gap validators
      // accept lengths and t-shirt sizes only, so `h-header`, `w-rail`, `p-gutter` and the
      // rest were classified into NO group at all — `cn('h-header', 'h-9')` emitted both.
      // WP4 is the package that actually uses them, so they are registered before it lands.
      h: [{ h: CHROME_SPACING }],
      w: [{ w: CHROME_SPACING }],
      "min-w": [{ "min-w": CHROME_SPACING }],
      "max-h": [{ "max-h": CHROME_SPACING }],
      "min-h": [{ "min-h": CHROME_SPACING }],
      p: [{ p: CHROME_SPACING }],
      px: [{ px: CHROME_SPACING }],
      py: [{ py: CHROME_SPACING }],
      pt: [{ pt: CHROME_SPACING }],
      pr: [{ pr: CHROME_SPACING }],
      pb: [{ pb: CHROME_SPACING }],
      pl: [{ pl: CHROME_SPACING }],
      m: [{ m: CHROME_SPACING }],
      mx: [{ mx: CHROME_SPACING }],
      my: [{ my: CHROME_SPACING }],
      mt: [{ mt: CHROME_SPACING }],
      mr: [{ mr: CHROME_SPACING }],
      mb: [{ mb: CHROME_SPACING }],
      ml: [{ ml: CHROME_SPACING }],
      gap: [{ gap: CHROME_SPACING }],
      "gap-x": [{ "gap-x": CHROME_SPACING }],
      "gap-y": [{ "gap-y": CHROME_SPACING }],
      inset: [{ inset: CHROME_SPACING }],
      top: [{ top: CHROME_SPACING }],
      right: [{ right: CHROME_SPACING }],
      bottom: [{ bottom: CHROME_SPACING }],
      left: [{ left: CHROME_SPACING }],
    },
  },
})

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}

/**
 * Detects if an error message indicates that Ollama is not installed or not running
 * @param errorMessage - The error message to check
 * @returns true if the error indicates Ollama is not installed/running
 */
export function isOllamaNotInstalledError(errorMessage: string): boolean {
  if (!errorMessage) return false;

  const lowerError = errorMessage.toLowerCase();

  // Check for common patterns that indicate Ollama is not installed or not running
  const patterns = [
    'cannot connect',
    'connection refused',
    'cli not found',
    'not in path',
    'ollama cli not found',
    'not found or not in path',
    'please check if the server is running',
    'please check if the ollama server is running',
    'econnrefused',
  ];

  return patterns.some(pattern => lowerError.includes(pattern));
}
