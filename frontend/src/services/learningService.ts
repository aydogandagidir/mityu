/**
 * Typed bindings for the learned-rule surface (ADR-0030 §9).
 *
 * Every method is a 1-to-1 wrapper around one Tauri command — no logic lives
 * here. The status machine, the birth policy and the scope filter are all Rust
 * side, where they are unit-tested; duplicating any of that judgement in TypeScript
 * would only create a second place for the two to disagree.
 *
 * Wire tokens are flat strings (`scope: "template:daily_standup"`), the same
 * closed vocabulary the database stores, pinned by tests on both sides.
 */

import { invoke } from '@tauri-apps/api/core';

/** Where a rule applies. `global` | `template:<id>` | `section:<title>`. */
export type RuleScopeToken = string;

/** A rule's lifecycle. Only `active` rules ever reach a prompt. */
export type RuleStatus = 'proposed' | 'active' | 'dismissed';

/**
 * Where a rule came from. Decides how the screen explains itself — and whether
 * the rule ever needed approval at all (`user_authored` rules are born active:
 * writing the rule IS the approval).
 */
export type RuleOrigin = 'mined_deterministic' | 'mined_llm' | 'user_authored';

/** What kind of preference a rule expresses. Advisory — `rule_text` carries the meaning. */
export type RuleKind = 'term_substitution' | 'style' | 'section_preference' | 'freeform';

/** One learned rule, as the screen sees it. */
export interface LearnedRule {
  /** Row id. */
  id: string;
  /** Scope token. */
  scope: RuleScopeToken;
  /** Kind token (advisory). */
  kind: RuleKind;
  /** The rule in plain language — user-editable, injected into prompts verbatim. */
  rule_text: string;
  /** Lifecycle. */
  status: RuleStatus;
  /** Provenance. */
  origin: RuleOrigin;
  /** How many corrections back it. */
  support_count: number;
}

/** One correction that backs a rule. */
export interface RuleEvidence {
  /** Correction-event id. */
  id: string;
  /** The meeting it happened in. */
  meeting_id: string;
  /** What the human did. */
  action: 'edit' | 'reject' | 'approve' | 'restore';
  /** What the model wrote. */
  original_text: string | null;
  /** What the human left behind. */
  final_text: string | null;
  /** Their rationale, if they gave one. */
  reason: string | null;
  /** When (RFC 3339). */
  created_at: string;
}

/** A rule's evidence, with the erased corrections counted rather than hidden. */
export interface RuleEvidenceResponse {
  /** The corrections that still exist, oldest first. */
  events: RuleEvidence[];
  /**
   * How many of this rule's corrections are gone — their meeting was deleted and
   * the cascade took them. EXPECTED, not an error: a rule outlives the meetings
   * it was mined from by design, so the UI reports it and moves on.
   */
  missing_count: number;
}

/**
 * The workspace's learning policy.
 *
 * NOTE: camelCase keys, matching the Rust `LearningConfig` serde rename. (The
 * older `RedactionConfig` uses snake_case; they differ because they were written
 * years apart, and matching each struct's own wire shape beats inventing a third.)
 */
export interface LearningConfig {
  /** Master switch: capture, mine, inject. Off ⇒ the app behaves as it did pre-ADR-0030. */
  enabled: boolean;
  /**
   * Whether a well-supported mined rule goes straight into force instead of
   * waiting for approval. On by default — bounded by the fact that every summary
   * still needs per-block human approval, every generation records the rules that
   * shaped it, and every rule is visible and deletable on this screen.
   */
  autoActivate: boolean;
  /** How many corrections a mined rule needs before auto-activation considers it. */
  autoActivateMinSupport: number;
  /**
   * Whether the LLM may be asked to mine subtler patterns. Off by default — not
   * for privacy (the transcript already goes to that provider) but because it
   * spends the user's own API budget.
   */
  llmMinerEnabled: boolean;
}

