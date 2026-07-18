/**
 * Tenant-scoped IndexedDB persistence for crash-recovery transcripts.
 *
 * Identity is never accepted from the renderer. Every operation resolves the
 * current workspace through Rust `context::current()` and uses compound keys or
 * indexes beginning with that trusted workspace id.
 */

export const RECOVERY_DB_VERSION = 2;

const DB_NAME = 'MeetilyRecoveryDB'; // Frozen so the v1 database can be upgraded in place.
const MEETINGS_STORE = 'meetingsV2';
const TRANSCRIPTS_STORE = 'transcriptsV2';
const LEGACY_MEETINGS_STORE = 'meetings';
const LEGACY_TRANSCRIPTS_STORE = 'transcripts';

const MEETING_WORKSPACE_INDEX = 'workspaceId';
const MEETING_WORKSPACE_SAVED_INDEX = 'workspaceSavedState';
const MEETING_WORKSPACE_UPDATED_INDEX = 'workspaceLastUpdated';
const TRANSCRIPT_WORKSPACE_INDEX = 'workspaceId';
const TRANSCRIPT_WORKSPACE_MEETING_INDEX = 'workspaceMeeting';
const TRANSCRIPT_WORKSPACE_STORED_INDEX = 'workspaceStoredAt';

type SavedState = 'pending' | 'saved';
type UnknownRecord = Record<string, unknown>;

export interface MeetingMetadata {
  workspaceId: string;
  meetingId: string;
  title: string;
  startTime: number;
  lastUpdated: number;
  transcriptCount: number;
  savedToSQLite: boolean;
  savedState: SavedState;
  folderPath?: string;
}

export type MeetingMetadataInput = Omit<MeetingMetadata, 'workspaceId' | 'savedState'>;

export interface StoredTranscript {
  workspaceId: string;
  meetingId: string;
  recoveryId: string;
  id?: number | string;
  text: string;
  timestamp: string;
  confidence: number;
  sequenceId: number;
  sequence_id: number;
  storedAt: number;
  audio_start_time?: number;
  audio_end_time?: number;
  duration?: number;
  [key: string]: any;
}

function requireIdentifier(value: unknown, label: string): string {
  if (typeof value !== 'string' || value.trim().length === 0) {
    throw new Error(`${label} must be a non-empty string`);
  }
  return value;
}

function finiteNumber(value: unknown): number | undefined {
  return typeof value === 'number' && Number.isFinite(value) ? value : undefined;
}

function sequenceNumber(value: unknown): number | undefined {
  const number = typeof value === 'number' ? value : Number(value);
  return Number.isSafeInteger(number) && number >= 0 ? number : undefined;
}

export function meetingKey(workspaceId: string, meetingId: string): [string, string] {
  return [
    requireIdentifier(workspaceId, 'workspaceId'),
    requireIdentifier(meetingId, 'meetingId'),
  ];
}

export function workspaceMeetingKey(
  workspaceId: string,
  meetingId: string,
): [string, string] {
  return meetingKey(workspaceId, meetingId);
}

export function transcriptKey(
  workspaceId: string,
  meetingId: string,
  recoveryId: string,
): [string, string, string] {
  return [
    ...meetingKey(workspaceId, meetingId),
    requireIdentifier(recoveryId, 'recoveryId'),
  ];
}

export function recordsForWorkspace<T extends { workspaceId: string }>(
  records: T[],
  workspaceId: string,
): T[] {
  return records.filter(record => record.workspaceId === workspaceId);
}

export function buildScopedMeetingRecord(
  metadata: MeetingMetadataInput | MeetingMetadata,
  workspaceId: string,
): MeetingMetadata {
  const trustedWorkspaceId = requireIdentifier(workspaceId, 'workspaceId');
  const meetingId = requireIdentifier(metadata.meetingId, 'meetingId');
  const savedToSQLite = metadata.savedToSQLite === true;

  return {
    ...metadata,
    workspaceId: trustedWorkspaceId,
    meetingId,
    savedToSQLite,
    savedState: savedToSQLite ? 'saved' : 'pending',
  };
}

