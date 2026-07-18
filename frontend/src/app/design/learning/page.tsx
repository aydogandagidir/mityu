'use client';

/**
 * /design/learning — a Tauri-free verification surface for the rules screen.
 *
 * Same purpose as `/design/tour` and `/design/hitl`: exercise the REAL component
 * in a plain browser, because gates passing is not evidence a feature works.
 *
 * It renders the real `LearningSettings` with a fixture-backed data source
 * (`LearningDataSource` — the seam the component exposes for exactly this).
 * Deliberately NOT `window.__TAURI_INTERNALS__`: stubbing that makes `isTauri()`
 * true and wakes every Tauri-gated provider in the root layout, which then calls
 * into an IPC that is not there and the page dies white. Injecting one prop
 * avoids the whole problem.
 *
 * The fixtures cover the states that actually carry risk, not just the happy one:
 *   - a PROPOSED rule (the approval path auto-activation is supposed to bypass),
 *   - an ACTIVE mined rule with evidence,
 *   - a rule whose evidence is PARTLY ERASED — the expected end state of the
 *     erasure asymmetry, which must read as an explanation and not a bug,
 *   - a USER-AUTHORED rule, which has no evidence at all,
 *   - a DISMISSED rule.
 *
 *   Preview URLs (static export or dev server on :3118):
 *     /design/learning          → a workspace with history
 *     /design/learning?empty=1  → a brand-new one, which is what every customer
 *                                 sees FIRST and where the arithmetic is most
 *                                 likely to render "0%" or "NaN%" at someone who
 *                                 has simply not started yet
 *
 * Not linked from product navigation; it exists for verification.
 */

import { useEffect, useState } from 'react';
import LearningSettings, { type LearningDataSource } from '@/components/LearningSettings';
import type { LearnedRule, LearningConfig } from '@/services/learningService';

const RULES: LearnedRule[] = [
  {
    id: 'r-proposed',
    scope: 'global',
    kind: 'term_substitution',
    rule_text: 'Call follow-ups “takip”, never “aksiyon”.',
    status: 'proposed',
    origin: 'mined_deterministic',
    support_count: 3,
  },
  {
    id: 'r-active',
    scope: 'section:Risks',
    kind: 'style',
    rule_text: 'Keep bullets under 15 words.',
    status: 'active',
    origin: 'mined_deterministic',
    support_count: 5,
  },
  {
    id: 'r-erased',
    scope: 'template:daily_standup',
    kind: 'section_preference',
    rule_text: 'Do not invent owners for tasks nobody claimed.',
    status: 'active',
    origin: 'mined_llm',
    support_count: 4,
  },
  {
    id: 'r-authored',
    scope: 'global',
    kind: 'freeform',
    rule_text: 'Write decisions as a single sentence.',
    status: 'active',
    origin: 'user_authored',
    support_count: 0,
  },
  {
    id: 'r-dismissed',
    scope: 'global',
    kind: 'style',
    rule_text: 'Always add a “Next steps” heading.',
    status: 'dismissed',
    origin: 'mined_deterministic',
    support_count: 3,
  },
];

const CONFIG: LearningConfig = {
  enabled: true,
  autoActivate: true,
  autoActivateMinSupport: 3,
  llmMinerEnabled: false,
};

/**
 * In-memory stand-in for the IPC. Mutates the fixture so the actions are real.
 *
 * `empty` gives the state EVERY new customer sees first: no rules, nothing
 * reviewed. It is previewable on purpose — a rate over zero items is `None` in
 * Rust, and the whole risk of this screen is that the arithmetic renders "0%" or
 * "NaN%" to someone who has simply not started yet.
 */
