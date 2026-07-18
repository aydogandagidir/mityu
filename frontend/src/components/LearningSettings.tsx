'use client';

import { useCallback, useEffect, useState } from 'react';
import {
  AlertTriangle,
  Check,
  ChevronDown,
  ChevronRight,
  Loader2,
  Pencil,
  Plus,
  Sparkles,
  Trash2,
  X,
} from 'lucide-react';
import { Switch } from '@/components/ui/switch';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import {
  learningService,
  type BurdenStats,
  type LearnedRule,
  type LearningConfig,
  type LearningStats,
  type RuleEvidenceResponse,
} from '@/services/learningService';

/**
 * What Mityu has learned — the rules screen (ADR-0024 §9).
 *
 * This is not a nice-to-have panel. ADR-0024 §7 lets a well-supported mined rule
 * activate WITHOUT asking, and that is only defensible because of three bounds,
 * one of which is this screen: every rule visible, readable in plain language,
 * editable, and deletable. An opaque "our AI learns you" is exactly what cannot
 * be defended under the EU AI Act or KVKK — so the cure is that learning is data
 * the user can read, not weights they cannot.
 *
 * Load + optimistic-save mirror `RedactionSettings`.
 */

/**
 * The slice of `learningService` this screen uses.
 *
 * Named so the data source can be INJECTED — the `/design/learning` surface
 * passes a fixture-backed fake, because `invoke` throws outside the Tauri shell
 * and a screen that can only be photographed in its error state is a screen
 * nobody verified. One optional prop, defaulting to the real singleton, so no
 * production call site knows this exists.
 */
export type LearningDataSource = Pick<
  typeof learningService,
  | 'listRules'
  | 'createRule'
  | 'activateRule'
  | 'dismissRule'
  | 'editRule'
  | 'deleteRule'
  | 'getRuleEvidence'
  | 'getConfig'
  | 'setConfig'
  | 'getStats'
>;

/**
 * The headline: what Mityu has to show for itself.
 *
 * **The wording here is load-bearing.** The burden number is a correlation, not a
 * result — it measures what the USER did, and someone reviewing less carefully
 * moves it exactly the way a working rule does. So this says what happened
 * ("you've taken 14 of the last 20 as written; before that, 8 of 20") and never
 * why. The rules sit right below it and the reader can join the two themselves,
 * which is the most this data honestly supports. Do not add "thanks to your
 * rules" here — see `learning::burden` for the whole argument.
 */
function LearningHeadline({ stats }: { stats: LearningStats }) {
  const { burden } = stats;
  const rate = (s: BurdenStats) => Math.round((s.accepted_as_written / s.reviewed) * 100);

  if (burden.overall.reviewed === 0) {
    return (
      <p className="text-sm text-muted-foreground">
        Nothing to measure yet — review a summary and Mityu starts keeping score of how much
        you had to change.
      </p>
    );
  }

  return (
    <div className="rounded-lg border border-border bg-muted/40 p-3 space-y-1">
      <p className="text-sm text-foreground">
        You&apos;ve reviewed{' '}
        <span className="font-semibold">{burden.overall.reviewed}</span>{' '}
        {burden.overall.reviewed === 1 ? 'point' : 'points'} and taken{' '}
        <span className="font-semibold">{rate(burden.overall)}%</span> of them exactly as Mityu
        wrote them.
      </p>

      {burden.recent && burden.earlier && (
        <p className="text-sm text-muted-foreground">
          Your last {burden.recent.reviewed}: {rate(burden.recent)}% kept as written. The{' '}
          {burden.earlier.reviewed} before those: {rate(burden.earlier)}%.
        </p>
      )}

      <p className="text-xs text-muted-foreground">
        {stats.rules_active} {stats.rules_active === 1 ? 'rule is' : 'rules are'} in force, learned
        from {stats.corrections_recorded}{' '}
        {stats.corrections_recorded === 1 ? 'correction' : 'corrections'}.
      </p>
    </div>
  );
}

/** How each origin explains itself. The user never sees the raw token. */
const ORIGIN_LABEL: Record<LearnedRule['origin'], string> = {
  mined_deterministic: 'Learned from your corrections',
  mined_llm: 'Suggested by the model',
  user_authored: 'Written by you',
};

