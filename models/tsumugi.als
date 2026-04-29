module tsumugi

-- Top-level module. Currently aggregates only the domain-agnostic core.
-- oxidtr resolves `open` directives transitively and emits one Rust /
-- TypeScript module per `module` declaration.
--
-- See docs/concept.md and docs/tech-architecture.md for the design.

open tsumugi/core
