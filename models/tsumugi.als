module tsumugi

-- Main module. Aggregates the core (domain-agnostic) and creative
-- (novel / TRPG / game-script) sub-modules and hosts any cross-module
-- invariants. oxidtr resolves `open` directives transitively and emits
-- one Rust module per `module` declaration.
--
-- See docs/concept.md and docs/tech-architecture.md for the design.

open tsumugi/core
open tsumugi/creative

-------------------------------------------------------------------------------
-- Cross-module invariants (core + creative)
-------------------------------------------------------------------------------

-- Character.first_appearance must point to a Chunk that actually exists in the
-- model (trivially enforced by Alloy's relation semantics — kept explicit for
-- documentation / future extension).
fact CharacterFirstAppearanceWellFormed {
  all ch: Character | all fa: ch.first_appearance | fa in Chunk
}