/** Turns a scope token into something readable. */
function describeScope(scope: string): string {
  if (scope === 'global') return 'Every summary';
  const [head, ...rest] = scope.split(':');
  const value = rest.join(':');
  if (head === 'template') return `Template: ${value}`;
  if (head === 'section') return `Section: ${value}`;
  return scope;
}

function RuleEvidencePanel({
  ruleId,
  service,
}: {
  ruleId: string;
  service: LearningDataSource;
}) {
  const [evidence, setEvidence] = useState<RuleEvidenceResponse | null>(null);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    let cancelled = false;
    service
      .getRuleEvidence(ruleId)
      .then((loaded) => !cancelled && setEvidence(loaded))
      .catch(() => !cancelled && setFailed(true));
    return () => {
      cancelled = true;
    };
  }, [ruleId, service]);

  if (failed) {
    return <p className="text-xs text-muted-foreground">Couldn&apos;t load the evidence.</p>;
  }
  if (!evidence) {
    return <Loader2 className="h-3.5 w-3.5 animate-spin text-muted-foreground" />;
  }

  return (
    <div className="space-y-2">
      {evidence.events.map((event) => (
        <div key={event.id} className="rounded border border-border bg-muted/50 p-2 text-xs">
          {event.action === 'reject' ? (
            <p className="text-muted-foreground">
              You rejected: <span className="line-through">{event.original_text}</span>
              {event.reason && <span className="block mt-1">Because: {event.reason}</span>}
            </p>
          ) : (
            <p className="text-muted-foreground">
              <span className="line-through">{event.original_text}</span>
              {event.final_text && (
                <>
                  {' → '}
                  <span className="text-foreground">{event.final_text}</span>
                </>
              )}
            </p>
          )}
        </div>
      ))}

      {/* Dangling evidence is the EXPECTED end state of the erasure asymmetry: the
          rule is an abstraction you approved and survives; the meeting text behind
          it does not. Say so, rather than showing a shorter list and looking like
          a bug. */}
      {evidence.missing_count > 0 && (
        <p className="text-xs text-muted-foreground italic">
          {evidence.missing_count}{' '}
          {evidence.missing_count === 1 ? 'correction' : 'corrections'} behind this rule{' '}
          {evidence.missing_count === 1 ? 'has' : 'have'} been erased with{' '}
          {evidence.missing_count === 1 ? 'its' : 'their'} meeting. The rule itself carries no
          meeting text, so it stays until you remove it.
        </p>
      )}

      {evidence.events.length === 0 && evidence.missing_count === 0 && (
        <p className="text-xs text-muted-foreground italic">
          You wrote this rule yourself, so there are no corrections behind it.
        </p>
      )}
    </div>
  );
}

