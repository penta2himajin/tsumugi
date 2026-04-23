// Chunk — runtime shape matching `tsumugi-core::domain::Chunk`.
//
// Includes all the hand-written fields the Rust side adds on top of the Alloy
// relational skeleton (strings, keywords, metadata, timestamps, flags).

import type { ChunkId, FactId, PendingItemId } from './ids.js';
import type { SourceLocationValue } from './source-location.js';
import type { SummaryMethod } from './summary.js';

export type Keyword = string;

export interface Chunk {
  readonly id: ChunkId;
  readonly text: string;
  readonly items: readonly unknown[];
  readonly summary: string;
  readonly keywords: readonly Keyword[];
  readonly facts: readonly FactId[];
  readonly pending: readonly PendingItemId[];
  readonly parent: ChunkId | null;
  readonly children: readonly ChunkId[];
  readonly metadata: Readonly<Record<string, unknown>>;
  /** ISO 8601 timestamp string (serde serializes `DateTime<Utc>` as ISO 8601). */
  readonly last_active_at: string;
  readonly order_in_parent: number;
  readonly source_location: SourceLocationValue | null;
  /** 0 = raw leaf, positive = summary abstraction level. */
  readonly summary_level: number;
  readonly summary_method: SummaryMethod;
  readonly edited_by_user: boolean;
  readonly auto_update_locked: boolean;
}

export function isRawLeaf(chunk: Chunk): boolean {
  return chunk.summary_level === 0;
}

export function isSummaryNode(chunk: Chunk): boolean {
  return chunk.summary_level > 0;
}

/**
 * Validate the hierarchical-summary invariants. Returns a message on violation
 * so callers can decide between throwing and surfacing in the UI.
 */
export function validateSummaryInvariants(chunk: Chunk): string | null {
  if (isRawLeaf(chunk)) {
    if (chunk.summary_method !== 'None') {
      return 'raw leaf (summary_level = 0) must have SummaryMethod "None"';
    }
  } else {
    if (chunk.children.length === 0) {
      return 'summary node (summary_level > 0) must have children';
    }
    if (chunk.summary_method === 'None') {
      return 'summary node must have a non-None SummaryMethod';
    }
  }
  return null;
}
