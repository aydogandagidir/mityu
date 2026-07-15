/**
 * Typed local bridge for the read-only Approved Action Center.
 *
 * The backend is the authority for tenant scoping, review-state filtering,
 * ordering, pagination, and source resolvability. Callers preserve its order.
 */

import { invoke } from '@tauri-apps/api/core';

export interface ApprovedActionItem {
  id: string;
  meetingId: string;
  meetingTitle: string;
  meetingCreatedAt: string;
  text: string;
  assignee: string | null;
  due: string | null;
  /** HITL review state. This is intentionally not a work-progress state. */
  reviewStatus: 'approved';
  sourceChunkId: string;
  sourceTimestamp: string;
  audioStartTime: number | null;
}

export interface ApprovedActionItemsPage {
  items: ApprovedActionItem[];
  hasMore: boolean;
  nextOffset: number | null;
}

export const ACTION_CENTER_PAGE_SIZE = 100;

/** Load one backend-ordered page of active, human-approved actions. */
export function listApprovedActionItems(
  offset = 0,
  limit = ACTION_CENTER_PAGE_SIZE,
): Promise<ApprovedActionItemsPage> {
  return invoke<ApprovedActionItemsPage>('api_list_approved_action_items', {
    limit,
    offset,
  });
}