function RuleRow({
  rule,
  onActivate,
  onDismiss,
  onEdit,
  onDelete,
  isBusy,
  service,
}: {
  rule: LearnedRule;
  onActivate: () => void;
  onDismiss: () => void;
  onEdit: (text: string) => Promise<boolean>;
  onDelete: () => void;
  isBusy: boolean;
  service: LearningDataSource;
}) {
  const [isEditing, setIsEditing] = useState(false);
  const [draft, setDraft] = useState(rule.rule_text);
  const [showEvidence, setShowEvidence] = useState(false);
  const [isSaving, setIsSaving] = useState(false);

  const saveEdit = async () => {
    setIsSaving(true);
    try {
      if (await onEdit(draft)) setIsEditing(false);
    } finally {
      setIsSaving(false);
    }
  };

  return (
    <div className="rounded-lg border border-border p-3 space-y-2">
      <div className="flex items-start justify-between gap-3">
        <div className="flex-1 min-w-0 space-y-1">
          {isEditing ? (
            <Input
              value={draft}
              onChange={(e) => setDraft(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter') void saveEdit();
                if (e.key === 'Escape') {
                  setDraft(rule.rule_text);
                  setIsEditing(false);
                }
              }}
              aria-label="Rule text"
              autoFocus
            />
          ) : (
            <p
              className={`text-sm ${
                rule.status === 'dismissed' ? 'text-muted-foreground line-through' : 'text-foreground'
              }`}
            >
              {rule.rule_text}
            </p>
          )}
          <p className="text-xs text-muted-foreground">
            {describeScope(rule.scope)} · {ORIGIN_LABEL[rule.origin]}
            {rule.support_count > 0 && ` · seen ${rule.support_count}×`}
          </p>
        </div>

        <div className="flex-shrink-0 flex items-center gap-1">
          {isEditing ? (
            <>
              <Button variant="green" size="sm" onClick={saveEdit} disabled={isSaving}>
                {isSaving ? <Loader2 className="h-4 w-4 animate-spin" /> : <Check className="h-4 w-4" />}
              </Button>
              <Button
                variant="outline"
                size="sm"
                onClick={() => {
                  setDraft(rule.rule_text);
                  setIsEditing(false);
                }}
                disabled={isSaving}
                aria-label="Cancel edit"
              >
                <X className="h-4 w-4" />
              </Button>
            </>
          ) : (
            <>
              {rule.status === 'proposed' && (
                <Button variant="green" size="sm" onClick={onActivate} disabled={isBusy}>
                  Use it
                </Button>
              )}
              {rule.status === 'dismissed' && (
                <Button variant="outline" size="sm" onClick={onActivate} disabled={isBusy}>
                  Turn on
                </Button>
              )}
              {rule.status !== 'dismissed' && (
                <Button
                  variant="outline"
                  size="sm"
                  onClick={onDismiss}
                  disabled={isBusy}
                  title="Stop using this, and don't suggest it again"
                >
                  {rule.status === 'proposed' ? 'No thanks' : 'Turn off'}
                </Button>
              )}
              <Button
                variant="outline"
                size="sm"
                onClick={() => {
                  setDraft(rule.rule_text);
                  setIsEditing(true);
                }}
                disabled={isBusy}
                title="Reword this rule"
                aria-label="Edit rule"
              >
                <Pencil className="h-4 w-4" />
              </Button>
              <Button
                variant="outline"
                size="sm"
                onClick={onDelete}
                disabled={isBusy}
                title="Remove from this list. Mityu may learn it again if you keep making the same correction — use 'Turn off' to refuse it for good."
                aria-label="Delete rule"
              >
                <Trash2 className="h-4 w-4 text-red-600 dark:text-red-400" />
              </Button>
            </>
          )}
        </div>
      </div>

      <button
        type="button"
        onClick={() => setShowEvidence((v) => !v)}
        className="inline-flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground"
      >
        {showEvidence ? (
          <ChevronDown className="h-3.5 w-3.5" />
        ) : (
          <ChevronRight className="h-3.5 w-3.5" />
        )}
        Why Mityu thinks this
      </button>
      {showEvidence && <RuleEvidencePanel ruleId={rule.id} service={service} />}
    </div>
  );
}

