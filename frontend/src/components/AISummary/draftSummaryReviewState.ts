/**
 * One review lock for every summary-block mutation control.
 *
 * An approved summary is immutable in the current review surface: changing a
 * block would invalidate the whole-summary approval and its export provenance.
 * While an approval or another block write is in flight, serializing controls
 * also prevents the UI from presenting a state that has not won the backend's
 * optimistic-concurrency gate yet.
 */
export function areBlockReviewControlsLocked(
  summaryStatus: 'draft' | 'approved',
  isApprovingSummary: boolean,
  isBlockMutationPending: boolean,
): boolean {
  return (
    summaryStatus === 'approved' ||
    isApprovingSummary ||
    isBlockMutationPending
  );
}