function legacyRecoveryId(record: UnknownRecord, ordinal: number): string {
  const legacyId = record.id;
  if (typeof legacyId === 'number' || typeof legacyId === 'string') {
    return `legacy:${typeof legacyId}:${String(legacyId)}`;
  }
  return `legacy:ordinal:${ordinal}`;
}

export interface TranscriptScopeOptions {
  legacyOrdinal?: number;
  now?: number;
}

export function buildScopedTranscriptRecord(
  meetingIdInput: string,
  transcript: object,
  workspaceId: string,
  options: TranscriptScopeOptions = {},
): StoredTranscript {
  const record = transcript as UnknownRecord;
  const trustedWorkspaceId = requireIdentifier(workspaceId, 'workspaceId');
  const meetingId = requireIdentifier(meetingIdInput, 'meetingId');
  const legacy = options.legacyOrdinal !== undefined;
  const sequenceId = sequenceNumber(record.sequenceId ?? record.sequence_id)
    ?? (legacy ? options.legacyOrdinal : undefined);

  if (sequenceId === undefined) {
    throw new Error('transcript sequence id must be a non-negative safe integer');
  }

  const now = options.now ?? Date.now();
  const storedAt = legacy ? (finiteNumber(record.storedAt) ?? now) : now;
  const recoveryId = legacy
    ? legacyRecoveryId(record, options.legacyOrdinal!)
    : `sequence:${sequenceId}`;

  return {
    ...record,
    workspaceId: trustedWorkspaceId,
    meetingId,
    recoveryId,
    sequenceId,
    sequence_id: sequenceId,
    storedAt,
  } as StoredTranscript;
}

export function migrateLegacyMeetingRecord(
  record: UnknownRecord,
  workspaceId: string,
  now = Date.now(),
): MeetingMetadata {
  const meetingId = requireIdentifier(record.meetingId, 'legacy meetingId');
  const startTime = finiteNumber(record.startTime) ?? now;
  const lastUpdated = finiteNumber(record.lastUpdated) ?? startTime;
  const transcriptCount = sequenceNumber(record.transcriptCount) ?? 0;

  return buildScopedMeetingRecord({
    ...record,
    meetingId,
    title: typeof record.title === 'string' ? record.title : 'Recovered meeting',
    startTime,
    lastUpdated,
    transcriptCount,
    savedToSQLite: record.savedToSQLite === true,
    folderPath: typeof record.folderPath === 'string' ? record.folderPath : undefined,
  } as MeetingMetadataInput, workspaceId);
}

async function resolveCurrentWorkspaceId(): Promise<string> {
  const { invoke } = await import('@tauri-apps/api/core');
  const workspaceId = await invoke<unknown>('api_get_current_workspace_id');
  return requireIdentifier(workspaceId, 'current workspace id');
}

function createV2Stores(db: IDBDatabase): void {
  if (!db.objectStoreNames.contains(MEETINGS_STORE)) {
    const meetings = db.createObjectStore(MEETINGS_STORE, {
      keyPath: ['workspaceId', 'meetingId'],
    });
    meetings.createIndex(MEETING_WORKSPACE_INDEX, 'workspaceId', { unique: false });
    meetings.createIndex(
      MEETING_WORKSPACE_SAVED_INDEX,
      ['workspaceId', 'savedState'],
      { unique: false },
    );
    meetings.createIndex(
      MEETING_WORKSPACE_UPDATED_INDEX,
      ['workspaceId', 'lastUpdated'],
      { unique: false },
    );
  }

  if (!db.objectStoreNames.contains(TRANSCRIPTS_STORE)) {
    const transcripts = db.createObjectStore(TRANSCRIPTS_STORE, {
      keyPath: ['workspaceId', 'meetingId', 'recoveryId'],
    });
    transcripts.createIndex(TRANSCRIPT_WORKSPACE_INDEX, 'workspaceId', { unique: false });
    transcripts.createIndex(
      TRANSCRIPT_WORKSPACE_MEETING_INDEX,
      ['workspaceId', 'meetingId'],
      { unique: false },
    );
    transcripts.createIndex(
      TRANSCRIPT_WORKSPACE_STORED_INDEX,
      ['workspaceId', 'storedAt'],
      { unique: false },
    );
  }
}

