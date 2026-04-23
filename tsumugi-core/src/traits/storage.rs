//! StorageProvider: persistence abstraction for Chunks, Facts, PendingItems.
//!
//! Creative-only methods (`save_character`, `save_lore_entry`, …) are gated
//! behind the `creative` feature. Implementations can attach to in-memory,
//! SQLite, or cloud-backed stores.

use crate::domain::{Chunk, ChunkId, Fact, FactId, PendingItem, PendingItemId};
use async_trait::async_trait;

#[cfg(feature = "creative")]
use crate::creative::{Character, CharacterId, LoreEntry, LoreEntryId};

pub type StorageResult<T> = Result<T, StorageError>;

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("item not found: {kind} {id}")]
    NotFound { kind: &'static str, id: String },
    #[error("storage backend failed: {0}")]
    Backend(String),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

#[async_trait]
pub trait StorageProvider: Send + Sync {
    // Chunk
    async fn save_chunk(&self, chunk: &Chunk) -> StorageResult<()>;
    async fn load_chunk(&self, id: ChunkId) -> StorageResult<Chunk>;
    async fn delete_chunk(&self, id: ChunkId) -> StorageResult<()>;
    async fn list_chunks(&self) -> StorageResult<Vec<ChunkId>>;

    // Fact
    async fn save_fact(&self, fact: &Fact) -> StorageResult<()>;
    async fn load_fact(&self, id: FactId) -> StorageResult<Fact>;
    async fn delete_fact(&self, id: FactId) -> StorageResult<()>;
    async fn list_facts(&self) -> StorageResult<Vec<FactId>>;

    // PendingItem
    async fn save_pending(&self, item: &PendingItem) -> StorageResult<()>;
    async fn load_pending(&self, id: PendingItemId) -> StorageResult<PendingItem>;
    async fn delete_pending(&self, id: PendingItemId) -> StorageResult<()>;
    async fn list_pending(&self) -> StorageResult<Vec<PendingItemId>>;

    // Character (creative feature)
    #[cfg(feature = "creative")]
    async fn save_character(&self, character: &Character) -> StorageResult<()>;
    #[cfg(feature = "creative")]
    async fn load_character(&self, id: CharacterId) -> StorageResult<Character>;
    #[cfg(feature = "creative")]
    async fn delete_character(&self, id: CharacterId) -> StorageResult<()>;
    #[cfg(feature = "creative")]
    async fn list_characters(&self) -> StorageResult<Vec<CharacterId>>;

    // LoreEntry (creative feature)
    #[cfg(feature = "creative")]
    async fn save_lore(&self, entry: &LoreEntry) -> StorageResult<()>;
    #[cfg(feature = "creative")]
    async fn load_lore(&self, id: LoreEntryId) -> StorageResult<LoreEntry>;
    #[cfg(feature = "creative")]
    async fn delete_lore(&self, id: LoreEntryId) -> StorageResult<()>;
    #[cfg(feature = "creative")]
    async fn list_lore(&self) -> StorageResult<Vec<LoreEntryId>>;
}
