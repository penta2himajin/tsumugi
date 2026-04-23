module tsumugi/core

-- Domain-agnostic core types. No dependency on creative (novel / TRPG /
-- game-script) concepts — those live in `tsumugi/creative`.
--
-- Corresponds to `tsumugi-core/src/domain.rs` on the Rust side (default
-- features). Hand-written extensions live there; types and invariants are
-- derived from this model.

-------------------------------------------------------------------------------
-- Opaque value sigs — payload / content details modeled on the Rust side.
-------------------------------------------------------------------------------

sig Item {}  -- serialized domain event carried by a raw-leaf Chunk

-------------------------------------------------------------------------------
-- Chunk: hierarchical narrative unit (0 = raw leaf, 1+ = summary node)
-------------------------------------------------------------------------------

sig Chunk {
  parent:              lone Chunk,
  children:            set Chunk,
  items:               set Item,
  facts:               set Fact,
  pending:             set PendingItem,
  source_location:     lone SourceLocationValue,
  summary_method:      lone SummaryMethod,
  summary_level:       one Int,
  edited_by_user:      lone Chunk,
  auto_update_locked:  lone Chunk
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

fact NoCyclicSupersession {
  no f: Fact | f in f.^superseded_by
}

-------------------------------------------------------------------------------
-- PendingItem lifecycle invariants
--
-- TODO(Phase 0 refinement): encode `introduced_at ≤ resolved_at` once temporal
-- ordering is modeled on Chunk (e.g., via a creation-order relation or a
-- dedicated Time sig). Currently only the structural links are expressed.
-------------------------------------------------------------------------------
