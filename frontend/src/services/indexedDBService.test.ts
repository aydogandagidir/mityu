import 'fake-indexeddb/auto';

import { describe, expect, it, vi } from 'vitest';

const trustedWorkspace = vi.hoisted(() => ({ id: 'workspace-a' }));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(async (command: string) => {
    if (command !== 'api_get_current_workspace_id') {
      throw new Error(`unexpected Tauri command: ${command}`);
    }
    return trustedWorkspace.id;
  }),
}));

import {
  RECOVERY_DB_VERSION,
  buildScopedMeetingRecord,
  buildScopedTranscriptRecord,
  meetingKey,
  migrateLegacyMeetingRecord,
  recordsForWorkspace,
  transcriptKey,
  workspaceMeetingKey,
  indexedDBService,
  type MeetingMetadata,
} from './indexedDBService';

function openRecoveryDatabase(version?: number): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const request = version === undefined
      ? indexedDB.open('MeetilyRecoveryDB')
      : indexedDB.open('MeetilyRecoveryDB', version);
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error);
  });
}

function transactionDone(transaction: IDBTransaction): Promise<void> {
  return new Promise((resolve, reject) => {
    transaction.oncomplete = () => resolve();
    transaction.onerror = () => reject(transaction.error);
    transaction.onabort = () => reject(transaction.error);
  });
}

function deleteRecoveryDatabase(): Promise<void> {
  return new Promise((resolve, reject) => {
    const request = indexedDB.deleteDatabase('MeetilyRecoveryDB');
    request.onsuccess = () => resolve();
    request.onerror = () => reject(request.error);
    request.onblocked = () => reject(new Error('recovery database deletion was blocked'));
  });
}

async function seedLegacyV1Recovery(): Promise<void> {
  await deleteRecoveryDatabase();
  const database = await new Promise<IDBDatabase>((resolve, reject) => {
    const request = indexedDB.open('MeetilyRecoveryDB', 1);
    request.onupgradeneeded = () => {
      request.result.createObjectStore('meetings', { keyPath: 'meetingId' });
      request.result.createObjectStore('transcripts', {
        keyPath: 'id',
        autoIncrement: true,
      });
    };
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error);
  });
  const transaction = database.transaction(['meetings', 'transcripts'], 'readwrite');
  transaction.objectStore('meetings').put({
    meetingId: 'shared-meeting-id',
    title: 'Legacy recovery',
    startTime: 10,
    lastUpdated: 20,
    transcriptCount: 1,
    savedToSQLite: false,
  });
  transaction.objectStore('transcripts').add({
    meetingId: 'shared-meeting-id',
    text: 'legacy transcript',
    timestamp: '2026-07-15T10:00:00Z',
    confidence: 0.9,
    sequenceId: 1,
    storedAt: 15,
  });
  await transactionDone(transaction);
  database.close();
}

