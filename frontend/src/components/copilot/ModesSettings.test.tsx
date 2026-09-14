// @vitest-environment jsdom

/**
 * What Settings → Modes must not do (BACKLOG I4b).
 *
 * The guarantees here are all about *who decides*. Every rule about a mode —
 * the size cap, id and name collisions, ADR-0038's rejected category — lives in
 * `modes::validator`, and the danger is a well-meaning second copy appearing in
 * TypeScript that drifts from it. So these assert the component asks the
 * backend and renders its answer, rather than asserting any rule itself.
 */

import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import ModesSettings, { draftFrom, draftIsSaveable } from './ModesSettings';
import { modesService } from '@/services/modesService';
import type { Mode, ModeRow, ModesView } from '@/types/modes';

function mode(over: Partial<ModeRow> = {}): ModeRow {
  return {
    id: 'general',
    name: 'General',
    purpose: 'An ordinary working conversation.',
    userRole: 'a participant',
    counterpartRole: 'the other participants',
    voice: 'Plain and short.',
    summaryTemplateId: 'standard_meeting',
    live: { allowedActions: ['recap'], evidencePolicy: 'sourceFirst', citeRequired: true },
    allowedSources: ['transcript'],
    builtin: true,
    active: true,
    ...over,
  };
}

const CUSTOM = mode({
  id: 'site_walk',
  name: 'Site walk',
  purpose: 'A walk around a site with the client.',
  builtin: false,
  active: false,
});

function view(over: Partial<ModesView> = {}): ModesView {
  return { modes: [mode(), CUSTOM], activeModeId: 'general', ...over };
}

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

async function renderSettings(v: ModesView = view()) {
  vi.spyOn(modesService, 'list').mockResolvedValue(v);
  const r = render(<ModesSettings />);
  await screen.findByText('Site walk');
  return r;
}

describe('what may be changed', () => {
  /**
   * A user who could rewrite `General` could make the copilot's default
   * behaviour unexplainable to the next person who opens the app.
   */
  it('offers no edit or remove control for a built-in', async () => {
    await renderSettings();
    expect(screen.queryByRole('button', { name: /edit general/i })).toBeNull();
    expect(screen.queryByRole('button', { name: /remove general/i })).toBeNull();
  });

  it('offers both for a custom mode', async () => {
    await renderSettings();
    expect(screen.getByRole('button', { name: /edit site walk/i })).toBeTruthy();
    expect(screen.getByRole('button', { name: /remove site walk/i })).toBeTruthy();
  });

  /**
   * Widening a permission through a text box is an install, not an edit — so
   * the editor must not expose actions or sources at all.
   */
  it('the editor exposes wording only, never what the mode permits', async () => {
    await renderSettings();
    fireEvent.click(screen.getByRole('button', { name: /edit site walk/i }));
    expect(screen.getByLabelText(/^Name/)).toBeTruthy();
    expect(screen.getByLabelText(/^Purpose/)).toBeTruthy();
    expect(screen.getByLabelText(/^Voice/)).toBeTruthy();
    expect(screen.queryByLabelText(/action/i)).toBeNull();
    expect(screen.queryByLabelText(/source/i)).toBeNull();
  });

  it('sends only the three wording fields when saving', async () => {
    const update = vi.spyOn(modesService, 'update').mockResolvedValue(view());
    await renderSettings();
    fireEvent.click(screen.getByRole('button', { name: /edit site walk/i }));
    fireEvent.change(screen.getByLabelText(/^Name/), { target: { value: 'Site visit' } });
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));
    await waitFor(() =>
      expect(update).toHaveBeenCalledWith('site_walk', {
        name: 'Site visit',
        purpose: CUSTOM.purpose,
        voice: CUSTOM.voice,
      })
    );
  });
});

describe('choosing a mode', () => {
  it('does not offer "Use this" for the mode already answering', async () => {
    await renderSettings();
    const buttons = screen.getAllByRole('button', { name: 'Use this' });
    // Only the inactive one.
    expect(buttons).toHaveLength(1);
  });

  it('re-renders from the backend answer rather than its own guess', async () => {
    const after = view({
      modes: [mode({ active: false }), { ...CUSTOM, active: true }],
      activeModeId: 'site_walk',
    });
    vi.spyOn(modesService, 'setActive').mockResolvedValue(after);
    await renderSettings();
    fireEvent.click(screen.getByRole('button', { name: 'Use this' }));
    await waitFor(() => expect(screen.getAllByText('Active')).toHaveLength(1));
    expect(screen.getAllByRole('button', { name: 'Use this' })).toHaveLength(1);
  });
});