/**
 * Copy v1 records into v2 inside the same versionchange transaction. If any
 * read/write fails, IndexedDB rolls the whole upgrade back and v1 remains
 * intact. On success the legacy stores are deleted, so migration cannot repeat.
 */
function migrateLegacyV1Stores(
  db: IDBDatabase,
  transaction: IDBTransaction,
  workspaceId: string,
): void {
  const hasMeetings = db.objectStoreNames.contains(LEGACY_MEETINGS_STORE);
  const hasTranscripts = db.objectStoreNames.contains(LEGACY_TRANSCRIPTS_STORE);
  if (!hasMeetings && !hasTranscripts) return;

  let legacyMeetings: UnknownRecord[] = [];
  let legacyTranscripts: UnknownRecord[] = [];
  let pendingReads = Number(hasMeetings) + Number(hasTranscripts);

  const abort = () => {
    try {
      transaction.abort();
    } catch {
      // The transaction may already have failed; the open request will surface it.
    }
  };

  const finish = () => {
    pendingReads -= 1;
    if (pendingReads !== 0) return;

    try {
      const now = Date.now();
      const meetingsStore = transaction.objectStore(MEETINGS_STORE);
      const transcriptsStore = transaction.objectStore(TRANSCRIPTS_STORE);
      const migratedMeetingIds = new Set<string>();
      const orphanCounts = new Map<string, { count: number; oldest: number; newest: number }>();

      for (const record of legacyMeetings) {
        const scoped = migrateLegacyMeetingRecord(record, workspaceId, now);
        migratedMeetingIds.add(scoped.meetingId);
        meetingsStore.put(scoped);
      }

      legacyTranscripts.forEach((record, ordinal) => {
        const meetingId = requireIdentifier(record.meetingId, 'legacy transcript meetingId');
        const scoped = buildScopedTranscriptRecord(meetingId, record, workspaceId, {
          legacyOrdinal: ordinal,
          now,
        });
        transcriptsStore.put(scoped);

        if (!migratedMeetingIds.has(meetingId)) {
          const prior = orphanCounts.get(meetingId);
          orphanCounts.set(meetingId, {
            count: (prior?.count ?? 0) + 1,
            oldest: Math.min(prior?.oldest ?? scoped.storedAt, scoped.storedAt),
            newest: Math.max(prior?.newest ?? scoped.storedAt, scoped.storedAt),
          });
        }
      });

      // Preserve otherwise-orphaned transcript records as recoverable meetings.
      for (const [meetingId, stats] of orphanCounts) {
        meetingsStore.put(buildScopedMeetingRecord({
          meetingId,
          title: 'Recovered meeting',
          startTime: stats.oldest,
          lastUpdated: stats.newest,
          transcriptCount: stats.count,
          savedToSQLite: false,
        }, workspaceId));
      }

      if (hasMeetings) db.deleteObjectStore(LEGACY_MEETINGS_STORE);
      if (hasTranscripts) db.deleteObjectStore(LEGACY_TRANSCRIPTS_STORE);
    } catch {
      abort();
    }
  };

  if (hasMeetings) {
    const request = transaction.objectStore(LEGACY_MEETINGS_STORE).getAll();
    request.onsuccess = () => {
      legacyMeetings = request.result as UnknownRecord[];
      finish();
    };
    request.onerror = abort;
  }

  if (hasTranscripts) {
    const request = transaction.objectStore(LEGACY_TRANSCRIPTS_STORE).getAll();
    request.onsuccess = () => {
      legacyTranscripts = request.result as UnknownRecord[];
      finish();
    };
    request.onerror = abort;
  }
}

