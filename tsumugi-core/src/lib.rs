//! tsumugi-core — Hierarchical narrative context middleware.
//!
//! See `docs/concept.md` and `docs/tech-architecture.md` for the design.
//!
//! # Features
//!
//! - `creative` (off by default): Character / SceneView / StylePreset / LoreEntry.

#![forbid(unsafe_code)]

pub mod domain;
pub mod traits;

#[cfg(feature = "creative")]
pub mod creative;