describe('importing a mode', () => {
  /**
   * The component validates nothing. It shows the backend's sentence, so the
   * wording of a refusal can never drift from the rule that produced it.
   */
  it('shows the backend refusal verbatim and installs nothing', async () => {
    const refusal =
      'A mode called "General" already exists. Two modes with one name cannot be told apart in the panel.';
    vi.spyOn(modesService, 'preview').mockRejectedValue(refusal);
    const install = vi.spyOn(modesService, 'install');
    await renderSettings();

    const file = new File(['{"id":"general"}'], 'mode.json', { type: 'application/json' });
    fireEvent.change(screen.getByLabelText(/choose a mode file/i), { target: { files: [file] } });

    const alert = await screen.findByRole('alert');
    expect(alert.textContent).toContain('already exists');
    expect(install).not.toHaveBeenCalled();
  });

  /** Preview first: nothing is written until the user has read what it is. */
  it('previews without installing, and installs only on confirmation', async () => {
    const incoming: Mode = {
      ...CUSTOM,
      id: 'client_walkthrough',
      name: 'Client walkthrough',
      purpose: 'Walking a client through a delivery.',
    };
    vi.spyOn(modesService, 'preview').mockResolvedValue(incoming);
    const install = vi.spyOn(modesService, 'install').mockResolvedValue(view());
    await renderSettings();

    const file = new File([JSON.stringify(incoming)], 'mode.json', { type: 'application/json' });
    fireEvent.change(screen.getByLabelText(/choose a mode file/i), { target: { files: [file] } });

    await screen.findByText('Client walkthrough');
    expect(install).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole('button', { name: 'Install' }));
    await waitFor(() => expect(install).toHaveBeenCalledOnce());
  });

  it('discarding a preview installs nothing', async () => {
    const incoming: Mode = { ...CUSTOM, id: 'x_mode', name: 'X mode' };
    vi.spyOn(modesService, 'preview').mockResolvedValue(incoming);
    const install = vi.spyOn(modesService, 'install');
    await renderSettings();

    const file = new File(['{}'], 'mode.json', { type: 'application/json' });
    fireEvent.change(screen.getByLabelText(/choose a mode file/i), { target: { files: [file] } });
    await screen.findByText('X mode');
    fireEvent.click(screen.getByRole('button', { name: 'Discard' }));

    expect(screen.queryByText('X mode')).toBeNull();
    expect(install).not.toHaveBeenCalled();
  });
});

describe('the save button is actually wired to the guard', () => {
  /**
   * Testing `draftIsSaveable` alone proved nothing about the button: the guard
   * could be unwired and every pure test would still pass. This asserts the
   * connection, which is the thing that can rot.
   */
  it('is disabled until something changes, and enabled after', async () => {
    await renderSettings();
    fireEvent.click(screen.getByRole('button', { name: /edit site walk/i }));
    const save = screen.getByRole('button', { name: 'Save' });
    expect(save.hasAttribute('disabled')).toBe(true);

    fireEvent.change(screen.getByLabelText(/^Name/), { target: { value: 'Site visit' } });
    expect(screen.getByRole('button', { name: 'Save' }).hasAttribute('disabled')).toBe(false);
  });

  it('closes again when a field is blanked', async () => {
    await renderSettings();
    fireEvent.click(screen.getByRole('button', { name: /edit site walk/i }));
    fireEvent.change(screen.getByLabelText(/^Voice/), { target: { value: '   ' } });
    expect(screen.getByRole('button', { name: 'Save' }).hasAttribute('disabled')).toBe(true);
  });
});

describe('the save guard', () => {
  const original = { name: 'A', purpose: 'B', voice: 'C' };

  it('is closed when nothing changed', () => {
    expect(draftIsSaveable({ ...original }, original)).toBe(false);
  });

  it('is closed when a field was blanked', () => {
    expect(draftIsSaveable({ ...original, purpose: '   ' }, original)).toBe(false);
  });

  it('is open for a real edit', () => {
    expect(draftIsSaveable({ ...original, voice: 'Terse.' }, original)).toBe(true);
  });

  it('takes its baseline from the mode itself', () => {
    expect(draftFrom(CUSTOM)).toEqual({
      name: CUSTOM.name,
      purpose: CUSTOM.purpose,
      voice: CUSTOM.voice,
    });
  });
});
