// @vitest-environment jsdom

/**
 * The dock is the only Stop that exists on /settings, /actions and /meeting-details, so
 * the things pinned here are the ones whose regression would strand a live recording:
 * Stop is never disabled, paused is distinguishable without colour, and the clock is the
 * backend's active duration rather than a timer this component runs.
 */

import React from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { SessionDock } from './SessionDock';

afterEach(cleanup);

describe('SessionDock', () => {
  it('renders nothing when there is no session', () => {
    const { container } = render(
      <SessionDock isRecording={false} isPaused={false} activeDuration={0} onStop={() => {}} />
    );
    expect(container.firstChild).toBeNull();
  });

  it('shows the backend active duration as mm:ss', () => {
    render(
      <SessionDock isRecording isPaused={false} activeDuration={754.8} onStop={() => {}} />
    );
    expect(screen.getByText('Recording • 12:34')).toBeTruthy();
  });

  it('keeps Stop enabled while the stop is draining', () => {
    render(
      <SessionDock isRecording isPaused={false} activeDuration={10} isStopping onStop={() => {}} />
    );
    const stop = screen.getByRole('button', { name: 'Stop recording' });
    expect(stop.hasAttribute('disabled')).toBe(false);
    expect(stop.textContent).toContain('Stopping');
  });

  it('stops on click', () => {
    const onStop = vi.fn();
    render(<SessionDock isRecording isPaused={false} activeDuration={10} onStop={onStop} />);
    fireEvent.click(screen.getByRole('button', { name: 'Stop recording' }));
    expect(onStop).toHaveBeenCalledTimes(1);
  });

  it('distinguishes paused by word and shape, not only by colour', () => {
    const { container } = render(
      <SessionDock isRecording isPaused activeDuration={754} onStop={() => {}} />
    );
    expect(screen.getByText('Paused • 12:34')).toBeTruthy();
    // A square glyph, never a second round dot: reduced motion removes the pulse, so
    // shape is what is left (DESIGN_SYSTEM.md §4.9).
    const glyph = container.querySelector('span[aria-hidden="true"]');
    expect(glyph?.className).toContain('bg-warning');
    expect(glyph?.className).not.toContain('rounded-full');
  });

  it('names the state for a screen reader that never sees the dot', () => {
    const { rerender } = render(
      <SessionDock isRecording isPaused={false} activeDuration={5} onStop={() => {}} />
    );
    expect(screen.getByLabelText('Recording in progress')).toBeTruthy();
    rerender(<SessionDock isRecording isPaused activeDuration={5} onStop={() => {}} />);
    expect(screen.getByLabelText('Recording paused')).toBeTruthy();
  });

  it('pulses only when motion is allowed', () => {
    const { container } = render(
      <SessionDock isRecording isPaused={false} activeDuration={5} onStop={() => {}} />
    );
    const glyph = container.querySelector('span[aria-hidden="true"]');
    expect(glyph?.className).toContain('motion-safe:animate-pulse');
  });
});
