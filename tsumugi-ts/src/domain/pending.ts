// PendingItem — runtime shape matching `tsumugi-core::domain::PendingItem`.

import type { ChunkId, PendingItemId } from './ids.js';

export type Priority = 'Low' | 'Medium' | 'High';

export interface PendingItem {
  readonly id: PendingItemId;
  readonly kind: string;
  readonly description: string;
  readonly introduced_at: ChunkId;
  readonly expected_resolution_chunk: ChunkId | null;
  readonly resolved_at: ChunkId | null;
  readonly priority: Priority;
}

export function isResolved(item: PendingItem): boolean {
  return item.resolved_at !== null;
}