/** Burden over one set of reviewed items. */
export interface BurdenStats {
  /** How many items carried a verdict. */
  reviewed: number;
  /** How many the user took exactly as written. */
  accepted_as_written: number;
  /** Mean distance from what Mityu wrote to what the user kept, 0..1. */
  mean_burden: number;
}

/**
 * How much of Mityu's writing the user has had to change, and whether it moved.
 *
 * **A correlation, never a result.** The number measures what the USER did: a
 * person reviewing less carefully moves it the same direction a working rule
 * does, and nothing on either side of this boundary can tell those apart. Copy
 * that renders it must not say "because". See `learning::burden` (Rust) for the
 * full argument.
 */
export interface BurdenTrend {
  /** Everything ever reviewed. */
  overall: BurdenStats;
  /** The most recent window. `null` until there is enough history to compare. */
  recent: BurdenStats | null;
  /** The window before that. `null` on the same condition. */
  earlier: BurdenStats | null;
}

/** The numbers behind "is this working?". */
export interface LearningStats {
  /** Corrections recorded in this workspace, ever. */
  corrections_recorded: number;
  /** Rules currently in force. */
  rules_active: number;
  /** Rules waiting for a human. */
  rules_proposed: number;
  /** See {@link BurdenTrend} — and its warning. */
  burden: BurdenTrend;
}

class LearningService {
  /** Every live rule in the workspace, proposed first. */
  async listRules(): Promise<LearnedRule[]> {
    return invoke<LearnedRule[]>('api_list_learned_rules');
  }

  /** Corrections, rules and burden. */
  async getStats(): Promise<LearningStats> {
    return invoke<LearningStats>('api_get_learning_stats');
  }

  /**
   * The user writes their own rule. Born active — writing it is the approval.
   * @param scope defaults to `global` when omitted.
   * @returns the new rule's id.
   */
  async createRule(ruleText: string, scope?: RuleScopeToken): Promise<string> {
    return invoke<string>('api_create_learned_rule', { ruleText, scope });
  }

  /**
   * Put a rule in force (`proposed → active`, or `dismissed → active`).
   * @returns `false` for an illegal transition / unknown rule (soft no-op).
   */
  async activateRule(ruleId: string): Promise<boolean> {
    return invoke<boolean>('api_activate_learned_rule', { ruleId });
  }

  /**
   * The human says no, ON THE RECORD. Distinct from {@link deleteRule}: a
   * dismissed rule stays, which is what stops it being suggested again.
   * @returns `false` for an illegal transition / unknown rule (soft no-op).
   */
  async dismissRule(ruleId: string): Promise<boolean> {
    return invoke<boolean>('api_dismiss_learned_rule', { ruleId });
  }

  /**
   * The user rewrites a rule. Keeps its status, origin and evidence — they are
   * refining something they already agreed to, not re-proposing it.
   * @returns `false` for an unknown rule (soft no-op).
   */
  async editRule(ruleId: string, ruleText: string): Promise<boolean> {
    return invoke<boolean>('api_edit_learned_rule', { ruleId, ruleText });
  }

  /**
   * Remove a rule from the list. NOT "never suggest this again" — that is
   * {@link dismissRule}. A deleted rule carries no record of refusal, so the same
   * behaviour can be learned again if it repeats.
   * @returns `false` for an unknown rule (soft no-op).
   */
  async deleteRule(ruleId: string): Promise<boolean> {
    return invoke<boolean>('api_delete_learned_rule', { ruleId });
  }

  /** The corrections behind a rule — "why does Mityu think this?" */
  async getRuleEvidence(ruleId: string): Promise<RuleEvidenceResponse> {
    return invoke<RuleEvidenceResponse>('api_get_rule_evidence', { ruleId });
  }

  /** The workspace's learning policy. */
  async getConfig(): Promise<LearningConfig> {
    return invoke<LearningConfig>('api_get_learning_config');
  }

  /** Persist the workspace's learning policy. */
  async setConfig(config: LearningConfig): Promise<void> {
    return invoke<void>('api_set_learning_config', { config });
  }
}

export const learningService = new LearningService();
