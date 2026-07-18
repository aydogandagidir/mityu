'use client';

/**
 * /design/hitl — a Tauri-free verification surface for the HITL review controls.
 *
 * Same purpose as `/design/tour`: exercise the REAL production components in a
 * plain browser, because gates passing is not evidence a feature works (the
 * 1.0.2 tour shipped green on tsc+lint and broken in the app).
 *
 * It renders the real `DraftSummaryView` with a fixture payload — the component
 * takes `draftResponse` as a prop, so no Tauri call is needed to mount it — which
 * makes the review controls, their local state and their keyboard handling all
 * real. What this surface proves: the reject-reason field (ADR-0024 §3) appears,
 * Enter submits it blank, Escape backs out.
 *
 * It deliberately does NOT stub `window.__TAURI_INTERNALS__`. Doing so makes
 * `isTauri()` true, which wakes every Tauri-gated provider in the root layout;
 * they then call into an IPC that is not there and the page dies white. (Learned
 * the hard way — the first version of this file did exactly that.) So `invoke`
 * rejects here, and confirming a reject shows the optimistic state, then the
 * revert and its error toast — which is itself the evidence that the whole chain
 * fired. The invoke PAYLOAD is pinned instead by `summaryDraftService.test.ts`,
 * which reads the arguments a mocked invoke received — a better instrument for
 * that question than a screenshot.
 *
 *   Preview URLs (static export or dev server on :3118):
 *     /design/hitl            → the review controls at rest
 *     /design/hitl?reject=1   → the reason field open on the first block
 *
 * `?reject=1` exists for the same reason `/design/tour` takes `?tour=N`: a
 * headless screenshot cannot click, so the page drives its own REAL Reject
 * button to reach the state worth photographing.
 *
 * Not linked from product navigation; it exists for verification.
 */

import { useEffect } from 'react';
import { DraftSummaryView } from '@/components/AISummary/DraftSummaryView';
import type { SummaryDraftResponse } from '@/services/summaryDraftService';

const MEETING_ID = 'preview-meeting';

const FIXTURE: SummaryDraftResponse = {
  status: 'draft',
  model: 'llama3.2',
  template_id: 'daily_standup',
  generated_at: '2026-07-16T10:00:00Z',
  approved_at: null,
  approved_by: null,
  draft: {
    meeting_id: MEETING_ID,
    status: 'draft',
    sections: [
      {
        title: 'Decisions',
        blocks: [
          {
            id: 'b1',
            type: 'bullet',
            content: 'Customer asked for a price revision on the conveyor line.',
            source_chunk_id: 'c-1',
            status: 'draft',
          },
          {
            id: 'b2',
            type: 'bullet',
            content: 'Three action items came out of the site visit.',
            source_chunk_id: 'c-2',
            status: 'draft',
          },
        ],
      },
    ],
  },
  action_items: [
    {
      id: 'a1',
      text: 'Send the revised quote to the customer.',
      status: 'draft',
      source_chunk_id: 'c-1',
    },
  ],
};

export default function HitlPreviewPage() {
  // `window.location` rather than `useSearchParams`, which would drag a Suspense
  // boundary into a page whose only job is to be screenshotted.
  useEffect(() => {
    if (!new URLSearchParams(window.location.search).has('reject')) return;
    const button = document.querySelector<HTMLButtonElement>(
      'button[aria-label="Reject block"]',
    );
    button?.click();
  }, []);

  return (
    <div className="bg-background text-foreground min-h-screen p-8 space-y-6">
      <header className="space-y-1">
        <h1 className="text-h2">HITL review controls</h1>
        <p className="text-small text-muted-foreground">
          Real components, fixture data, no IPC. Reject a block: the reason field is
          optional and Enter submits it blank.
        </p>
      </header>

      <div className="border border-border rounded-lg p-4">
        <DraftSummaryView
          meetingId={MEETING_ID}
          draftResponse={FIXTURE}
          meetingTitle="Site visit"
          meetingDate="16 July 2026"
        />
      </div>
    </div>
  );
}
