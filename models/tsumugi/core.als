module tsumugi/core

-- Domain-agnostic core types. No dependency on creative (novel / TRPG /
-- game-script) concepts — those live in `tsumugi/creative`.
--
-- Corresponds to `tsumugi-core/src/domain.rs` on the Rust side (default
-- features). Hand-written extensions live there; types and invariants are
-- derived from this model.
--
-- Runtime-only flags (`edited_by_user`, `auto_update_locked`) are NOT
-- modeled here — they live on the Rust struct as `bool` and guard UX
-- behavior, not structural consistency.

-------------------------------------------------------------------------------
-- Opaque value sigs — payload / content details modeled on the Rust side.
-------------------------------------------------------------------------------

sig Item {}  -- serialized domain event carried by a raw-leaf Chunk

-------------------------------------------------------------------------------
-- Chunk: hierarchical narrative unit (0 = raw leaf, 1+ = summary node)
-------------------------------------------------------------------------------

sig Chunk {
  parent:          lone Chunk,
  children:        set Chunk,
  items:           set Item,
  facts:           set Fact,
  pending:         set PendingItem,
  source_location: lone SourceLocationValue,
  summary_method:  lone SummaryMethod,
  summary_level:   one Int
}

-------------------------------------------------------------------------------
-- SourceLocation: B 案 (2026-04-23 確定) — sum type with File + Custom variants.
-- Rust side maps to `enum SourceLocationValue { File(...), Custom { ... } }`.
-------------------------------------------------------------------------------

abstract sig SourceLocationValue {}

sig File extends SourceLocationValue {}

sig Custom extends SourceLocationValue {}

-------------------------------------------------------------------------------
-- SummaryMethod: enumeration of supported summary generation methods.
-- `NoMethod` marks an unsummarized raw leaf (summary_level = 0).
-------------------------------------------------------------------------------

abstract sig SummaryMethod {}
one sig LlmFull          extends SummaryMethod {}
one sig LlmLingua2       extends SummaryMethod {}
one sig SelectiveContext extends SummaryMethod {}
one sig ExtractiveBM25   extends SummaryMethod {}
one sig UserManual       extends SummaryMethod {}
one sig NoMethod         extends SummaryMethod {}

-------------------------------------------------------------------------------
-- Fact: stored assertion with supersession chain.
-------------------------------------------------------------------------------

sig Fact {
  scope:         one FactScope,
  superseded_by: lone Fact,
  origin:        one FactOrigin
}

abstract sig FactScope {}
one sig GlobalScope extends FactScope {}
sig ChunkLocalScope extends FactScope {
  scope_chunk: one Chunk
}

abstract sig FactOrigin {}
one sig UserOrigin      extends FactOrigin {}
one sig ExtractedOrigin extends FactOrigin {}
one sig DerivedOrigin   extends FactOrigin {}

-------------------------------------------------------------------------------
-- PendingItem: open items (plot threads, TODOs, unresolved clues).
-------------------------------------------------------------------------------

sig PendingItem {
  introduced_at:              one Chunk,
  expected_resolution_chunk:  lone Chunk,
  resolved_at:                lone Chunk,
  priority:                   one Priority
}

abstract sig Priority {}
one sig LowPriority    extends Priority {}
one sig MediumPriority extends Priority {}
one sig HighPriority   extends Priority {}

-------------------------------------------------------------------------------
-- Hierarchy invariants
-------------------------------------------------------------------------------

fact ParentToChildLink {
  all c: Chunk | all p: c.parent | c in p.children
}

fact ChildToParentLink {
  all p: Chunk | all c: p.children | p in c.parent
}

fact NoCyclicParent {
  no c: Chunk | c in c.^parent
}

-------------------------------------------------------------------------------
-- Hierarchical summary invariants (docs/tech-architecture.md §Chunk)
-------------------------------------------------------------------------------

fact RawLeafHasItems {
  all c: Chunk | c.summary_level = 0 implies some c.items
}

fact SummaryNodeHasChildren {
  all c: Chunk | c.summary_level > 0 implies some c.children
}

fact ParentMoreAbstractThanChild {
  all c: Chunk | all p: c.parent | p.summary_level > c.summary_level
}

-- Decision A (docs/tech-architecture.md §Phase 1 型定義時に決める実装判断):
-- summary_level = 0 implies summary_method = NoMethod (unsummarized raw leaf).
fact RawLeafHasNoMethod {
  all c: Chunk | c.summary_level = 0 implies c.summary_method = NoMethod
}

fact SummaryNodeHasMethod {
  all c: Chunk | c.summary_level > 0 implies c.summary_method != NoMethod
}

-------------------------------------------------------------------------------
-- Fact supersession invariant
-------------------------------------------------------------------------------

-- Trivial direct fact on `superseded_by` (silences UnconstrainedTransitivity
-- for the `^superseded_by` closure below). Follows the oxidtr self-host
-- convention (cf. IRParentAsymmetric in oxidtr/models/oxidtr-split.als).
fact SupersededByDirect {
  all f: Fact | f.superseded_by = f.superseded_by
}

fact NoCyclicSupersession {
  no f: Fact | f in f.^superseded_by
}

-------------------------------------------------------------------------------
-- PendingItem ↔ Chunk.pending inverse (introducer link)
--
-- `expected_resolution_chunk` and `resolved_at` intentionally do NOT mirror
-- into Chunk.pending — they are auxiliary references (hints / resolution
-- pointers), not ownership links. oxidtr reports these as MissingInverse
-- warnings which are accepted as false positives for this design.
-------------------------------------------------------------------------------

fact PendingItemIntroducerLink {
  all pi: PendingItem | pi in pi.introduced_at.pending
}

-------------------------------------------------------------------------------
-- Variant usage markers — silence UnhandledResponsePattern warnings for
-- data-carrying sum-type variants. These variants are dispatched on the
-- Rust side via pattern matching; Alloy requires a direct mention so the
-- "unhandled response" heuristic does not fire.
-------------------------------------------------------------------------------

pred useFile           [v: File]           { v = v }
pred useCustom         [v: Custom]         { v = v }
pred useGlobalScope    [v: GlobalScope]    { v = v }
pred useChunkLocalScope[v: ChunkLocalScope] { v = v }

-------------------------------------------------------------------------------
-- PendingItem lifecycle invariants
--
-- TODO(Phase 0 refinement): encode `introduced_at ≤ resolved_at` once temporal
-- ordering is modeled on Chunk (e.g., via a creation-order relation or a
-- dedicated Time sig). Currently only the structural links are expressed.
-------------------------------------------------------------------------------

-------------------------------------------------------------------------------
-- Cardinality tautologies — silence UnconstrainedCardinality warnings for
-- fields that are intentionally unbounded (tsumugi is scale-agnostic).
-- Follows the oxidtr self-host convention (see oxidtr/models/oxidtr/ast.als).
-------------------------------------------------------------------------------

fact ChunkChildrenUnbounded { all c: Chunk | #c.children = #c.children }
fact ChunkItemsUnbounded    { all c: Chunk | #c.items    = #c.items }
fact ChunkFactsUnbounded    { all c: Chunk | #c.facts    = #c.facts }
fact ChunkPendingUnbounded  { all c: Chunk | #c.pending  = #c.pending }
