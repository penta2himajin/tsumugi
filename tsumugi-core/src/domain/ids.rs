//! Newtype-wrapped identifiers.
//!
//! IDs are opaque UUIDv4 values in the default constructor, but products can
//! supply their own (e.g., TRPG session-scoped keys) via `from_uuid`.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

macro_rules! id_newtype {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(
            Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(pub Uuid);

        impl $name {
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }

            pub fn from_uuid(uuid: Uuid) -> Self {
                Self(uuid)
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                self.0.fmt(f)
            }
        }
    };
}

id_newtype!(ChunkId, "Unique identifier for a Chunk.");
id_newtype!(FactId, "Unique identifier for a Fact.");
id_newtype!(PendingItemId, "Unique identifier for a PendingItem.");