function makeFixtureService(empty = false): LearningDataSource {
  let rules = empty ? [] : [...RULES];
  let config = { ...CONFIG };

  return {
    listRules: async () => rules,
    getConfig: async () => config,
    setConfig: async (next) => {
      config = next;
    },
    // A workspace with two full windows, so the comparison renders. The numbers
    // deliberately show an improvement — that is the case whose WORDING carries
    // risk, since the copy must report it without claiming the rules caused it.
    getStats: async () =>
      empty
        ? {
            corrections_recorded: 0,
            rules_active: 0,
            rules_proposed: 0,
            burden: {
              overall: { reviewed: 0, accepted_as_written: 0, mean_burden: 0 },
              recent: null,
              earlier: null,
            },
          }
        : {
            corrections_recorded: 47,
            rules_active: rules.filter((r) => r.status === 'active').length,
            rules_proposed: rules.filter((r) => r.status === 'proposed').length,
            burden: {
              overall: { reviewed: 62, accepted_as_written: 38, mean_burden: 0.31 },
              recent: { reviewed: 20, accepted_as_written: 14, mean_burden: 0.18 },
              earlier: { reviewed: 20, accepted_as_written: 8, mean_burden: 0.44 },
            },
          },
    createRule: async (ruleText) => {
      const id = `r-new-${rules.length}`;
      rules = [
        ...rules,
        {
          id,
          scope: 'global',
          kind: 'freeform',
          rule_text: ruleText,
          status: 'active',
          origin: 'user_authored',
          support_count: 0,
        },
      ];
      return id;
    },
    activateRule: async (ruleId) => {
      rules = rules.map((r) => (r.id === ruleId ? { ...r, status: 'active' } : r));
      return true;
    },
    dismissRule: async (ruleId) => {
      rules = rules.map((r) => (r.id === ruleId ? { ...r, status: 'dismissed' } : r));
      return true;
    },
    editRule: async (ruleId, ruleText) => {
      rules = rules.map((r) => (r.id === ruleId ? { ...r, rule_text: ruleText } : r));
      return true;
    },
    deleteRule: async (ruleId) => {
      rules = rules.filter((r) => r.id !== ruleId);
      return true;
    },
    getRuleEvidence: async (ruleId) => {
      if (ruleId === 'r-authored') return { events: [], missing_count: 0 };
      if (ruleId === 'r-erased') {
        // Every correction behind this one went with its meeting.
        return { events: [], missing_count: 4 };
      }
      return {
        events: [
          {
            id: 'e1',
            meeting_id: 'm1',
            action: 'edit',
            original_text: '3 aksiyon çıktı',
            final_text: '3 takip maddesi çıktı',
            reason: null,
            created_at: '2026-07-15T10:00:00Z',
          },
          {
            id: 'e2',
            meeting_id: 'm2',
            action: 'reject',
            original_text: 'Müşteri memnun görünüyordu',
            final_text: null,
            reason: 'bu bir karar değil, sohbetti',
            created_at: '2026-07-16T11:00:00Z',
          },
        ],
        // One of its three corrections has been erased with its meeting.
        missing_count: 1,
      };
    },
  };
}

const POPULATED = makeFixtureService();
const EMPTY = makeFixtureService(true);

export default function LearningPreviewPage() {
  // `window.location` rather than `useSearchParams`, which would drag a Suspense
  // boundary into a page whose only job is to be screenshotted.
  const [empty, setEmpty] = useState<boolean | null>(null);
  useEffect(() => {
    setEmpty(new URLSearchParams(window.location.search).has('empty'));
  }, []);
  if (empty === null) return null;

  return (
    <div className="bg-background text-foreground min-h-screen p-8">
      <div className="max-w-3xl mx-auto space-y-6">
        <header className="space-y-1">
          <h1 className="text-h2">Learning settings</h1>
          <p className="text-small text-muted-foreground">
            Real component, fixture data, no IPC. {empty ? 'A brand-new workspace.' : ''}
          </p>
        </header>
        <div className="bg-card rounded-lg border border-border p-6 shadow-sm">
          <LearningSettings service={empty ? EMPTY : POPULATED} />
        </div>
      </div>
    </div>
  );
}
