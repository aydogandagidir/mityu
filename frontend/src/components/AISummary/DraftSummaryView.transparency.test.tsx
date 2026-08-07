/**
 * @vitest-environment jsdom
 *
 * EU AI Act Art. 50 transparency guard.
 *
 * The "AI-generated · review required" banner is a compliance affordance, not a
 * design flourish: Art. 50 obliges the marking of AI-generated output, and the
 * obligation is CONTINUOUS, so the protection has to be executable rather than
 * a comment. Before this suite existed, deleting `<ReviewRequiredBanner />` from
 * any branch of `DraftSummaryView` left every test green.
 *
 * What is pinned here:
 *  - the banner renders in EVERY reachable state of the draft surface — loading,
 *    error, empty, and a populated draft — so no path shows model output unlabelled;
 *  - it is not dismissable: no button/control inside it can hide it.
 *
 * Deliberately NOT pinned: wording, colours, or icon. Those may change; the
 * presence of an accessible AI-generated marking may not. The banner is queried
 * through its ARIA role + label, which is also what a screen-reader user gets.
 *
 * This file opts into jsdom per-file (see the docblock above); the suite default
 * stays `environment: 'node'`.
 */

import { cleanup, render, screen, within } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';

import { DraftSummaryView } from './DraftSummaryView';
import type { SummaryDraftResponse } from '@/services/summaryDraftService';

afterEach(cleanup);

/** The accessible marking, as exposed to assistive tech. */
const BANNER = { role: 'note', name: /AI-generated content, human review required/i };

const POPULATED: SummaryDraftResponse = {
  draft: {
    meeting_id: 'm1',
    status: 'draft',
    sections: [
      {
        title: 'Key points',
        blocks: [
          {
            id: 'b1',
            type: 'bullet',
            content: 'The vendor contract renews in March.',
            source_chunk_id: 'chunk-1',
            status: 'draft',
          },
        ],
      },
    ],
  },
  status: 'draft',
  model: 'test-model',
  template_id: null,
  generated_at: '2026-07-26T10:00:00Z',
  approved_at: null,
  approved_by: null,
  action_items: [
    {
      id: 'a1',
      text: 'Send the renewal notice.',
      status: 'draft',
      source_chunk_id: 'chunk-1',
    },
  ],
};

describe('Art. 50 — the AI-generated marking is always present', () => {
  it('labels the loading state', () => {
    render(<DraftSummaryView meetingId="m1" draftResponse={null} isLoading />);
    expect(screen.getByRole(BANNER.role, { name: BANNER.name })).toBeDefined();
  });

  it('labels the error state', () => {
    render(
      <DraftSummaryView
        meetingId="m1"
        draftResponse={null}
        error="Couldn't load the draft"
      />,
    );
    expect(screen.getByRole(BANNER.role, { name: BANNER.name })).toBeDefined();
  });

  it('labels the empty state', () => {
    render(<DraftSummaryView meetingId="m1" draftResponse={null} />);
    expect(screen.getByRole(BANNER.role, { name: BANNER.name })).toBeDefined();
  });

  it('labels a populated draft, and the model text really is on screen', () => {
    render(<DraftSummaryView meetingId="m1" draftResponse={POPULATED} />);

    // Guards the guard: if the draft failed to render, a passing banner
    // assertion would prove nothing about labelling actual model output.
    expect(screen.getByText(/vendor contract renews in March/i)).toBeDefined();
    expect(screen.getByRole(BANNER.role, { name: BANNER.name })).toBeDefined();
  });

  it('cannot be dismissed — the banner contains no control that hides it', () => {
    render(<DraftSummaryView meetingId="m1" draftResponse={POPULATED} />);

    const banner = screen.getByRole(BANNER.role, { name: BANNER.name });
    expect(within(banner).queryAllByRole('button')).toHaveLength(0);
    expect(banner.querySelectorAll('button, [role="button"], input')).toHaveLength(0);
  });
});