class IndexedDBService {
  private db: IDBDatabase | null = null;
  private initPromise: Promise<void> | null = null;

  async init(): Promise<void> {
    if (this.db) return;
    if (this.initPromise) return this.initPromise;

    this.initPromise = this.initialize().catch(error => {
      this.initPromise = null;
      throw error;
    });
    return this.initPromise;
  }

  private async initialize(): Promise<void> {
    // This trusted value is used only to assign legacy v1 records during the
    // versionchange transaction. Normal operations resolve current() again.
    const migrationWorkspaceId = await resolveCurrentWorkspaceId();

    await new Promise<void>((resolve, reject) => {
      let request: IDBOpenDBRequest;
      try {
        request = indexedDB.open(DB_NAME, RECOVERY_DB_VERSION);
      } catch (error) {
        reject(error);
        return;
      }

      request.onerror = () => reject(request.error);
      request.onblocked = () => reject(new Error('IndexedDB upgrade is blocked by another window'));
      request.onsuccess = () => {
        this.db = request.result;
        this.db.onversionchange = () => {
          this.db?.close();
          this.db = null;
          this.initPromise = null;
        };
        resolve();
      };
      request.onupgradeneeded = event => {
        const db = request.result;
        const transaction = request.transaction;
        if (!transaction) {
          reject(new Error('IndexedDB upgrade transaction is unavailable'));
          return;
        }

        try {
          createV2Stores(db);
          if (event.oldVersion < RECOVERY_DB_VERSION) {
            migrateLegacyV1Stores(db, transaction, migrationWorkspaceId);
          }
        } catch {
          transaction.abort();
        }
      };
    });
  }

  private async scopedDatabase(): Promise<{ db: IDBDatabase; workspaceId: string }> {
    await this.init();
    const workspaceId = await resolveCurrentWorkspaceId();
    if (!this.db) throw new Error('IndexedDB is not initialized');
    return { db: this.db, workspaceId };
  }

  async saveMeetingMetadata(metadata: MeetingMetadataInput | MeetingMetadata): Promise<void> {
    try {
      const { db, workspaceId } = await this.scopedDatabase();
      const transaction = db.transaction(MEETINGS_STORE, 'readwrite');
      transaction.objectStore(MEETINGS_STORE).put(buildScopedMeetingRecord(metadata, workspaceId));
      await this.waitForTransaction(transaction);
    } catch (error) {
      console.warn('Failed to save meeting metadata to IndexedDB:', error);
    }
  }

  async getMeetingMetadata(meetingId: string): Promise<MeetingMetadata | null> {
    try {
      const { db, workspaceId } = await this.scopedDatabase();
      const transaction = db.transaction(MEETINGS_STORE, 'readonly');
      const request = transaction.objectStore(MEETINGS_STORE).get(meetingKey(workspaceId, meetingId));
      const result = await this.waitForRequest<MeetingMetadata | undefined>(request);
      return result?.workspaceId === workspaceId ? result : null;
    } catch (error) {
      console.error('Failed to get meeting metadata from IndexedDB:', error);
      return null;
    }
  }

  async getAllMeetings(): Promise<MeetingMetadata[]> {
    try {
      const { db, workspaceId } = await this.scopedDatabase();
      const transaction = db.transaction(MEETINGS_STORE, 'readonly');
      const index = transaction.objectStore(MEETINGS_STORE).index(MEETING_WORKSPACE_SAVED_INDEX);
      const request = index.getAll(IDBKeyRange.only([workspaceId, 'pending']));
      const meetings = recordsForWorkspace(
        await this.waitForRequest<MeetingMetadata[]>(request),
        workspaceId,
      );
      return meetings.sort((a, b) => b.lastUpdated - a.lastUpdated);
    } catch (error) {
      console.error('Failed to get meetings from IndexedDB:', error);
      return [];
    }
  }

