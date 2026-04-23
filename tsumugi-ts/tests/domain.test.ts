import { describe, expect, it } from 'vitest';

import {
  asSourceLocation,
  ChunkId,
  FactId,
  FactScope,
  isActive,
  isRawLeaf,
  isRawLeafMethod,
  isResolved,
  isSummaryNode,
  isUuid,
  SourceLocationValue,
  validateSummaryInvariants,
} from '../src/index.js';
import type { Chunk, Fact, PendingItem } from '../src/index.js';

const UUID_A = '11111111-2222-3333-4444-555555555555';
const UUID_B = '11111111-2222-3333-4444-555555555556';

describe('ids', () => {
  it('accepts canonical UUIDs', () => {
    expect(isUuid(UUID_A)).toBe(true);
    expect(() => ChunkId.from(UUID_A)).not.toThrow();
  });

  it('rejects malformed strings', () => {
    expect(isUuid('not-a-uuid')).toBe(false);
    expect(() => ChunkId.from('not-a-uuid')).toThrow(TypeError);
  });
});

describe('source location', () => {
  it('File variant has schema = "file"', () => {
    const v = SourceLocationValue.file('src/a/b.rs');
    const loc = asSourceLocation(v);
    expect(loc.schema).toBe('file');
    expect(loc.path).toBe('src/a/b.rs');
  });

  it('file proximity is 1.0 for identical paths', () => {
    const a = asSourceLocation(SourceLocationValue.file('src/a/b.rs'));
    const b = asSourceLocation(SourceLocationValue.file('src/a/b.rs'));
    expect(a.proximity(b)).toBe(1.0);
  });

  it('file proximity falls off with path distance', () => {
    const a = asSourceLocation(SourceLocationValue.file('src/a/b.rs'));
    const b = asSourceLocation(SourceLocationValue.file('src/a/c.rs'));
    const c = asSourceLocation(SourceLocationValue.file('docs/readme.md'));
    expect(a.proximity(b)).toBeGreaterThan(0.5);
    expect(a.proximity(b)).toBeLessThan(1.0);
    expect(a.proximity(c)).toBe(0);
  });

  it('cross-schema proximity is 0', () => {
    const file = asSourceLocation(SourceLocationValue.file('src/a.rs'));
    const custom = asSourceLocation(
      SourceLocationValue.custom('trpg-session', { id: 1 }),
    );
    expect(file.proximity(custom)).toBe(0);
    expect(custom.proximity(file)).toBe(0);
  });
});

describe('summary invariants', () => {
  const baseChunk: Chunk = {
    id: ChunkId.from(UUID_A),
    text: 'x',
    items: [{}],
    summary: '',
    keywords: [],
    facts: [],
    pending: [],
    parent: null,
    children: [],
    metadata: {},
    last_active_at: new Date().toISOString(),
    order_in_parent: 0,
    source_location: null,
    summary_level: 0,
    summary_method: 'None',
    edited_by_user: false,
    auto_update_locked: false,
  };

  it('raw leaf with SummaryMethod=None is valid', () => {
    expect(validateSummaryInvariants(baseChunk)).toBeNull();
    expect(isRawLeaf(baseChunk)).toBe(true);
    expect(isSummaryNode(baseChunk)).toBe(false);
    expect(isRawLeafMethod(baseChunk.summary_method)).toBe(true);
  });

  it('summary node without children is rejected', () => {
    const invalid: Chunk = { ...baseChunk, summary_level: 1, summary_method: 'ExtractiveBM25' };
    expect(validateSummaryInvariants(invalid)).toContain('children');
  });

  it('summary node with None method is rejected', () => {
    const invalid: Chunk = {
      ...baseChunk,
      summary_level: 1,
      summary_method: 'None',
      children: [ChunkId.from(UUID_B)],
    };
    expect(validateSummaryInvariants(invalid)).toContain('non-None');
  });

  it('raw leaf with non-None method is rejected', () => {
    const invalid: Chunk = { ...baseChunk, summary_method: 'ExtractiveBM25' };
    expect(validateSummaryInvariants(invalid)).toContain('None');
  });
});

describe('fact state', () => {
  const activeFact: Fact = {
    id: FactId.from(UUID_A),
    key: 'hp',
    value: '12',
    scope: FactScope.global(),
    superseded_by: null,
    created_at: new Date().toISOString(),
    origin: 'User',
  };

  it('fact with no successor is active', () => {
    expect(isActive(activeFact)).toBe(true);
  });

  it('fact with successor is inactive', () => {
    const superseded: Fact = { ...activeFact, superseded_by: FactId.from(UUID_B) };
    expect(isActive(superseded)).toBe(false);
  });

  it('chunkLocal scope carries the chunk', () => {
    const scope = FactScope.chunkLocal(ChunkId.from(UUID_A));
    expect(scope.kind).toBe('ChunkLocal');
    if (scope.kind === 'ChunkLocal') {
      expect(scope.chunk).toBe(UUID_A);
    }
  });
});

describe('pending item', () => {
  it('unresolved item reports not resolved', () => {
    const item: PendingItem = {
      id: FactId.from(UUID_A) as unknown as import('../src/index.js').PendingItemId,
      kind: 'plot',
      description: 'find the key',
      introduced_at: ChunkId.from(UUID_A),
      expected_resolution_chunk: null,
      resolved_at: null,
      priority: 'High',
    };
    expect(isResolved(item)).toBe(false);
  });
});
