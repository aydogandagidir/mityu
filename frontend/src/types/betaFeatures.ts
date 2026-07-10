/**
 * Beta Features Type System
 *
 * This file defines the scalable architecture for managing beta features.
 *
 * ## Adding a New Beta Feature
 * 1. Add property to BetaFeatures interface
 * 2. Add default value in DEFAULT_BETA_FEATURES
 * 3. Add analytics mapping in BETA_FEATURE_ANALYTICS_MAP
 * 4. Add UI strings in BETA_FEATURE_NAMES and BETA_FEATURE_DESCRIPTIONS
 * 5. Use in components: `betaFeatures.yourFeatureName`
 *
 * ## Graduating a Feature to Stable
 * 1. Remove property from BetaFeatures interface
 * 2. TypeScript will error at all usage sites
 * 3. Remove conditional checks - feature is now always-on
 */

export interface BetaFeatures {
  /**
   * Import audio files and retranscribe existing meetings with different language settings
   * @since v0.3.0
   */
  importAndRetranscribe: boolean;

  /**
   * Opt-in source-linked structured summaries (human review required).
   * When on, new summaries render as an editable draft where every block and
   * action item is bound to its source transcript segment and must be approved
   * by a human before it is "approved" (HITL, EU AI Act Art. 50). Draft
   * generation is requested via `api_process_transcript({ structured: true })`.
   * @since v0.4.0
   */
  structuredSummaries: boolean;
}

export const DEFAULT_BETA_FEATURES: BetaFeatures = {
  importAndRetranscribe: true, // Default: enabled
  // Default: ENABLED (2026-07-10, owner call). The structured draft is the surface
  // that satisfies CLAUDE.md §0.5 (every AI output is a source-linked draft behind
  // human approval) and it is the read.ai-style Report (docs/DESIGN_READAI.md).
  // The legacy BlockNote path has neither source links nor an approval gate, so it
  // should not be what new users land on. Users who explicitly saved a preference
  // keep it — loadBetaFeatures() merges saved values over these defaults.
  structuredSummaries: true,
};


/**
 * Human-readable feature names for UI display
 */
export const BETA_FEATURE_NAMES: Record<keyof BetaFeatures, string> = {
  importAndRetranscribe: 'Import Audio & Retranscribe',
  structuredSummaries: 'Source-Linked Structured Summaries',
};

/**
 * Feature descriptions for UI tooltips/help text
 */
export const BETA_FEATURE_DESCRIPTIONS: Record<keyof BetaFeatures, string> = {
  importAndRetranscribe: 'Import audio files to transcribe or retranscribe existing meetings with different language settings.',
  structuredSummaries: 'Opt-in source-linked structured summaries. Each summary block and action item is linked to its transcript segment and requires human review and approval before it is finalized.',
};

/**
 * Type-safe feature key union
 * This ensures only valid feature keys can be used
 */
export type BetaFeatureKey = keyof BetaFeatures;

/**
 * Version stamp persisted alongside saved preferences (as `_v`). Bump it to
 * re-apply a changed default ONCE to preferences saved under an older version;
 * any save after that carries the current version, so explicit user choices are
 * never overridden again. v2 = structuredSummaries re-defaulted to true.
 */
export const BETA_PREFS_VERSION = 2;

/**
 * Load beta features from localStorage
 *
 * @returns BetaFeatures object with values from localStorage or defaults
 */
export function loadBetaFeatures(): BetaFeatures {
  if (typeof window === 'undefined') {
    return { ...DEFAULT_BETA_FEATURES };
  }

  try {
    const saved = localStorage.getItem('betaFeatures');
    if (saved) {
      const parsed = JSON.parse(saved) as Partial<BetaFeatures> & { _v?: number };

      // One-time re-default (owner call, 2026-07-10): structuredSummaries became
      // the primary summary surface (source-linked HITL report — CLAUDE.md §0.5),
      // but preferences saved before that carry the old `false` default forever.
      // Bump stored prefs below version 2 to the new default ONCE; from then on
      // the user's explicit toggle (saved with _v: 2) always wins.
      if ((parsed._v ?? 1) < BETA_PREFS_VERSION) {
        const migrated: BetaFeatures = {
          ...DEFAULT_BETA_FEATURES,
          ...parsed,
          structuredSummaries: true,
        };
        saveBetaFeatures(migrated);
        return migrated;
      }

      // Merge with defaults to handle missing keys (graceful degradation)
      const { _v, ...rest } = parsed;
      return { ...DEFAULT_BETA_FEATURES, ...rest };
    }
  } catch (error) {
    console.error('[BetaFeatures] Failed to load from localStorage:', error);
  }

  return { ...DEFAULT_BETA_FEATURES };
}

/**
 * Save beta features to localStorage
 *
 * @param features - BetaFeatures object to save
 */
export function saveBetaFeatures(features: BetaFeatures): void {
  if (typeof window === 'undefined') return;

  try {
    // Stamp the prefs version so one-time default migrations (loadBetaFeatures)
    // never re-run over an explicit user choice.
    localStorage.setItem('betaFeatures', JSON.stringify({ ...features, _v: BETA_PREFS_VERSION }));
  } catch (error) {
    console.error('[BetaFeatures] Failed to save to localStorage:', error);
  }
}
