import { useEffect, useState } from 'react';
import { Switch } from '@/components/ui/switch';
import { CheckCircle2, Info, Loader2, Users } from 'lucide-react';
import {
  readRecordingConsentState,
  setRecordingConsentAcknowledged,
  setRecordingConsentAlwaysAsk,
  type RecordingConsentState,
} from '@/lib/recordingConsent';

/**
 * Recording-consent settings card content (BACKLOG C5).
 *
 * Surfaces the multi-party consent guidance, the current acknowledgment state,
 * and a toggle that re-arms the pre-recording reminder so the user controls
 * whether the gate is one-time or shown before every recording. All state is
 * local (recording-consent.json); mirrors RedactionSettings' load/optimistic-save
 * shape.
 */
export default function RecordingConsentSettings() {
  const [state, setState] = useState<RecordingConsentState | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [isSaving, setIsSaving] = useState(false);

  useEffect(() => {
    let cancelled = false;

    const loadState = async () => {
      const loaded = await readRecordingConsentState();
      if (!cancelled) {
        setState(loaded);
        setIsLoading(false);
      }
    };

    loadState();
    return () => {
      cancelled = true;
    };
  }, []);

  // Persist the "ask before every recording" flag with an optimistic update.
  const handleToggleAlwaysAsk = async (alwaysAsk: boolean) => {
    if (!state) return;
    const previous = state;
    setState({ ...state, alwaysAsk });
    setIsSaving(true);
    try {
      await setRecordingConsentAlwaysAsk(alwaysAsk);
    } catch (error) {
      console.error('Failed to save recording consent preference:', error);
      setState(previous);
    } finally {
      setIsSaving(false);
    }
  };

  // Clear the one-time acknowledgment so the reminder returns on the next
  // recording (an explicit "re-arm the gate now" action).
  const handleResetAcknowledgment = async () => {
    if (!state) return;
    const previous = state;
    setState({ ...state, acknowledged: false });
    setIsSaving(true);
    try {
      await setRecordingConsentAcknowledged(false);
    } catch (error) {
      console.error('Failed to reset recording consent acknowledgment:', error);
      setState(previous);
    } finally {
      setIsSaving(false);
    }
  };

  return (
    <div className="space-y-4">
      <div>
        <h3 className="text-base font-semibold text-gray-800 mb-2">Recording Consent</h3>
        <p className="text-sm text-gray-600 mb-4">
          Before your first recording, Mityu reminds you that recording laws vary by
          jurisdiction and that you are responsible for ensuring all participants are
          informed or consent. This acknowledgment is stored locally on this device.
        </p>
      </div>

      <div className="flex items-start gap-3 rounded-lg border border-amber-300 bg-amber-50 p-3">
        <Users className="mt-0.5 h-5 w-5 flex-shrink-0 text-amber-600" aria-hidden="true" />
        <div className="text-sm text-amber-900">
          <p className="font-semibold">Multi-party consent</p>
          <p className="mt-1 text-amber-800">
            Many jurisdictions require every participant to be informed or to consent
            before a conversation is recorded. Make sure everyone in the
            meeting/conversation is aware, and obtain any consent required where you
            are, before you start.
          </p>
        </div>
      </div>

      {isLoading && (
        <div className="flex items-center gap-2 p-3 bg-gray-50 rounded-lg border border-gray-200">
          <Loader2 className="w-4 h-4 animate-spin text-gray-500" />
          <span className="text-sm text-gray-600">Loading recording consent settings...</span>
        </div>
      )}

      {!isLoading && state && (
        <>
          {/* Current acknowledgment state */}
          <div className="flex items-center justify-between p-3 bg-gray-50 rounded-lg border border-gray-200">
            <div className="flex items-start gap-2">
              <CheckCircle2
                className={`mt-0.5 h-5 w-5 flex-shrink-0 ${
                  state.acknowledged ? 'text-green-600' : 'text-gray-400'
                }`}
                aria-hidden="true"
              />
              <div>
                <h4 className="font-semibold text-gray-800">Acknowledgment</h4>
                <p className="text-sm text-gray-600">
                  {state.acknowledged
                    ? 'You have acknowledged the consent reminder on this device.'
                    : 'The consent reminder will be shown before your next recording.'}
                </p>
              </div>
            </div>
            {state.acknowledged && (
              <button
                type="button"
                onClick={handleResetAcknowledgment}
                disabled={isSaving}
                className="flex-shrink-0 ml-4 px-3 py-1.5 text-sm border border-gray-300 rounded-md hover:bg-gray-100 transition-colors disabled:cursor-not-allowed disabled:opacity-50"
              >
                Reset
              </button>
            )}
          </div>

          {/* Re-arm toggle: show the reminder before every recording */}
          <div className="flex items-center justify-between p-3 bg-gray-50 rounded-lg border border-gray-200">
            <div>
              <h4 className="font-semibold text-gray-800">
                Show the consent reminder before each recording
              </h4>
              <p className="text-sm text-gray-600">
                {isSaving
                  ? 'Updating...'
                  : 'When on, the reminder appears every time, even after you acknowledge it'}
              </p>
            </div>
            <div className="flex items-center gap-2 ml-4">
              {isSaving && <Loader2 className="w-4 h-4 animate-spin text-gray-500" />}
              <Switch
                checked={state.alwaysAsk}
                onCheckedChange={handleToggleAlwaysAsk}
                disabled={isSaving}
              />
            </div>
          </div>

          <div className="flex items-start gap-2 p-2 bg-blue-50 rounded border border-blue-200">
            <Info className="w-4 h-4 text-blue-600 mt-0.5 flex-shrink-0" />
            <div className="text-xs text-blue-700">
              <p>
                Mityu cannot verify consent for you. This reminder is guidance only and
                does not constitute legal advice; recording laws differ by country and
                region.
              </p>
            </div>
          </div>
        </>
      )}
    </div>
  );
}