export default function LearningSettings({
  service = learningService,
}: {
  /** Defaults to the real service; injected only by `/design/learning`. */
  service?: LearningDataSource;
} = {}) {
  const [config, setConfig] = useState<LearningConfig | null>(null);
  const [rules, setRules] = useState<LearnedRule[]>([]);
  const [stats, setStats] = useState<LearningStats | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [loadFailed, setLoadFailed] = useState(false);
  const [busyId, setBusyId] = useState<string | null>(null);
  const [newRule, setNewRule] = useState('');
  const [isAdding, setIsAdding] = useState(false);

  const reload = useCallback(async () => {
    const [loadedRules, loadedStats] = await Promise.all([
      service.listRules(),
      service.getStats(),
    ]);
    setRules(loadedRules);
    setStats(loadedStats);
  }, [service]);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const [loadedConfig, loadedRules, loadedStats] = await Promise.all([
          service.getConfig(),
          service.listRules(),
          service.getStats(),
        ]);
        if (!cancelled) {
          setConfig(loadedConfig);
          setRules(loadedRules);
          setStats(loadedStats);
        }
      } catch (error) {
        console.error('Failed to load learning settings:', error);
        if (!cancelled) setLoadFailed(true);
      } finally {
        if (!cancelled) setIsLoading(false);
      }
    })();
    return () => {
      cancelled = true;
    };
    // `service` is a stable singleton in production and a module constant in the
    // preview, so this re-runs on neither — but a caller who ever passes a fresh
    // object should get a fresh load rather than stale rules.
  }, [service]);

  const persistConfig = async (next: LearningConfig, previous: LearningConfig) => {
    setConfig(next); // optimistic
    try {
      await service.setConfig(next);
    } catch (error) {
      console.error('Failed to save learning settings:', error);
      setConfig(previous);
    }
  };

  const runOnRule = async (ruleId: string, action: () => Promise<boolean>) => {
    setBusyId(ruleId);
    try {
      await action();
      await reload();
    } catch (error) {
      console.error('Rule action failed:', error);
    } finally {
      setBusyId(null);
    }
  };

  const addRule = async () => {
    if (!newRule.trim()) return;
    setIsAdding(true);
    try {
      await service.createRule(newRule.trim());
      setNewRule('');
      await reload();
    } catch (error) {
      console.error('Failed to add rule:', error);
    } finally {
      setIsAdding(false);
    }
  };

  const proposed = rules.filter((r) => r.status === 'proposed');
  const active = rules.filter((r) => r.status === 'active');
  const dismissed = rules.filter((r) => r.status === 'dismissed');

  return (
    <div className="space-y-4">
      <div>
        <h3 className="text-base font-semibold text-foreground mb-2">What Mityu has learned</h3>
        <p className="text-sm text-muted-foreground mb-4">
          When you fix a summary, Mityu notices. It turns what you keep changing into plain-language
          rules and follows them next time. The rules live on this device, you can rewrite or delete
          any of them, and nothing is ever trained into a model — so removing a rule really removes
          it.
        </p>
      </div>

      {isLoading && (
        <div className="flex items-center gap-2 p-3 bg-muted rounded-lg border border-border">
          <Loader2 className="w-4 h-4 animate-spin text-muted-foreground" />
          <span className="text-sm text-muted-foreground">Loading...</span>
        </div>
      )}

      {!isLoading && (loadFailed || !config) && (
        <div className="flex items-start gap-2 p-3 bg-amber-50 dark:bg-amber-500/10 rounded-lg border border-amber-200 dark:border-amber-500/25">
          <AlertTriangle className="w-4 h-4 text-amber-600 dark:text-amber-400 mt-0.5 flex-shrink-0" />
          <p className="text-sm text-amber-700 dark:text-amber-300">
            Learning settings could not be loaded. Your other preferences are unaffected — close and
            reopen Settings to try again.
          </p>
        </div>
      )}

      {!isLoading && config && (
        <>
          {stats && <LearningHeadline stats={stats} />}

          <div className="flex items-center justify-between p-3 bg-muted rounded-lg border border-border">
            <div>
              <h4 className="font-semibold text-foreground">Learn from my corrections</h4>
              <p className="text-sm text-muted-foreground">
                Capture what you change, and apply what it learns.
              </p>
            </div>
            <Switch
              checked={config.enabled}
              onCheckedChange={(enabled) => persistConfig({ ...config, enabled }, config)}
              aria-label="Learn from my corrections"
            />
          </div>

          <div className="flex items-center justify-between p-3 rounded-lg border border-border">
            <div>
              <h4 className="font-semibold text-foreground">Use new rules automatically</h4>
              <p className="text-sm text-muted-foreground">
                After the same correction {config.autoActivateMinSupport} times, start following it
                without asking. Summaries still need your approval either way, and every rule shows
                up here.
              </p>
            </div>
            <Switch
              checked={config.autoActivate}
              disabled={!config.enabled}
              onCheckedChange={(autoActivate) => persistConfig({ ...config, autoActivate }, config)}
              aria-label="Use new rules automatically"
            />
          </div>

          <div className="flex items-center justify-between p-3 rounded-lg border border-border">
            <div>
              <h4 className="font-semibold text-foreground">Let the model look for patterns</h4>
              <p className="text-sm text-muted-foreground">
                Finds subtler habits than the built-in checks, each time you approve a summary. Free
                with a local model (Ollama); with a paid provider it spends tokens on every approval.
              </p>
            </div>
            <Switch
              checked={config.llmMinerEnabled}
              disabled={!config.enabled}
              onCheckedChange={(llmMinerEnabled) =>
                persistConfig({ ...config, llmMinerEnabled }, config)
              }
              aria-label="Let the model look for patterns"
            />
          </div>

          {proposed.length > 0 && (
            <section className="space-y-2">
              <h4 className="text-sm font-semibold text-foreground flex items-center gap-1.5">
                <Sparkles className="h-4 w-4 text-blue-500" />
                Mityu noticed something ({proposed.length})
              </h4>
              {proposed.map((rule) => (
                <RuleRow
                  key={rule.id}
                  rule={rule}
                  isBusy={busyId === rule.id}
                  onActivate={() => runOnRule(rule.id, () => service.activateRule(rule.id))}
                  onDismiss={() => runOnRule(rule.id, () => service.dismissRule(rule.id))}
                  onEdit={(text) => service.editRule(rule.id, text).then(async (ok) => {
                    if (ok) await reload();
                    return ok;
                  })}
                  onDelete={() => runOnRule(rule.id, () => service.deleteRule(rule.id))}
                  service={service}
                />
              ))}
            </section>
          )}

          <section className="space-y-2">
            <h4 className="text-sm font-semibold text-foreground">
              Rules in use ({active.length})
            </h4>
            {active.length === 0 ? (
              <p className="text-sm text-muted-foreground">
                Nothing yet. Keep reviewing summaries and Mityu will start spotting what you change —
                or write a rule yourself below.
              </p>
            ) : (
              active.map((rule) => (
                <RuleRow
                  key={rule.id}
                  rule={rule}
                  isBusy={busyId === rule.id}
                  onActivate={() => runOnRule(rule.id, () => service.activateRule(rule.id))}
                  onDismiss={() => runOnRule(rule.id, () => service.dismissRule(rule.id))}
                  onEdit={(text) => service.editRule(rule.id, text).then(async (ok) => {
                    if (ok) await reload();
                    return ok;
                  })}
                  onDelete={() => runOnRule(rule.id, () => service.deleteRule(rule.id))}
                  service={service}
                />
              ))
            )}
          </section>

          <div className="flex items-center gap-2">
            <Input
              value={newRule}
              onChange={(e) => setNewRule(e.target.value)}
              onKeyDown={(e) => e.key === 'Enter' && void addRule()}
              placeholder="Write your own rule — e.g. “Call follow-ups ‘takip’, not ‘aksiyon’.”"
              aria-label="Write your own rule"
              disabled={!config.enabled || isAdding}
            />
            <Button
              variant="outline"
              size="sm"
              onClick={addRule}
              disabled={!config.enabled || isAdding || !newRule.trim()}
            >
              {isAdding ? <Loader2 className="h-4 w-4 animate-spin" /> : <Plus className="h-4 w-4" />}
              <span className="hidden lg:inline">Add</span>
            </Button>
          </div>

          {dismissed.length > 0 && (
            <section className="space-y-2">
              <h4 className="text-sm font-semibold text-muted-foreground">
                Refused ({dismissed.length})
              </h4>
              <p className="text-xs text-muted-foreground">
                Mityu won&apos;t suggest these again. Deleting one instead lets it come back if you
                keep making the same correction.
              </p>
              {dismissed.map((rule) => (
                <RuleRow
                  key={rule.id}
                  rule={rule}
                  isBusy={busyId === rule.id}
                  onActivate={() => runOnRule(rule.id, () => service.activateRule(rule.id))}
                  onDismiss={() => runOnRule(rule.id, () => service.dismissRule(rule.id))}
                  onEdit={(text) => service.editRule(rule.id, text).then(async (ok) => {
                    if (ok) await reload();
                    return ok;
                  })}
                  onDelete={() => runOnRule(rule.id, () => service.deleteRule(rule.id))}
                  service={service}
                />
              ))}
            </section>
          )}
        </>
      )}
    </div>
  );
}
