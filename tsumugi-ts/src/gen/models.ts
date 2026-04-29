export interface Item {}

/**
 * @invariant ParentToChildLink
 * @invariant ChildToParentLink
 * @invariant NoCyclicParent
 * @invariant RawLeafHasItems
 * @invariant SummaryNodeHasChildren
 * @invariant ParentMoreAbstractThanChild
 * @invariant RawLeafHasNoMethod
 * @invariant SummaryNodeHasMethod
 * @invariant HappensBeforeTransitive
 * @invariant NoCyclicHappensBefore
 * @invariant ChunkChildrenUnbounded
 * @invariant ChunkItemsUnbounded
 * @invariant ChunkFactsUnbounded
 * @invariant ChunkPendingUnbounded
 * @invariant ChunkHappensBeforeUnbounded
 */
export interface Chunk {
  readonly parent: Chunk | null;
  readonly children: Set<Chunk>;
  readonly items: Set<Item>;
  readonly facts: Set<Fact>;
  readonly pending: Set<PendingItem>;
  readonly source_location: SourceLocationValue | null;
  readonly summary_method: SummaryMethod | null;
  readonly summary_level: number;
  readonly happens_before: Set<Chunk>;
}

export type SourceLocationValue = "File" | "Custom";

export type SummaryMethod = "LlmFull" | "LlmLingua2" | "SelectiveContext" | "ExtractiveBM25" | "UserManual" | "NoMethod";

/**
 * @invariant SupersededByDirect
 * @invariant NoCyclicSupersession
 */
export interface Fact {
  readonly scope: FactScope;
  readonly superseded_by: Fact | null;
  readonly origin: FactOrigin;
}

export interface GlobalScope {
  readonly kind: "GlobalScope";
}

export interface ChunkLocalScope {
  readonly kind: "ChunkLocalScope";
  readonly scope_chunk: Chunk;
}

export type FactScope = GlobalScope | ChunkLocalScope;

export type FactOrigin = "UserOrigin" | "ExtractedOrigin" | "DerivedOrigin";

/**
 * @invariant PendingItemIntroducerLink
 * @invariant PendingItemResolvedAtAfterIntroduction
 * @invariant PendingItemExpectedResolutionAfterIntroduction
 */
export interface PendingItem {
  readonly introduced_at: Chunk;
  readonly expected_resolution_chunk: Chunk | null;
  readonly resolved_at: Chunk | null;
  readonly priority: Priority;
}

export type Priority = "LowPriority" | "MediumPriority" | "HighPriority";

