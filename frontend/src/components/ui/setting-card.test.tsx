// @vitest-environment jsdom

/**
 * The density primitive (G4).
 *
 * The request was a less text-heavy interface. The danger in answering it is
 * that "less text" becomes "less honest" — this app carries statements about
 * what deleting a meeting cannot erase, what the AI produced, and what it has
 * not validated, and none of those may quietly disappear into a redesign.
 *
 * So the property under test is not that the card looks tidier. It is that
 * folding an explanation SHORTENS it rather than removing it: the text stays in
 * the document, the browser's own find-in-page can reach it, and it opens
 * without JavaScript.
 */

import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import { SettingCard } from './setting-card';

// This suite renders the same titles more than once; without an explicit
// cleanup the queries match across tests (the repo does not enable
// auto-cleanup globally).
afterEach(cleanup);

describe('what a folded explanation still guarantees', () => {
  it('keeps the text in the document when collapsed', () => {
    render(
      <SettingCard
        title="Where your data is stored"
        description="Everything stays on this computer."
        details="Copies outside Mityu can survive a deletion."
      />,
    );
    // Present, not merely rendered-when-open: a claim removed from the DOM is a
    // claim the user cannot find.
    expect(screen.getByText('Copies outside Mityu can survive a deletion.')).toBeTruthy();
  });

  it('is collapsed by default, so the density win is real', () => {
    const { container } = render(
      <SettingCard title="T" details="Long explanation." detailsLabel="More" />,
    );
    const disclosure = container.querySelector('details');
    expect(disclosure).toBeTruthy();
    expect(disclosure?.hasAttribute('open')).toBe(false);
  });

  it('uses a native disclosure, so it opens with no JavaScript', () => {
    const { container } = render(<SettingCard title="T" details="X" detailsLabel="More" />);
    expect(container.querySelector('details > summary')?.textContent).toBe('More');
  });

  it('names what is inside rather than saying only "Details"', () => {
    render(
      <SettingCard
        title="Where your data is stored"
        details="…"
        detailsLabel="What deleting a meeting does and does not erase"
      />,
    );
    expect(screen.getByText('What deleting a meeting does and does not erase')).toBeTruthy();
  });

  it('renders no disclosure at all when there is nothing to fold', () => {
    const { container } = render(<SettingCard title="Appearance" description="One line." />);
    expect(container.querySelector('details')).toBeNull();
  });
});

describe('rank', () => {
  /**
   * The cause of the wall-of-text was that explanation was typeset exactly as
   * loudly as the control it explained. The title and the description must not
   * share a size.
   */
  it('does not typeset the description as loudly as the title', () => {
    render(<SettingCard title="Notifications" description="Tell me when a meeting starts." />);
    const title = screen.getByText('Notifications');
    const description = screen.getByText('Tell me when a meeting starts.');
    expect(title.className).toContain('text-title');
    expect(description.className).toContain('text-meta');
    expect(title.className).not.toBe(description.className);
  });

  it('keeps the control beside the title rather than after the prose', () => {
    render(
      <SettingCard title="Appearance" description="One line." action={<button>Toggle</button>} />,
    );
    const action = screen.getByRole('button', { name: 'Toggle' });
    const title = screen.getByText('Appearance');
    // Same row: the action is not a descendant of the text column.
    expect(title.parentElement?.contains(action)).toBe(false);
  });
});
