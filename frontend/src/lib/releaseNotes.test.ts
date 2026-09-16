/**
 * What's new (BACKLOG G2).
 *
 * The two ways this feature can lie to a user: showing release notes to
 * somebody who just installed the app (nothing is "new" to them), and hiding
 * the versions they skipped. Both are pinned here.
 */

import { describe, expect, it } from 'vitest';
import { APP_VERSION } from './appVersion';
import { RELEASE_NOTES, compareVersions, releaseNoteFor, whatsNewFor } from './releaseNotes';

describe('which notes a user sees', () => {
  it('shows nothing on a fresh install', () => {
    expect(whatsNewFor('1.2.1', null)).toEqual([]);
  });

  it('shows the note for the version just installed', () => {
    const notes = whatsNewFor('1.2.1', '1.2.0');
    expect(notes.map((n) => n.version)).toEqual(['1.2.1']);
  });

  it('shows every skipped version, newest first', () => {
    const notes = whatsNewFor('1.2.1', '1.1.0');
    expect(notes.map((n) => n.version)).toEqual(['1.2.1', '1.2.0']);
  });

  it('shows nothing when the version is unchanged', () => {
    expect(whatsNewFor('1.2.1', '1.2.1')).toEqual([]);
  });

  it('shows nothing on a downgrade', () => {
    expect(whatsNewFor('1.2.0', '1.2.1')).toEqual([]);
  });

  it('never shows a note for a version newer than the running build', () => {
    const notes = whatsNewFor('1.2.0', '1.1.0');
    expect(notes.every((n) => compareVersions(n.version, '1.2.0') <= 0)).toBe(true);
  });
});

describe('the notes themselves', () => {
  /**
   * The running build must have a note, or the dialog opens empty for the one
   * user who most needs it. This fails the moment a release bumps the version
   * and forgets the note.
   */
  it('has an entry for the version this build actually is', () => {
    expect(releaseNoteFor(APP_VERSION), `no release note for ${APP_VERSION}`).toBeTruthy();
  });

  it('states what is still not true in every release', () => {
    for (const note of RELEASE_NOTES) {
      expect(Array.isArray(note.caveats), note.version).toBe(true);
      expect(note.changes.length, note.version).toBeGreaterThan(0);
    }
  });

  it('is ordered newest first', () => {
    for (let i = 1; i < RELEASE_NOTES.length; i += 1) {
      expect(compareVersions(RELEASE_NOTES[i - 1].version, RELEASE_NOTES[i].version)).toBeGreaterThan(0);
    }
  });
});

describe('version comparison', () => {
  it('orders by major, then minor, then patch', () => {
    expect(compareVersions('2.0.0', '1.9.9')).toBeGreaterThan(0);
    expect(compareVersions('1.3.0', '1.2.9')).toBeGreaterThan(0);
    expect(compareVersions('1.2.2', '1.2.1')).toBeGreaterThan(0);
    expect(compareVersions('1.2.1', '1.2.1')).toBe(0);
  });

  it('does not compare version parts as text', () => {
    // "10" < "9" as strings; as versions it is greater.
    expect(compareVersions('1.10.0', '1.9.0')).toBeGreaterThan(0);
  });

  it('treats a missing or unparseable part as zero rather than NaN', () => {
    expect(compareVersions('1.2', '1.2.0')).toBe(0);
    expect(compareVersions('1.2.1-beta', '1.2.1')).toBe(0);
  });
});
