//! Domain-agnostic core runtime types.
//!
//! The Alloy model in `models/tsumugi/core.als` captures the relational
//! skeleton (which chunks are children of which, which pending items
//! resolve where). oxidtr lowers that to `crate::generated::core` (see
//! `gen/`). The types here add the runtime payload (text, keywords,
//! metadata, timestamps) and trait-object-backed behaviors that Alloy
//! cannot express.
//!
//! Every type in this module has a structural counterpart in `generated::core`;
//! see `impl_structural_link!` for the bridge macros where a mapping exists.

mod chunk;
mod fact;
mod ids;
mod pending;
mod source_location;
mod summary;

pub use chunk::{Chunk, Keyword};
pub use fact::{Fact, FactOrigin, FactScope};
pub use ids::{ChunkId, FactId, PendingItemId};
pub use pending::{PendingItem, Priority};
pub use source_location::{FileSourceLocation, SourceLocation, SourceLocationValue};
pub use summary::SummaryMethod;
