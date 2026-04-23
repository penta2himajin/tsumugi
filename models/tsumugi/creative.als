module tsumugi/creative

-- Creative-domain extension sigs. Gated behind the Rust feature flag
-- `creative` (暫定名) on the implementation side. Depends on tsumugi/core.

open tsumugi/core

-------------------------------------------------------------------------------
-- Character: participant / narrator / NPC / PC
-------------------------------------------------------------------------------

sig Character {
  speech_traits:     lone SpeechTraits,
  first_appearance:  lone Chunk
}

sig SpeechTraits {
  formality: one Formality
}

abstract sig Formality {}
one sig Casual   extends Formality {}
one sig Neutral  extends Formality {}
one sig Formal   extends Formality {}

-------------------------------------------------------------------------------
-- SceneView: specialized read-only view onto a Chunk
-------------------------------------------------------------------------------

sig SceneView {
  viewed_chunk:  one Chunk,
  participants:  set Character
}

-------------------------------------------------------------------------------
-- StylePreset: writing style configuration
-------------------------------------------------------------------------------

sig StylePreset {
  pov:       one PoV,
  tense:     one Tense,
  formality: one Formality
}

abstract sig PoV {}
one sig FirstPerson  extends PoV {}
one sig SecondPerson extends PoV {}
one sig ThirdPerson  extends PoV {}

abstract sig Tense {}
one sig PresentTense extends Tense {}
one sig PastTense    extends Tense {}

-------------------------------------------------------------------------------
-- LoreEntry: lorebook keyword-triggered reference (moved from core, 2026-04-23)
-------------------------------------------------------------------------------

sig LoreEntry {
  scope: one LoreScope
}

abstract sig LoreScope {}
one sig LoreGlobal       extends LoreScope {}
sig LoreChunkLocal       extends LoreScope { lore_chunk: one Chunk }
-- Conditional scope carries an opaque predicate string — non-empty check
-- is enforced on the Rust side (newtype with TryFrom).
sig LoreConditional      extends LoreScope {}

-------------------------------------------------------------------------------
-- Invariants
--
-- TODO(Phase 0 refinement): LoreEntry.scope Conditional non-empty invariant
-- — currently deferred to the Rust newtype (`ConditionalExpr::new`). Consider
-- whether to express it structurally here once opaque-string handling is
-- stabilized.
-------------------------------------------------------------------------------

-- Cardinality tautology — scale-agnostic on participants (see core.als).
fact SceneViewParticipantsUnbounded {
  all sv: SceneView | #sv.participants = #sv.participants
}

-------------------------------------------------------------------------------
-- Usage markers — declare these standalone sigs as "used by the creative
-- surface" so oxidtr's UnreferencedSig warning is suppressed. They are
-- referenced through the context compiler on the Rust side, not structurally
-- from other sigs. Follows the oxidtr self-host convention
-- (see oxidtr/models/oxidtr-split.als `pred useX[x: X] { x = x }`).
-------------------------------------------------------------------------------

pred useSceneView     [sv: SceneView]      { sv = sv }
pred useStylePreset   [sp: StylePreset]    { sp = sp }
pred useLoreEntry     [le: LoreEntry]      { le = le }
pred useLoreGlobal    [v: LoreGlobal]      { v = v }
pred useLoreChunkLocal[v: LoreChunkLocal]  { v = v }
pred useLoreConditional[v: LoreConditional] { v = v }
