import { describe, expect, it } from 'vitest';
import { endsRecordingUi, parseTranscriptionError } from './transcriptionErrorPolicy';

describe('a transcription error mid-recording', () => {
  it('one failed chunk does not end the recording in the UI', () => {
    // The exact payload `transcribe_chunk_with_provider` emits for a failed chunk.
    const payload = {
      error: 'Transcription engine failed: decode error',
      userMessage: 'Transcription failed: Transcription engine failed: decode error',
      actionable: false,
    };
    expect(endsRecordingUi(payload)).toBe(false);
    expect(parseTranscriptionError(payload).message).toContain('Transcription failed');
  });

  it('an engine that could not start does', () => {
    const payload = {
      error: 'No model',
      userMessage:
        'Recording failed: Unable to initialize speech recognition. Please check your model settings.',
      actionable: true,
    };
    expect(endsRecordingUi(payload)).toBe(true);
  });

  it('a bare string payload is shown and never ends the recording', () => {
    expect(parseTranscriptionError('boom')).toEqual({ message: 'boom', actionable: false });
    expect(endsRecordingUi('boom')).toBe(false);
  });

  it('a payload with no flag is treated as one bad chunk', () => {
    expect(endsRecordingUi({ error: 'x' })).toBe(false);
    expect(parseTranscriptionError({}).message).toBe('Transcription failed.');
  });
});
