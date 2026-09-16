import { Settings2, Mic, Database as DatabaseIcon, SparkleIcon, FlaskConical, KeyRound } from 'lucide-react';

/**
 * The settings tabs, in the order they are rendered.
 *
 * This lives outside `page.tsx` because a Next App Router page module may only
 * export `default` and a fixed set of route options — any other named export
 * fails the build's generated type check ("Property ... is not assignable to
 * type 'never'"). Keeping the tab list and its pure resolver here makes both
 * importable by tests without loosening that contract.
 */
export const TABS = [
  { value: 'general', label: 'General', icon: Settings2 },
  { value: 'recording', label: 'Recordings', icon: Mic },
  { value: 'Transcriptionmodels', label: 'Transcription', icon: DatabaseIcon },
  { value: 'summaryModels', label: 'Summary', icon: SparkleIcon },
  { value: 'beta', label: 'Beta', icon: FlaskConical },
  { value: 'license', label: 'License', icon: KeyRound },
] as const;

/**
 * The tab named in `?tab=`, or null to stay where we are.
 *
 * An unrecognised value must leave the user on General rather than selecting a
 * tab that does not exist and rendering an empty panel.
 */
export function resolveTabFromSearch(search: string): string | null {
  const requested = new URLSearchParams(search).get('tab');
  return requested && TABS.some((tab) => tab.value === requested) ? requested : null;
}
