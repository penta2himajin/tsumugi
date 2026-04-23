//! tsumugi-core — Hierarchical narrative context middleware.
//!
//! See `docs/concept.md` and `docs/tech-architecture.md` for the design.
//!
//! # Features
//!
//! - `creative` (off by default): Character / SceneView / StylePreset / LoreEntry.
//!
//! # Layout
//!
//! - `gen/` is deterministically generated from `models/` via oxidtr.
//!   Do NOT hand-edit — regenerate with `scripts/regen.sh`.
//! - Only the **types subtree** (`gen/tsumugi/{core,creative}/`) is wired in
//!   here via `#[path]`. The scaffolding files oxidtr emits alongside
//!   (`operations.rs` / `newtypes.rs` / `tests.rs` / `fixtures.rs` /
//!   `helpers.rs`) are intentionally NOT included — Phase 0 / 1 revisits
//!   which scaffolding to adopt.
//! - Hand-written extensions live in `domain.rs` (core) and `creative.rs`
//!   (feature = "creative") which re-export and augment generated types.

#![forbid(unsafe_code)]

// Generated types subtree. Rooted as `crate::tsumugi::{core,creative}` so
// generated files referencing absolute paths like `crate::tsumugi::core::Chunk`
// resolve correctly. Visibility is `pub(crate)` — external crates use the
// `domain` / `creative` re-exports instead.
#[allow(dead_code, unused_imports, clippy::all)]
#[path = "gen/tsumugi"]
pub(crate) mod tsumugi {
    pub mod core;
    pub mod creative;
}

pub mod domain;
pub mod traits;

#[cfg(feature = "creative")]
pub mod creative;
