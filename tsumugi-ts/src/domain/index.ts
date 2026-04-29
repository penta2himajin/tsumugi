export { ChunkId, FactId, PendingItemId, isUuid } from './ids.js';
export type { Chunk, Keyword } from './chunk.js';
export { isRawLeaf, isSummaryNode, validateSummaryInvariants } from './chunk.js';
export type { Fact, FactOrigin } from './fact.js';
export { FactScope, isActive } from './fact.js';
export type { PendingItem, Priority } from './pending.js';
export { isResolved } from './pending.js';
export type {
  FileSourceLocation,
  SourceLocation,
  Span,
} from './source-location.js';
export { SourceLocationValue, asSourceLocation } from './source-location.js';
export type { SummaryMethod } from './summary.js';
export { isRawLeafMethod } from './summary.js';
