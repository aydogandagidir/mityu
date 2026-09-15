// @vitest-environment jsdom

/**
 * The About dialog is where the desktop app describes itself, and it must say
 * the same things the landing page says (`landing/index.html`, whose side is
 * held by `landing/tests/whats-new.test.mjs`). These tests pin the load-bearing
 * phrases, not the prose: the copilot is a beta, off by default, reached from
 * Settings → Beta and not yet checked against a real model; the validation
 * notice is still present; and the word "undetectable" never appears in
 * product copy (ADR-0038 invariant 1).
 */

import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import { About } from './About';

vi.mock('@tauri-apps/api/app', () => ({
  getVersion: () => Promise.resolve('1.2.1'),
}));
vi.mock('@/services/systemService', () => ({
  openExternalUrl: vi.fn(() => Promise.resolve()),
}));
vi.mock('@/services/updateService', () => ({
  updateService: { checkForUpdates: vi.fn(() => Promise.resolve({ available: false })) },
}));
vi.mock('./AnalyticsConsentSwitch', () => ({
  default: () => <div data-testid="analytics-consent-switch" />,
}));
vi.mock('./UpdateDialog', () => ({
  UpdateDialog: () => null,
}));

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

describe('About describes the app the way the landing page does', () => {
  it('presents the live copilot as a beta that is off by default and unvalidated', () => {
    render(<About />);
    expect(screen.getByRole('heading', { name: 'In beta' })).toBeTruthy();
    expect(screen.getByRole('heading', { name: /Live copilot — off by default/ })).toBeTruthy();
    const text = document.body.textContent ?? '';
    expect(text).toMatch(/Settings → Beta/);
    expect(text).toMatch(/not yet been checked against a real model/);
    expect(text).toMatch(/cites the transcript segment it came from, or is refused/);
  });

  it('keeps the validation notice and describes the shipped features', () => {
    render(<About />);
    const text = document.body.textContent ?? '';
    expect(text).toMatch(/target-environment benchmark and human pilot have not been performed/);
    expect(text).toMatch(/Speaker separation and talk time run on this device after the recording ends/);
    expect(text).toMatch(/Ask this meeting answers only from that meeting/);
    expect(text).toMatch(/Export approved notes to PDF, Word or Markdown/);
  });

  it('lists what is in development without dates and never uses forbidden wording', () => {
    render(<About />);
    const text = document.body.textContent ?? '';
    expect(text).toMatch(/In development, not in this build/);
    expect(text).toMatch(/macOS build/);
    expect(text).not.toMatch(/undetectable/i);
    expect(text).not.toMatch(/Coming soon:/);
    // No latency or accuracy figure before the I9 / A5 gates.
    expect(text).not.toMatch(/\b\d+(\.\d+)?\s?(ms|milliseconds)\b/i);
  });
});
