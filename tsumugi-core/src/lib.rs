//! tsumugi-core — General-purpose memory-layer framework for LLM applications.
//!
//! See `docs/concept.md` and `docs/tech-architecture.md` for the design.
//!
//! # Layout
//!
//! - `gen/` — deterministically generated from `models/` via oxidtr.
//!   Do NOT hand-edit; regenerate with `scripts/regen.sh`. The generated
//!   types capture Alloy's relational skeleton; the runtime API (strings,
//!   metadata, timestamps, trait objects) is hand-written in the layers
//!   below.
//! - `domain/` — hand-written core types (Chunk, Fact, PendingItem, Ids,
//!   SourceLocation) and runtime fields beyond Alloy's scope.
//! - `traits/` — the 9 core trait definitions.
//! - `storage/` / `retriever/` / `scorer/` / `detector/` / `classifier/` /
//!   `compressor/` / `summarizer/` / `compiler/` — trait implementations.

#![forbid(unsafe_code)]

// Generated types subtree — Alloy-derived relational skeleton. `pub(crate)`
// because external users consume the hand-written `domain` re-exports
// instead. The alias name `tsumugi` is what the generated files reference
// via absolute paths (`crate::tsumugi::core::Chunk`).
#[allow(dead_code, unused_imports, clippy::all)]
#[path = "gen/tsumugi"]
pub(crate) mod tsumugi {
    pub mod core;
}

pub mod domain;
pub mod traits;

pub mod classifier;
pub mod compiler;
pub mod compressor;
pub mod detector;
pub mod providers;
pub mod retriever;
pub mod scorer;
pub mod storage;
pub mod summarizer;
