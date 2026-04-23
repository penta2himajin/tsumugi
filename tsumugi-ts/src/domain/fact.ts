// Fact — runtime shape matching `tsumugi-core::domain::Fact`.

import type { ChunkId, FactId } from './ids.js';

export type FactScope =
  | { readonly kind: 'Global' }
  | { readonly kind: 'ChunkLocal'; readonly chunk: ChunkId };

export type FactOrigin = 'User' | 'Extracted' | 'Derived';

export interface Fact {
  readonly id: FactId;
  readonly key: string;
  readonly value: string;
  readonly scope: FactScope;
  readonly superseded_by: FactId | null;
  readonly created_at: string;
  readonly origin: FactOrigin;
}

export function isActive(fact: Fact): boolean {
  return fact.superseded_by === null;
}

export const FactScope = {
  global(): FactScope {
    return { kind: 'Global' };
  },
  chunkLocal(chunk: ChunkId): FactScope {
    return { kind: 'ChunkLocal', chunk };
  },
};
