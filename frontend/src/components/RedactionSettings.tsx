import { useEffect, useState } from 'react';
import { Switch } from '@/components/ui/switch';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { AlertTriangle, Info, Loader2, Plus, X } from 'lucide-react';
import { configService, type RedactionConfig } from '@/services/configService';

/**
 * Redaction settings card content (BACKLOG C6 follow-up).
 *
 * Per-workspace, opt-in PII/keyword redaction policy. All reads/writes go
 * through configService (api_get_redaction_config / api_set_redaction_config);
 * saves are immediate with optimistic updates, mirroring AnalyticsConsentSwitch.
 */
export default function RedactionSettings() {
  const [config, setConfig] = useState<RedactionConfig | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [loadFailed, setLoadFailed] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [newTerm, setNewTerm] = useState('');

  useEffect(() => {
    let cancelled = false;

    const loadConfig = async () => {
      try {
        const loaded = await configService.getRedactionConfig();
        if (!cancelled) {
          setConfig(loaded);
        }
      } catch (error) {
        console.error('Failed to load redaction settings:', error);
        if (!cancelled) {
          setLoadFailed(true);
        }
      } finally {
        if (!cancelled) {
          setIsLoading(false);
        }
      }
    };

    loadConfig();
    return () => {
      cancelled = true;
    };
  }, []);

  const persistConfig = async (next: RedactionConfig, previous: RedactionConfig) => {
    // Optimistic update - immediately update UI state
    setConfig(next);
    setIsSaving(true);

    try {
      // Always send the full config object (snake_case keys, see RedactionConfig)
      await configService.setRedactionConfig(next);
    } catch (error) {
      console.error('Failed to save redaction settings:', error);
      // Revert the optimistic update on error
      setConfig(previous);
    } finally {
      setIsSaving(false);
    }
  };

  const handleToggleEnabled = async (enabled: boolean) => {
    if (!config) return;
    await persistConfig({ ...config, enabled }, config);
  };

  const handleToggleDefaultPatterns = async (useDefaultPatterns: boolean) => {
    if (!config) return;
    await persistConfig({ ...config, use_default_patterns: useDefaultPatterns }, config);
  };

  const handleAddTerm = async () => {
    if (!config) return;

    const term = newTerm.trim();
    if (!term) return;

    setNewTerm('');

    // Case-insensitive dedupe: silently ignore terms that are already listed
    const alreadyExists = config.custom_terms.some(
      (existing) => existing.toLowerCase() === term.toLowerCase()
    );
    if (alreadyExists) return;

    await persistConfig({ ...config, custom_terms: [...config.custom_terms, term] }, config);
  };

  const handleRemoveTerm = async (term: string) => {
    if (!config) return;
    await persistConfig(
      { ...config, custom_terms: config.custom_terms.filter((existing) => existing !== term) },
      config
    );
  };

  const isEnabled = config?.enabled ?? false;
  // Sub-controls stay visible when the master switch is off, but are inert
  const subControlsDisabled = !isEnabled || isSaving;

  return (
    <div className="space-y-4">
      <div>
        <h3 className="text-base font-semibold text-gray-800 mb-2">Sensitive Data Redaction</h3>
        <p className="text-sm text-gray-600 mb-4">
          Off by default. When enabled, sensitive content is scrubbed from new transcripts on this
          device before it is stored or summarized. Settings apply to this workspace and are stored
          locally.
        </p>
      </div>

      {isLoading && (
        <div className="flex items-center gap-2 p-3 bg-gray-50 rounded-lg border border-gray-200">
          <Loader2 className="w-4 h-4 animate-spin text-gray-500" />
          <span className="text-sm text-gray-600">Loading redaction settings...</span>
        </div>
      )}

      {!isLoading && (loadFailed || !config) && (
        <div className="flex items-start gap-2 p-3 bg-amber-50 rounded-lg border border-amber-200">
          <AlertTriangle className="w-4 h-4 text-amber-600 mt-0.5 flex-shrink-0" />
          <p className="text-sm text-amber-700">
            Redaction settings could not be loaded. Your other preferences are unaffected — close
            and reopen Settings to try again.
          </p>
        </div>
      )}

      {!isLoading && config && (
        <>
          <div className="flex items-center justify-between p-3 bg-gray-50 rounded-lg border border-gray-200">
            <div>
              <h4 className="font-semibold text-gray-800">Redact sensitive content</h4>
              <p className="text-sm text-gray-600">
                {isSaving
                  ? 'Updating...'
                  : 'Scrub sensitive data from new transcripts before they are saved or summarized'}
              </p>
            </div>
            <div className="flex items-center gap-2 ml-4">
              {isSaving && <Loader2 className="w-4 h-4 animate-spin text-gray-500" />}
              <Switch
                checked={config.enabled}
                onCheckedChange={handleToggleEnabled}
                disabled={isSaving}
              />
            </div>
          </div>

          <div
            className={`flex items-center justify-between p-3 bg-gray-50 rounded-lg border border-gray-200 ${
              !isEnabled ? 'opacity-50' : ''
            }`}
          >
            <div>
              <h4 className="font-semibold text-gray-800">Built-in PII patterns</h4>
              <p className="text-sm text-gray-600">
                Replaces emails with [EMAIL], phone numbers with [PHONE], credit card numbers with
                [CARD], IBANs with [IBAN], and Turkish ID numbers (TC Kimlik No) with [ID]
              </p>
            </div>
            <div className="flex items-center gap-2 ml-4">
              <Switch
                checked={config.use_default_patterns}
                onCheckedChange={handleToggleDefaultPatterns}
                disabled={subControlsDisabled}
              />
            </div>
          </div>

          <div
            className={`p-3 bg-gray-50 rounded-lg border border-gray-200 ${
              !isEnabled ? 'opacity-50' : ''
            }`}
          >
            <h4 className="font-semibold text-gray-800">Custom terms</h4>
            <p className="text-sm text-gray-600 mb-3">
              Words or phrases to replace with [REDACTED], matched case-insensitively (for example
              project codenames or client names)
            </p>

            {config.custom_terms.length > 0 ? (
              <div className="flex flex-wrap gap-2 mb-3">
                {config.custom_terms.map((term) => (
                  <span
                    key={term}
                    className="inline-flex items-center gap-1 bg-white border border-gray-300 rounded-full pl-2.5 pr-1 py-0.5 text-xs text-gray-700"
                  >
                    {term}
                    <button
                      type="button"
                      onClick={() => handleRemoveTerm(term)}
                      disabled={subControlsDisabled}
                      className="p-0.5 rounded-full text-gray-400 hover:text-gray-700 hover:bg-gray-100 disabled:cursor-not-allowed disabled:hover:bg-transparent disabled:hover:text-gray-400"
                      aria-label={`Remove custom term ${term}`}
                      title="Remove term"
                    >
                      <X className="w-3 h-3" />
                    </button>
                  </span>
                ))}
              </div>
            ) : (
              <p className="text-xs text-gray-500 mb-3">No custom terms added yet</p>
            )}

            <div className="flex items-center gap-2">
              <Input
                value={newTerm}
                onChange={(e) => setNewTerm(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === 'Enter') {
                    e.preventDefault();
                    handleAddTerm();
                  }
                }}
                placeholder="Add a term to redact..."
                disabled={subControlsDisabled}
                className="h-8 bg-white text-sm"
              />
              <Button
                onClick={handleAddTerm}
                variant="outline"
                size="sm"
                disabled={subControlsDisabled || newTerm.trim() === ''}
                className="flex-shrink-0"
              >
                <Plus className="w-3.5 h-3.5" />
                <span>Add</span>
              </Button>
            </div>
          </div>

          <div className="flex items-start gap-2 p-2 bg-blue-50 rounded border border-blue-200">
            <Info className="w-4 h-4 text-blue-600 mt-0.5 flex-shrink-0" />
            <div className="text-xs text-blue-700">
              <p className="mb-1">
                Redaction applies on this device before saving and before any summary provider
                (including cloud providers) sees the text.
              </p>
              <p>
                Existing saved transcripts are not changed retroactively — only new transcripts are
                redacted.
              </p>
            </div>
          </div>
        </>
      )}
    </div>
  );
}
