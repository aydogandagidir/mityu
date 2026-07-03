/**
 * Database Service
 *
 * Handles local-database-related Tauri backend calls.
 * Pure 1-to-1 wrapper - no error handling changes, exact same behavior as direct invoke calls.
 */

import { invoke } from '@tauri-apps/api/core';

/**
 * At-rest encryption status of the currently-open local database (ADR-0014).
 * Kept as an object (not a bare boolean) to mirror the Rust `DbEncryptionStatus`
 * struct so it can grow without breaking callers.
 */
export interface DbEncryptionStatus {
  /**
   * `true` = the DB opened with the SQLCipher key (encrypted at rest);
   * `false` = the fail-open plaintext branch ran because the keychain key was
   * unavailable on a plaintext/fresh DB — the UI should warn the user.
   */
  encrypted: boolean;
}

/**
 * Database Service
 * Singleton service for local-database status/introspection operations
 */
export class DbService {
  /**
   * Report whether the local database opened ENCRYPTED at rest or fell back to
   * PLAINTEXT (ADR-0014 follow-up).
   * @returns Promise with { encrypted }
   */
  async getDbEncryptionStatus(): Promise<DbEncryptionStatus> {
    return invoke<DbEncryptionStatus>('get_db_encryption_status');
  }
}

// Export singleton instance
export const dbService = new DbService();

/**
 * Typed convenience wrapper.
 * @returns Promise with { encrypted }
 */
export function getDbEncryptionStatus(): Promise<DbEncryptionStatus> {
  return dbService.getDbEncryptionStatus();
}
