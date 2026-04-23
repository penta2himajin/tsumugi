//! Storage implementations.

mod in_memory;

pub use in_memory::InMemoryStorage;

#[cfg(feature = "sqlite")]
mod sqlite;

#[cfg(feature = "sqlite")]
pub use sqlite::SqliteStorage;
