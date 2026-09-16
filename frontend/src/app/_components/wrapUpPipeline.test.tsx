// @vitest-environment jsdom

/**
 * The wrap-up panel exists to show a message the app has always written and never
 * displayed. These pin the two things that can silently regress: the developer wording
 * reaching the user unchanged, and the step whose turn it is.
 */

import React from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { WrapUpPipeline } from './WrapUpPipeline';
import { RecordingStatus } from '@/contexts/RecordingStateContext';

afterEach(cleanup);

describe('WrapUpPipeline', () => {
  it('translates the queue depth instead of printing "chunks"', () => {
    render(
      <WrapUpPipeline
        status={RecordingStatus.PROCESSING_TRANSCRIPTS}
        statusMessage="Processing 7 remaining chunks..."
        segmentCount={41}
      />
    );
    expect(screen.getByText('7 pieces of audio still being transcribed')).toBeTruthy();
    expect(screen.queryByText(/chunks/i)).toBeNull();
  });

  it('says "1 piece" for the last one', () => {
    render(
      <WrapUpPipeline
        status={RecordingStatus.PROCESSING_TRANSCRIPTS}
        statusMessage="Processing 1 remaining chunks..."
        segmentCount={41}
      />
    );
    expect(screen.getByText('1 piece of audio still being transcribed')).toBeTruthy();
  });

  it('attaches the status message to the step that is actually running', () => {
    const { rerender } = render(
      <WrapUpPipeline
        status={RecordingStatus.PROCESSING_TRANSCRIPTS}
        statusMessage="Processing 3 remaining chunks..."
        segmentCount={4}
      />
    );
    const transcribing = screen.getByText('3 pieces of audio still being transcribed');
    expect(transcribing.closest('li')?.textContent).toContain('Finalizing transcript');

    rerender(
      <WrapUpPipeline
        status={RecordingStatus.SAVING}
        statusMessage="Saving meeting to database..."
        segmentCount={4}
      />
    );
    expect(screen.queryByText('3 pieces of audio still being transcribed')).toBeNull();
    const saving = screen.getByText('Writing the meeting to this device');
    expect(saving.closest('li')?.textContent).toContain('Saving meeting');
  });

  it('is a live region, so a screen reader hears the step change', () => {
    render(
      <WrapUpPipeline status={RecordingStatus.SAVING} segmentCount={2} />
    );
    const region = screen.getByRole('status');
    expect(region.getAttribute('aria-live')).toBe('polite');
  });

  it('offers the report only once there is a meeting to open', () => {
    const onOpenReport = vi.fn();
    const { rerender } = render(
      <WrapUpPipeline
        status={RecordingStatus.COMPLETED}
        segmentCount={12}
        meetingId={null}
        onOpenReport={onOpenReport}
      />
    );
    expect(screen.queryByRole('button', { name: 'Open report' })).toBeNull();

    rerender(
      <WrapUpPipeline
        status={RecordingStatus.COMPLETED}
        segmentCount={12}
        meetingId="m-1"
        onOpenReport={onOpenReport}
      />
    );
    fireEvent.click(screen.getByRole('button', { name: 'Open report' }));
    expect(onOpenReport).toHaveBeenCalledTimes(1);
  });

  it('tells the user their audio survived a failed save, and what broke', () => {
    render(
      <WrapUpPipeline
        status={RecordingStatus.ERROR}
        statusMessage="database is locked"
        segmentCount={12}
      />
    );
    expect(screen.getByText('The recording was kept, but saving did not finish')).toBeTruthy();
    expect(screen.getByText(/still on this device/)).toBeTruthy();
    expect(screen.getByText(/database is locked/)).toBeTruthy();
    // Nothing succeeded is marked as having failed: the app does not report which step broke.
    expect(screen.queryByText('Closing the recording')).toBeNull();
  });

  it('counts the saved segments in plain words', () => {
    render(<WrapUpPipeline status={RecordingStatus.COMPLETED} segmentCount={1} />);
    expect(screen.getByText('1 transcript segment saved.')).toBeTruthy();
  });
});