  async markMeetingSaved(meetingId: string): Promise<void> {
    const { db, workspaceId } = await this.scopedDatabase();
    try {
      const transaction = db.transaction(MEETINGS_STORE, 'readwrite');
      const transactionComplete = this.waitForTransaction(transaction);
      const store = transaction.objectStore(MEETINGS_STORE);
      const request = store.get(meetingKey(workspaceId, meetingId));

      request.onsuccess = () => {
        const meeting = request.result as MeetingMetadata | undefined;
        if (meeting?.workspaceId === workspaceId) {
          store.put(buildScopedMeetingRecord({
            ...meeting,
            savedToSQLite: true,
            lastUpdated: Date.now(),
          }, workspaceId));
        }
      };

      await transactionComplete;
      await this.deleteMeetingForWorkspace(db, workspaceId, meetingId);
    } catch (error) {
      console.warn('Failed to mark meeting as saved:', error);
      throw error;
    }
  }

  async deleteMeeting(meetingId: string): Promise<void> {
    const { db, workspaceId } = await this.scopedDatabase();
    await this.deleteMeetingForWorkspace(db, workspaceId, meetingId);
  }

  private async deleteMeetingForWorkspace(
    db: IDBDatabase,
    workspaceId: string,
    meetingId: string,
  ): Promise<void> {
    const transaction = db.transaction([MEETINGS_STORE, TRANSCRIPTS_STORE], 'readwrite');
    const meetings = transaction.objectStore(MEETINGS_STORE);
    const transcripts = transaction.objectStore(TRANSCRIPTS_STORE);
    this.queueTranscriptDeletion(transcripts, workspaceId, meetingId);
    meetings.delete(meetingKey(workspaceId, meetingId));
    await this.waitForTransaction(transaction);
  }

  async saveTranscript(meetingId: string, transcript: object): Promise<void> {
    try {
      const { db, workspaceId } = await this.scopedDatabase();
      const transaction = db.transaction([TRANSCRIPTS_STORE, MEETINGS_STORE], 'readwrite');
      const transactionComplete = this.waitForTransaction(transaction);
      const transcripts = transaction.objectStore(TRANSCRIPTS_STORE);
      const meetings = transaction.objectStore(MEETINGS_STORE);
      const meetingRequest = meetings.get(meetingKey(workspaceId, meetingId));
      let missingMeeting = false;

      meetingRequest.onsuccess = () => {
        const meeting = meetingRequest.result as MeetingMetadata | undefined;
        if (!meeting || meeting.workspaceId !== workspaceId) {
          missingMeeting = true;
          transaction.abort();
          return;
        }

        const scopedTranscript = buildScopedTranscriptRecord(meetingId, transcript, workspaceId);
        const transcriptRequest = transcripts.get(transcriptKey(
          workspaceId,
          meetingId,
          scopedTranscript.recoveryId,
        ));
        transcriptRequest.onsuccess = () => {
          const alreadyStored = transcriptRequest.result !== undefined;
          transcripts.put(scopedTranscript);
          meetings.put(buildScopedMeetingRecord({
            ...meeting,
            lastUpdated: Date.now(),
            transcriptCount: meeting.transcriptCount + (alreadyStored ? 0 : 1),
          }, workspaceId));
        };
      };

      try {
        await transactionComplete;
      } catch (error) {
        if (missingMeeting) throw new Error('current-workspace recovery meeting was not found');
        throw error;
      }
    } catch (error) {
      console.warn('Failed to save transcript to IndexedDB:', error);
    }
  }

  async getTranscripts(meetingId: string): Promise<StoredTranscript[]> {
    try {
      const { db, workspaceId } = await this.scopedDatabase();
      const transaction = db.transaction(TRANSCRIPTS_STORE, 'readonly');
      const index = transaction
        .objectStore(TRANSCRIPTS_STORE)
        .index(TRANSCRIPT_WORKSPACE_MEETING_INDEX);
      const request = index.getAll(IDBKeyRange.only(workspaceMeetingKey(workspaceId, meetingId)));
      const transcripts = recordsForWorkspace(
        await this.waitForRequest<StoredTranscript[]>(request),
        workspaceId,
      );
      return transcripts.sort((a, b) => a.sequenceId - b.sequenceId);
    } catch (error) {
      console.error('Failed to get transcripts from IndexedDB:', error);
      return [];
    }
  }

