// @vitest-environment jsdom

/**
 * The record button's absence used to be the whole message.
 *
 * The home page rendered its recording controls only when a microphone had been
 * found, so a user whose mic was unplugged, disabled in Windows, or blocked by
 * the privacy setting was shown an empty strip: no button, no disabled button,
 * no sentence anywhere in the app. "The button is missing" and "the button does
 * nothing" look identical to the person pressing it, and both read as a broken
 * product — which is exactly how the shipped bug was reported.
 *
 * What is tested here is therefore not styling. It is that the slot always
 * says something, that it distinguishes "you have no microphone" from "we could
 * not ask", and that it offers a way out without restarting the app.
 */

import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { NoMicrophoneNotice } from './NoMicrophoneNotice';

afterEach(cleanup);

describe('the notice shown where the record button would be', () => {
  it('names the problem instead of leaving the space blank', () => {
    render(<NoMicrophoneNotice error={null} onRetry={() => {}} />);

    expect(screen.getByText('No microphone found')).toBeDefined();
  });

  it('tells the user what to check, including the Windows privacy setting', () => {
    render(<NoMicrophoneNotice error={null} onRetry={() => {}} />);

    const body = screen.getByRole('status').textContent ?? '';
    expect(body).toMatch(/plugged in and enabled/i);
    // The single most common cause on the platform this ships to, and one the
    // app cannot fix for the user — so it has to be named, not hinted at.
    expect(body).toMatch(/Privacy & security/i);
  });

  it('distinguishes "could not ask" from "you have none", and shows the reason', () => {
    // Failing to enumerate devices is a different fault from having no device,
    // and sending the user to the wrong settings screen wastes their time.
    render(<NoMicrophoneNotice error="device enumeration failed" onRetry={() => {}} />);

    const body = screen.getByRole('status').textContent ?? '';
    expect(body).toMatch(/could not read the list of audio devices/i);
    expect(body).toContain('device enumeration failed');
    expect(body).not.toMatch(/plugged in and enabled/i);
  });

  it('offers a way out that is not restarting the app', () => {
    const onRetry = vi.fn();
    render(<NoMicrophoneNotice error={null} onRetry={onRetry} />);

    fireEvent.click(screen.getByRole('button', { name: /check again/i }));
    expect(onRetry).toHaveBeenCalledTimes(1);
  });

  it('announces itself to assistive technology', () => {
    render(<NoMicrophoneNotice error={null} onRetry={() => {}} />);

    expect(screen.getByRole('status')).toBeDefined();
  });
});
