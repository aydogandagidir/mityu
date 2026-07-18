import { describe, expect, it } from 'vitest';

import { areBlockReviewControlsLocked } from './draftSummaryReviewState';

describe('summary block review lock', () => {
  it('locks block mutations after whole-summary approval', () => {
    expect(areBlockReviewControlsLocked('approved', false, false)).toBe(true);
  });

  it('locks block mutations while approval or another mutation is pending', () => {
    expect(areBlockReviewControlsLocked('draft', true, false)).toBe(true);
    expect(areBlockReviewControlsLocked('draft', false, true)).toBe(true);
  });

  it('allows review controls only for an idle draft', () => {
    expect(areBlockReviewControlsLocked('draft', false, false)).toBe(false);
  });
});