describe('IndexedDB recovery tenant boundary', () => {
  it('uses schema v2 and workspace-prefixed compound keys', () => {
    expect(RECOVERY_DB_VERSION).toBe(2);
    expect(meetingKey('workspace-a', 'meeting-1')).toEqual(['workspace-a', 'meeting-1']);
    expect(workspaceMeetingKey('workspace-b', 'meeting-1')).toEqual([
      'workspace-b',
      'meeting-1',
    ]);
    expect(meetingKey('workspace-a', 'meeting-1')).not.toEqual(
      meetingKey('workspace-b', 'meeting-1'),
    );
    expect(transcriptKey('workspace-a', 'meeting-1', 'sequence:7')).toEqual([
      'workspace-a',
      'meeting-1',
      'sequence:7',
    ]);
  });

  it('overwrites renderer-supplied workspace and saved-state fields', () => {
    const spoofed = {
      workspaceId: 'foreign-workspace',
      meetingId: 'meeting-1',
      title: 'Recovery',
      startTime: 10,
      lastUpdated: 20,
      transcriptCount: 1,
      savedToSQLite: false,
      savedState: 'saved',
    } as MeetingMetadata;

    const scoped = buildScopedMeetingRecord(spoofed, 'trusted-workspace');
    expect(scoped.workspaceId).toBe('trusted-workspace');
    expect(scoped.savedToSQLite).toBe(false);
    expect(scoped.savedState).toBe('pending');
  });

  it('normalizes transcript sequence ids and forces trusted compound-key fields', () => {
    const scoped = buildScopedTranscriptRecord('meeting-1', {
      workspaceId: 'foreign-workspace',
      meetingId: 'foreign-meeting',
      recoveryId: 'renderer-controlled',
      text: 'source text',
      timestamp: '2026-07-15T10:00:00Z',
      confidence: 0.9,
      sequence_id: 7,
      storedAt: 1,
    }, 'trusted-workspace', { now: 500 });

    expect(scoped.workspaceId).toBe('trusted-workspace');
    expect(scoped.meetingId).toBe('meeting-1');
    expect(scoped.recoveryId).toBe('sequence:7');
    expect(scoped.sequenceId).toBe(7);
    expect(scoped.sequence_id).toBe(7);
    expect(scoped.storedAt).toBe(500);
  });

  it('maps legacy records deterministically without dropping payload fields', () => {
    const legacy = {
      id: 42,
      meetingId: 'legacy-meeting',
      text: 'legacy transcript',
      timestamp: '2026-07-15T10:00:00Z',
      confidence: 0.75,
      sequence_id: 3,
      storedAt: 123,
      customLegacyField: 'preserved',
    };

    const first = buildScopedTranscriptRecord(
      'legacy-meeting',
      legacy,
      'trusted-workspace',
      { legacyOrdinal: 0, now: 999 },
    );
    const retried = buildScopedTranscriptRecord(
      'legacy-meeting',
      legacy,
      'trusted-workspace',
      { legacyOrdinal: 0, now: 999 },
    );

    expect(first.recoveryId).toBe('legacy:number:42');
    expect(retried.recoveryId).toBe(first.recoveryId);
    expect(first.storedAt).toBe(123);
    expect(first.customLegacyField).toBe('preserved');

    const withoutLegacyIdA = buildScopedTranscriptRecord(
      'legacy-meeting',
      { ...legacy, id: undefined },
      'trusted-workspace',
      { legacyOrdinal: 1, now: 999 },
    );
    const withoutLegacyIdB = buildScopedTranscriptRecord(
      'legacy-meeting',
      { ...legacy, id: undefined },
      'trusted-workspace',
      { legacyOrdinal: 2, now: 999 },
    );
    expect(withoutLegacyIdA.recoveryId).not.toBe(withoutLegacyIdB.recoveryId);
  });

  it('assigns legacy meetings to the trusted migration workspace once', () => {
    const migrated = migrateLegacyMeetingRecord({
      workspaceId: 'untrusted-legacy-value',
      meetingId: 'legacy-meeting',
      title: 'Legacy title',
      startTime: 10,
      lastUpdated: 20,
      transcriptCount: 2,
      savedToSQLite: true,
    }, 'trusted-workspace', 1000);

    expect(migrated.workspaceId).toBe('trusted-workspace');
    expect(migrated.savedState).toBe('saved');
    expect(migrated.title).toBe('Legacy title');
  });

  it('defensively excludes records from every other workspace', () => {
    const records = [
      { workspaceId: 'workspace-a', meetingId: 'a' },
      { workspaceId: 'workspace-b', meetingId: 'b' },
      { workspaceId: 'workspace-a', meetingId: 'c' },
    ];

    expect(recordsForWorkspace(records, 'workspace-a').map(record => record.meetingId)).toEqual([
      'a',
      'c',
    ]);
  });

  it('atomically upgrades v1 stores and isolates identical meeting ids end to end', async () => {
    trustedWorkspace.id = 'workspace-a';
    await seedLegacyV1Recovery();
    await indexedDBService.init();

    const upgraded = await openRecoveryDatabase();
    expect(upgraded.version).toBe(RECOVERY_DB_VERSION);
    expect([...upgraded.objectStoreNames]).toEqual(['meetingsV2', 'transcriptsV2']);
    upgraded.close();

    expect((await indexedDBService.getMeetingMetadata('shared-meeting-id'))?.workspaceId)
      .toBe('workspace-a');
    expect(await indexedDBService.getTranscriptCount('shared-meeting-id')).toBe(1);
    expect((await indexedDBService.getTranscripts('shared-meeting-id'))[0]?.text)
      .toBe('legacy transcript');

    const database = await openRecoveryDatabase();
    const foreignWrite = database.transaction(['meetingsV2', 'transcriptsV2'], 'readwrite');
    foreignWrite.objectStore('meetingsV2').put({
      workspaceId: 'workspace-b',
      meetingId: 'shared-meeting-id',
      title: 'Foreign recovery',
      startTime: 30,
      lastUpdated: 40,
      transcriptCount: 1,
      savedToSQLite: false,
      savedState: 'pending',
    });
    foreignWrite.objectStore('transcriptsV2').put({
      workspaceId: 'workspace-b',
      meetingId: 'shared-meeting-id',
      recoveryId: 'sequence:1',
      text: 'foreign transcript',
      timestamp: '2026-07-15T10:01:00Z',
      confidence: 0.8,
      sequenceId: 1,
      sequence_id: 1,
      storedAt: 35,
    });
    await transactionDone(foreignWrite);
    database.close();

    expect((await indexedDBService.getAllMeetings()).map(meeting => meeting.workspaceId))
      .toEqual(['workspace-a']);
    await indexedDBService.deleteMeeting('shared-meeting-id');
    expect(await indexedDBService.getMeetingMetadata('shared-meeting-id')).toBeNull();

    trustedWorkspace.id = 'workspace-b';
    expect((await indexedDBService.getMeetingMetadata('shared-meeting-id'))?.title)
      .toBe('Foreign recovery');
    expect((await indexedDBService.getTranscripts('shared-meeting-id'))[0]?.text)
      .toBe('foreign transcript');

    await deleteRecoveryDatabase();
  });
});