  async getTranscriptCount(meetingId: string): Promise<number> {
    try {
      const { db, workspaceId } = await this.scopedDatabase();
      const transaction = db.transaction(TRANSCRIPTS_STORE, 'readonly');
      const index = transaction
        .objectStore(TRANSCRIPTS_STORE)
        .index(TRANSCRIPT_WORKSPACE_MEETING_INDEX);
      return await this.waitForRequest<number>(
        index.count(IDBKeyRange.only(workspaceMeetingKey(workspaceId, meetingId))),
      );
    } catch (error) {
      console.error('Failed to get transcript count from IndexedDB:', error);
      return 0;
    }
  }

  async purgeSavedMeetings(): Promise<number> {
    const { db, workspaceId } = await this.scopedDatabase();
    try {
      const meetings = await this.getStoredMeetings(db, workspaceId, 'saved');
      for (const meeting of meetings) {
        await this.deleteMeetingForWorkspace(db, workspaceId, meeting.meetingId);
      }
      return meetings.length;
    } catch (error) {
      console.error('Failed to purge saved meetings from IndexedDB:', error);
      throw error;
    }
  }

  async deleteOldMeetings(daysOld: number): Promise<number> {
    try {
      const { db, workspaceId } = await this.scopedDatabase();
      const cutoffTime = Date.now() - (daysOld * 24 * 60 * 60 * 1000);
      const meetings = await this.getStoredMeetings(db, workspaceId);
      const expired = meetings.filter(meeting => meeting.lastUpdated < cutoffTime);

      for (const meeting of expired) {
        await this.deleteMeetingForWorkspace(db, workspaceId, meeting.meetingId);
      }
      return expired.length;
    } catch (error) {
      console.error('Failed to delete old meetings:', error);
      return 0;
    }
  }

  async deleteSavedMeetings(_hoursOld: number): Promise<number> {
    return this.purgeSavedMeetings();
  }

  private queueTranscriptDeletion(
    transcriptsStore: IDBObjectStore,
    workspaceId: string,
    meetingId: string,
  ): void {
    const index = transcriptsStore.index(TRANSCRIPT_WORKSPACE_MEETING_INDEX);
    const request = index.openCursor(IDBKeyRange.only(workspaceMeetingKey(workspaceId, meetingId)));
    request.onsuccess = () => {
      const cursor = request.result;
      if (cursor) {
        cursor.delete();
        cursor.continue();
      }
    };
  }

  private async getStoredMeetings(
    db: IDBDatabase,
    workspaceId: string,
    savedState?: SavedState,
  ): Promise<MeetingMetadata[]> {
    const transaction = db.transaction(MEETINGS_STORE, 'readonly');
    const store = transaction.objectStore(MEETINGS_STORE);
    const request = savedState
      ? store.index(MEETING_WORKSPACE_SAVED_INDEX)
        .getAll(IDBKeyRange.only([workspaceId, savedState]))
      : store.index(MEETING_WORKSPACE_INDEX).getAll(IDBKeyRange.only(workspaceId));
    return recordsForWorkspace(
      await this.waitForRequest<MeetingMetadata[]>(request),
      workspaceId,
    );
  }

  private waitForRequest<T>(request: IDBRequest): Promise<T> {
    return new Promise((resolve, reject) => {
      request.onsuccess = () => resolve(request.result as T);
      request.onerror = () => reject(request.error);
    });
  }

  private waitForTransaction(transaction: IDBTransaction): Promise<void> {
    return new Promise((resolve, reject) => {
      transaction.oncomplete = () => resolve();
      transaction.onerror = () => reject(
        transaction.error ?? new Error('IndexedDB transaction failed'),
      );
      transaction.onabort = () => reject(
        transaction.error ?? new Error('IndexedDB transaction was aborted'),
      );
    });
  }
}

export const indexedDBService = new IndexedDBService();
