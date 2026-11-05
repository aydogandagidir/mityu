import { afterEach, describe, expect, it } from 'vitest';
import {
  attachLiveNotesToMeeting,
  getLiveNotes,
  peekNotesForMeeting,
  resetLiveNotesForTests,
  setLiveNotes,
} from './liveMeetingNotes';

afterEach(resetLiveNotesForTests);

describe('liveMeetingNotes', () => {
  it('starts empty', () => {
    expect(getLiveNotes()).toBe('');
    expect(peekNotesForMeeting('m-1')).toBe('');
  });

  it('binds the live notes to the saved meeting and empties the live buffer', () => {
    setLiveNotes('  decide pricing by Friday \n');
    attachLiveNotesToMeeting('m-1');
    expect(peekNotesForMeeting('m-1')).toBe('decide pricing by Friday');
    expect(getLiveNotes()).toBe('');
  });

  it('never hands one meeting the notes of another', () => {
    setLiveNotes('notes for m-1');
    attachLiveNotesToMeeting('m-1');
    expect(peekNotesForMeeting('m-2')).toBe('');
    expect(peekNotesForMeeting(null)).toBe('');
    expect(peekNotesForMeeting(undefined)).toBe('');
  });

  it('does not overwrite earlier bound notes with an empty recording', () => {
    setLiveNotes('first');
    attachLiveNotesToMeeting('m-1');
    setLiveNotes('   ');
    attachLiveNotesToMeeting('m-2');
    expect(peekNotesForMeeting('m-1')).toBe('first');
    expect(peekNotesForMeeting('m-2')).toBe('');
  });

  it('reading is not consuming — reopening the report still has them', () => {
    setLiveNotes('keep me');
    attachLiveNotesToMeeting('m-1');
    peekNotesForMeeting('m-1');
    expect(peekNotesForMeeting('m-1')).toBe('keep me');
  });

  it('ignores an empty meeting id', () => {
    setLiveNotes('orphan');
    attachLiveNotesToMeeting('');
    expect(peekNotesForMeeting('')).toBe('');
  });
});

