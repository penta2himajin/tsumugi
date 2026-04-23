import type * as M from './models';

/** Transitive closure traversal for Chunk.happens_before. */
export function tcHappens_before(start: M.Chunk): M.Chunk[] {
  const result: M.Chunk[] = [];
  const queue: M.Chunk[] = [...start.happens_before];
  while (queue.length > 0) {
    const next = queue.pop()!;
    if (!result.includes(next)) {
      result.push(next);
      queue.push(...next.happens_before);
    }
  }
  return result;
}

/** Transitive closure traversal for Chunk.parent. */
export function tcParent(start: M.Chunk): M.Chunk[] {
  const result: M.Chunk[] = [];
  let current: M.Chunk | null = start.parent;
  while (current !== null) {
    result.push(current);
    current = current.parent;
  }
  return result;
}

/** Transitive closure traversal for Fact.superseded_by. */
export function tcSuperseded_by(start: M.Fact): M.Fact[] {
  const result: M.Fact[] = [];
  let current: M.Fact | null = start.superseded_by;
  while (current !== null) {
    result.push(current);
    current = current.superseded_by;
  }
  return result;
}

