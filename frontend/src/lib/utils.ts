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
